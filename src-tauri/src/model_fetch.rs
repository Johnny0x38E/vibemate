//! Fetch a provider's model list and merge it into `provider_model` (P12).
//!
//! A fetch runs only when the user asks for it. The steps, in order:
//!
//! 1. **Register** in `ModelFetchRegistry`. A second fetch for the same provider
//!    is refused with `model_fetch_in_progress`. Registering returns the cancel
//!    signal for this fetch.
//! 2. **Snapshot** (`prepare_fetch`): read kind, protocol, base URL and revision
//!    under the database lock, release the lock, then read the API key from the
//!    credential store. The store can show an OS prompt, so the lock is never held
//!    while it runs. No key reference or no stored key is `secret_missing`.
//! 3. **Download** (`download_catalog`): send `GET <base_url>/models` with
//!    `Authorization: Bearer <key>` to every vendor (decision D17). OpenRouter is
//!    paged with offsets vibemate computes itself. Cancelling drops the request at
//!    once, and the whole download has an overall deadline (90 s in production).
//! 4. **Merge** (`merge_catalog`): in one `IMMEDIATE` transaction, check that the
//!    provider still has the snapshot's revision and base URL (else
//!    `model_fetch_stale`; deleted is `not_found`), then apply the merge rules and
//!    record the fetch summary. Once merging starts, cancelling has no effect.
//!
//! Steps 2 and 4 are blocking; the Tauri command (P12.b.1) runs them with
//! `spawn_blocking`. `fetch_provider_models` runs all steps in order for callers
//! that may block, such as tests.
//!
//! Merge rules (a complete fetch saw the whole, non-empty list without hitting a cap):
//! - A listed model that has no row is inserted as `fetched` and **not selected**.
//! - A listed model that has a row gets fresh metadata, `last_seen_at = now` and
//!   no `missing_since`. Its `source`, `selected` and `alias` never change, so a
//!   manual row stays manual when the vendor later lists the same ID.
//! - On a complete fetch, a row that is not listed:
//!   - fetched and not selected: deleted;
//!   - fetched and selected: kept, and `missing_since` is set (if not already);
//!   - manual and listed before: kept, and `missing_since` is set (if not already);
//!   - manual and never listed: unchanged.
//! - An incomplete fetch (page cap, model cap, or an empty list) only inserts and
//!   updates. It never marks a model missing and never deletes one.
//!
//! The API key exists only in the `Secret` passed to the HTTP client. Errors are
//! payload-free codes, and nothing here logs, stores or returns the key.

mod types;
pub use types::{
    BrowseUpstreamModelsRequest, LastModelFetch, ModelFetchStatus, ModelFetchSummary,
    PreparedFetch, ProviderSnapshot, UPSTREAM_BROWSE_PAGE_SIZE, UpstreamBrowseModel,
    UpstreamBrowsePage,
};
mod cache;
mod registry;
pub use registry::{FetchRegistration, ModelFetchRegistry};
mod download;
pub use download::{ModelFetcher, download_catalog};
mod browse;
pub use browse::browse_upstream_page;

use crate::credentials::CredentialStore;
use crate::model_catalog::Catalog;
use crate::models::{ModelError, parse_provider_id};
mod repository;
use crate::providers::ProviderId;
use crate::storage::Storage;
pub use repository::fetch_status;
use repository::lock;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::collections::{HashMap, HashSet};

/// Read the provider snapshot, release the database, then read the key.
///
/// # Errors
/// `NotFound`, `InvalidStoredProvider`, `SecretMissing`, a credential-store code,
/// or a storage code.
pub fn prepare_fetch(
    storage: &Storage,
    store: &dyn CredentialStore,
    provider_id: &ProviderId,
) -> Result<PreparedFetch, ModelError> {
    let (snapshot, reference) = repository::read_provider_snapshot(storage, provider_id)?;
    let reference = reference.ok_or(ModelError::SecretMissing)?;
    let key = store.load(&reference)?.ok_or(ModelError::SecretMissing)?;
    Ok(PreparedFetch { snapshot, key })
}

/// One existing row, as far as merging needs it.
struct ExistingRow {
    manual: bool,
    selected: bool,
    listed_before: bool,
    missing: bool,
}

/// Check that the provider is unchanged, apply the merge rules, and record the
/// fetch, all in one transaction.
///
/// # Errors
/// `NotFound` when the provider was deleted, `ModelFetchStale` when its revision or
/// base URL changed, or a storage code. On any error nothing is written.
pub fn merge_catalog(
    storage: &Storage,
    snapshot: &ProviderSnapshot,
    catalog: &Catalog,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    let provider_id = snapshot.provider_id.as_str();
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;

    let current: Option<(i64, String)> = transaction
        .query_row(
            "SELECT revision, base_url FROM provider_instance WHERE id = ?1",
            [provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    let (revision, base_url) = current.ok_or(ModelError::NotFound)?;
    if revision != snapshot.revision || base_url != snapshot.base_url {
        return Err(ModelError::ModelFetchStale);
    }

    let mut existing = HashMap::new();
    {
        let mut statement = transaction
            .prepare(
                "SELECT model_id, source, selected, last_seen_at, missing_since
                 FROM provider_model WHERE provider_id = ?1",
            )
            .map_err(|_| ModelError::ReadFailed)?;
        let rows = statement
            .query_map([provider_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ExistingRow {
                        manual: row.get::<_, String>(1)? == "manual",
                        selected: row.get::<_, i64>(2)? == 1,
                        listed_before: row.get::<_, Option<i64>>(3)?.is_some(),
                        missing: row.get::<_, Option<i64>>(4)?.is_some(),
                    },
                ))
            })
            .map_err(|_| ModelError::ReadFailed)?;
        for row in rows {
            let (model_id, state) = row.map_err(|_| ModelError::ReadFailed)?;
            existing.insert(model_id, state);
        }
    }

    let mut summary = ModelFetchSummary {
        complete: catalog.incomplete.is_none(),
        incomplete_reason: catalog.incomplete,
        listed: catalog.models.len(),
        added: 0,
        updated: 0,
        marked_missing: 0,
        pruned: 0,
        skipped_invalid: catalog.skipped_invalid + catalog.duplicates,
        fetched_at_ms: now_ms,
    };

    let mut listed = HashSet::with_capacity(catalog.models.len());
    for model in &catalog.models {
        listed.insert(model.model_id.as_str());
        let json = |list: &Option<Vec<String>>| -> Result<Option<String>, ModelError> {
            list.as_ref()
                .map(|items| serde_json::to_string(items).map_err(|_| ModelError::WriteFailed))
                .transpose()
        };
        let input = json(&model.input_modalities)?;
        let output = json(&model.output_modalities)?;
        let endpoints = json(&model.supported_endpoints)?;
        if existing.contains_key(&model.model_id) {
            transaction
                .execute(
                    "UPDATE provider_model SET
                         upstream_name = ?3, context_window = ?4, max_output_tokens = ?5,
                         input_modalities = ?6, output_modalities = ?7,
                         supported_endpoints = ?8, last_seen_at = ?9, missing_since = NULL,
                         updated_at = max(updated_at, ?9)
                     WHERE provider_id = ?1 AND model_id = ?2",
                    params![
                        provider_id,
                        model.model_id,
                        model.upstream_name,
                        model.context_window,
                        model.max_output_tokens,
                        input,
                        output,
                        endpoints,
                        now_ms
                    ],
                )
                .map_err(|_| ModelError::WriteFailed)?;
            summary.updated += 1;
        } else {
            transaction
                .execute(
                    "INSERT INTO provider_model
                         (provider_id, model_id, source, selected, upstream_name, context_window,
                          max_output_tokens, input_modalities, output_modalities,
                          supported_endpoints, created_at, updated_at, last_seen_at)
                     VALUES (?1, ?2, 'fetched', 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?9)",
                    params![
                        provider_id,
                        model.model_id,
                        model.upstream_name,
                        model.context_window,
                        model.max_output_tokens,
                        input,
                        output,
                        endpoints,
                        now_ms
                    ],
                )
                .map_err(|_| ModelError::WriteFailed)?;
            summary.added += 1;
        }
    }

    if summary.complete {
        for (model_id, row) in &existing {
            if listed.contains(model_id.as_str()) {
                continue;
            }
            if !row.manual && !row.selected {
                transaction
                    .execute(
                        "DELETE FROM provider_model WHERE provider_id = ?1 AND model_id = ?2",
                        params![provider_id, model_id],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
                summary.pruned += 1;
            } else if row.listed_before && !row.missing {
                transaction
                    .execute(
                        "UPDATE provider_model
                         SET missing_since = ?3, updated_at = max(updated_at, ?3)
                         WHERE provider_id = ?1 AND model_id = ?2",
                        params![provider_id, model_id, now_ms],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
                summary.marked_missing += 1;
            }
        }
    }

    let listed_count = i64::try_from(summary.listed).map_err(|_| ModelError::WriteFailed)?;
    transaction
        .execute(
            "INSERT INTO provider_model_fetch (provider_id, fetched_at, complete, listed_count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (provider_id) DO UPDATE SET
                 fetched_at = excluded.fetched_at,
                 complete = excluded.complete,
                 listed_count = excluded.listed_count",
            params![provider_id, now_ms, summary.complete, listed_count],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(summary)
}

/// Run a whole fetch: register, snapshot and key, download, merge.
///
/// The snapshot and merge steps block on SQLite and the credential store, so call
/// this only where blocking is allowed. The Tauri command runs those steps with
/// `spawn_blocking` instead.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `ModelFetchInProgress`, and every error of
/// `prepare_fetch`, `download_catalog` and `merge_catalog`.
pub async fn fetch_provider_models(
    storage: &Storage,
    store: &dyn CredentialStore,
    fetcher: &ModelFetcher,
    registry: &ModelFetchRegistry,
    provider_id: &str,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let (registration, cancel) = registry.begin(&provider_id)?;
    let prepared = prepare_fetch(storage, store, &provider_id)?;
    let catalog = download_catalog(fetcher, &prepared, cancel).await?;
    // The key is no longer needed; drop it before touching the database again.
    let PreparedFetch { snapshot, key } = prepared;
    drop(key);
    registration.enter_merge();
    merge_catalog(storage, &snapshot, &catalog, now_ms)
}

#[cfg(test)]
mod tests;
