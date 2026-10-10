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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use tokio::sync::oneshot;

use crate::credentials::{CredentialStore, Secret};
use crate::http_client::{
    HttpClient, HttpSettings, OPERATION_DEADLINE, run_cancellable, with_deadline,
};
use crate::model_catalog::{
    Catalog, CatalogCollector, IncompleteReason, OpenRouterPager, PageStep, models_url,
    openrouter_page_url, parse_page,
};
use crate::models::{ModelError, lock, parse_provider_id};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol};
use crate::storage::Storage;

/// The provider settings a fetch was started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshot {
    pub provider_id: ProviderId,
    pub kind: ProviderKind,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub revision: i64,
}

/// A snapshot plus the key read for it. `Secret`'s `Debug` output is redacted.
#[derive(Debug)]
pub struct PreparedFetch {
    pub snapshot: ProviderSnapshot,
    key: Secret,
}

/// What one fetch changed, returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFetchSummary {
    /// Whether the fetch saw the whole list (only then are models marked or pruned).
    pub complete: bool,
    pub incomplete_reason: Option<IncompleteReason>,
    /// Unique valid models in the vendor's response.
    pub listed: usize,
    /// Listed models that had no row and were inserted, not selected.
    pub added: usize,
    /// Listed models whose existing row was refreshed.
    pub updated: usize,
    /// Rows newly marked as missing upstream.
    pub marked_missing: usize,
    /// Unselected fetched rows deleted because the vendor no longer lists them.
    pub pruned: usize,
    /// Entries skipped because their ID was invalid or repeated.
    pub skipped_invalid: usize,
    pub fetched_at_ms: i64,
}

/// The last saved fetch of a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastModelFetch {
    pub fetched_at_ms: i64,
    pub complete: bool,
    pub listed_count: i64,
}

/// Whether a fetch is running and how the last saved one went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFetchStatus {
    pub running: bool,
    pub last_fetch: Option<LastModelFetch>,
}

/// The HTTP client and overall deadline used for fetches.
#[derive(Debug, Clone)]
pub struct ModelFetcher {
    client: HttpClient,
    deadline: Duration,
}

impl ModelFetcher {
    /// The production fetcher: system proxy, HTTPS only, 90 s overall deadline.
    ///
    /// # Errors
    /// `OperationFailed` when the HTTP client cannot be built.
    pub fn production() -> Result<Self, ModelError> {
        Ok(Self {
            client: HttpClient::new(HttpSettings::production())?,
            deadline: OPERATION_DEADLINE,
        })
    }

    /// A fetcher for the local test server, with a chosen overall deadline.
    #[cfg(test)]
    pub(crate) fn for_mock_server(deadline: Duration) -> Self {
        Self {
            client: HttpClient::new(HttpSettings::for_mock_server()).expect("test client"),
            deadline,
        }
    }
}

/// Running fetches, one per provider. Clones share the same registry.
#[derive(Debug, Clone, Default)]
pub struct ModelFetchRegistry {
    /// Provider ID → the cancel sender, or `None` once the fetch is merging.
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
}

/// Proof that a fetch is registered. Dropping it unregisters the fetch.
#[derive(Debug)]
pub struct FetchRegistration {
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
    provider_id: String,
}

impl ModelFetchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fetch and return its registration and cancel signal.
    ///
    /// # Errors
    /// `ModelFetchInProgress` when this provider already has a running fetch.
    pub fn begin(
        &self,
        provider_id: &ProviderId,
    ) -> Result<(FetchRegistration, oneshot::Receiver<()>), ModelError> {
        let mut running = self
            .running
            .lock()
            .map_err(|_| ModelError::OperationFailed)?;
        if running.contains_key(provider_id.as_str()) {
            return Err(ModelError::ModelFetchInProgress);
        }
        let (sender, receiver) = oneshot::channel();
        running.insert(provider_id.as_str().to_owned(), Some(sender));
        Ok((
            FetchRegistration {
                running: Arc::clone(&self.running),
                provider_id: provider_id.as_str().to_owned(),
            },
            receiver,
        ))
    }

    /// Ask a running fetch to stop. Returns whether a download was still running
    /// and received the signal; `false` when nothing runs or it is already merging.
    pub fn cancel(&self, provider_id: &ProviderId) -> bool {
        let Ok(mut running) = self.running.lock() else {
            return false;
        };
        match running.get_mut(provider_id.as_str()).and_then(Option::take) {
            Some(sender) => sender.send(()).is_ok(),
            None => false,
        }
    }

    /// Whether a fetch for this provider is registered.
    pub fn is_running(&self, provider_id: &ProviderId) -> bool {
        self.running
            .lock()
            .is_ok_and(|running| running.contains_key(provider_id.as_str()))
    }
}

impl FetchRegistration {
    /// Mark the fetch as merging: from now on `cancel` reports `false`.
    pub fn enter_merge(&self) {
        if let Ok(mut running) = self.running.lock()
            && let Some(slot) = running.get_mut(&self.provider_id)
        {
            *slot = None;
        }
    }
}

impl Drop for FetchRegistration {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&self.provider_id);
        }
    }
}

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
    let (snapshot, reference) = {
        let connection = lock(storage)?;
        let row: Option<(String, String, String, i64, Option<String>)> = connection
            .query_row(
                "SELECT p.kind, p.protocol, p.base_url, p.revision, c.credential_ref
                 FROM provider_instance p
                 LEFT JOIN provider_credential c ON c.provider_id = p.id
                 WHERE p.id = ?1",
                [provider_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| ModelError::ReadFailed)?;
        let (kind, protocol, base_url, revision, reference) = row.ok_or(ModelError::NotFound)?;
        let snapshot = ProviderSnapshot {
            provider_id: provider_id.clone(),
            kind: ProviderKind::from_value(&kind).ok_or(ModelError::InvalidStoredProvider)?,
            protocol: ProviderProtocol::from_value(&protocol)
                .ok_or(ModelError::InvalidStoredProvider)?,
            base_url,
            revision,
        };
        (snapshot, reference)
        // The connection guard is dropped here, before the credential store runs.
    };
    let reference = reference.ok_or(ModelError::SecretMissing)?;
    let key = store.load(&reference)?.ok_or(ModelError::SecretMissing)?;
    Ok(PreparedFetch { snapshot, key })
}

/// Download and parse the whole list, stopping at once on `cancel` or the deadline.
///
/// # Errors
/// `ModelFetchCancelled`, `RequestTimedOut`, a network or status code,
/// `UpstreamResponseInvalid`, or `InvalidStoredProvider` for an unusable base URL.
pub async fn download_catalog(
    fetcher: &ModelFetcher,
    prepared: &PreparedFetch,
    cancel: oneshot::Receiver<()>,
) -> Result<Catalog, ModelError> {
    let work = with_deadline(fetcher.deadline, download(&fetcher.client, prepared));
    run_cancellable(work, cancel).await
}

async fn download(client: &HttpClient, prepared: &PreparedFetch) -> Result<Catalog, ModelError> {
    let snapshot = &prepared.snapshot;
    let mut collector = CatalogCollector::new();
    if snapshot.kind != ProviderKind::OpenRouter {
        let url = models_url(&snapshot.base_url)?;
        let body = client.get_bounded(&url, &prepared.key).await?;
        collector.add_page(parse_page(snapshot.kind, &body)?);
        return Ok(collector.finish(false));
    }
    let mut pager = OpenRouterPager::new();
    let mut offset = pager.next_offset();
    loop {
        let url = openrouter_page_url(&snapshot.base_url, offset)?;
        let body = client.get_bounded(&url, &prepared.key).await?;
        let page = parse_page(snapshot.kind, &body)?;
        let step = pager.record(&page);
        collector.add_page(page);
        match step {
            PageStep::Fetch { offset: next } => offset = next,
            PageStep::Done => return Ok(collector.finish(false)),
            PageStep::PageLimitReached => return Ok(collector.finish(true)),
        }
    }
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

/// Report whether a fetch is running and the last saved fetch.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `NotFound`, or a storage code.
pub fn fetch_status(
    storage: &Storage,
    registry: &ModelFetchRegistry,
    provider_id: &str,
) -> Result<ModelFetchStatus, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let connection = lock(storage)?;
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM provider_instance WHERE id = ?1",
            [provider_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    exists.ok_or(ModelError::NotFound)?;
    let last_fetch = connection
        .query_row(
            "SELECT fetched_at, complete, listed_count FROM provider_model_fetch
             WHERE provider_id = ?1",
            [provider_id.as_str()],
            |row| {
                Ok(LastModelFetch {
                    fetched_at_ms: row.get(0)?,
                    complete: row.get::<_, i64>(1)? == 1,
                    listed_count: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    Ok(ModelFetchStatus {
        running: registry.is_running(&provider_id),
        last_fetch,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    use serde_json::json;

    use super::*;
    use crate::credentials::CredentialError;
    use crate::credentials::fake::FakeStore;
    use crate::http_client::HttpError;
    use crate::http_client::test_server::{MockServer, Reply, response};
    use crate::model_catalog::RouteSupport;
    use crate::models::{
        AddManualModelRequest, ListProviderModelsRequest, ModelSource, ProviderModel,
        SetModelsSelectedRequest, UpstreamState, add_manual_model, list_provider_models,
        set_models_selected,
    };

    const PROVIDER: &str = "0123456789abcdef0123456789abcdef";
    /// A made-up key that is easy to search for in files and messages.
    const KEY: &str = "sk-vibemate-SYNTHETIC-fetch-key-4b1d";
    const COMMAND_CODE_FIXTURE: &[u8] =
        include_bytes!("../tests/fixtures/command-code-models-2026-10-10.json");

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("vibemate-model-fetch-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn open(&self) -> Storage {
            Storage::open_in_directory(&self.0).expect("open storage")
        }

        fn database_bytes(&self) -> Vec<u8> {
            fs::read(self.0.join("vibemate.sqlite3")).expect("read database file")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn provider_id() -> ProviderId {
        ProviderId::parse(PROVIDER).unwrap()
    }

    fn reference() -> String {
        provider_id().credential_reference()
    }

    fn key_store() -> FakeStore {
        FakeStore::holding(&reference(), KEY)
    }

    /// Insert a provider (revision 1) and its key reference directly.
    fn insert_provider(storage: &Storage, kind: &str, protocol: &str, base_url: &str) {
        let connection = storage.lock().unwrap();
        connection
            .execute(
                "INSERT INTO provider_instance
                     (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
                 VALUES (?1, ?2, 'Test', ?3, ?4, 1, 1, 1)",
                params![PROVIDER, kind, base_url, protocol],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
                 VALUES (?1, ?2, 1)",
                params![PROVIDER, reference()],
            )
            .unwrap();
    }

    fn ok_json(body: &serde_json::Value) -> Reply {
        Reply::Respond(response(
            200,
            &[("Content-Type", "application/json")],
            body.to_string().as_bytes(),
        ))
    }

    fn deepseek_body(ids: &[&str]) -> serde_json::Value {
        json!({
            "object": "list",
            "data": ids.iter().map(|id| json!({
                "id": id, "object": "model", "owned_by": "deepseek",
                "context_window": 1_000_000, "max_output_tokens": 384_000,
                "input_modalities": ["text"], "output_modalities": ["text"]
            })).collect::<Vec<_>>()
        })
    }

    fn openrouter_body(ids: &[&str], more: bool) -> serde_json::Value {
        json!({
            "data": ids.iter().map(|id| json!({
                "id": id, "name": format!("Name {id}"), "context_length": 128_000,
                "architecture": {"input_modalities": ["text", "image"], "output_modalities": ["text"]},
                "top_provider": {"max_completion_tokens": 4096}
            })).collect::<Vec<_>>(),
            "total_count": 5,
            "links": {"next": if more { json!("/api/v1/models?offset=x") } else { json!(null) }}
        })
    }

    fn fetcher() -> ModelFetcher {
        ModelFetcher::for_mock_server(Duration::from_secs(20))
    }

    async fn fetch(
        storage: &Storage,
        store: &FakeStore,
        now_ms: i64,
    ) -> Result<ModelFetchSummary, ModelError> {
        fetch_provider_models(
            storage,
            store,
            &fetcher(),
            &ModelFetchRegistry::new(),
            PROVIDER,
            now_ms,
        )
        .await
    }

    fn all_models(storage: &Storage) -> Vec<ProviderModel> {
        list_provider_models(
            storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: None,
                limit: 200,
                filter: "all".to_owned(),
                query: None,
            },
        )
        .unwrap()
        .items
    }

    fn model<'a>(models: &'a [ProviderModel], id: &str) -> Option<&'a ProviderModel> {
        models.iter().find(|model| model.model_id == id)
    }

    fn count_rows(storage: &Storage, table: &str) -> i64 {
        storage
            .lock()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn assert_nothing_written(storage: &Storage) {
        assert_eq!(count_rows(storage, "provider_model"), 0);
        assert_eq!(count_rows(storage, "provider_model_fetch"), 0);
    }

    /// Whether the server's request heads carried the key as a Bearer token.
    fn sent_bearer(server: &MockServer) -> bool {
        let expected = format!("authorization: bearer {KEY}").to_lowercase();
        server
            .requests()
            .iter()
            .all(|head| head.to_lowercase().contains(&expected))
    }

    fn select(storage: &Storage, ids: &[&str]) {
        set_models_selected(
            storage,
            &SetModelsSelectedRequest {
                provider_id: PROVIDER.to_owned(),
                model_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
                selected: true,
            },
            50,
        )
        .unwrap();
    }

    async fn wait_for_requests(server: &MockServer, count: usize) {
        let start = Instant::now();
        while server.requests().len() < count {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "request never arrived"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn wait_for_close(server: &MockServer) {
        let start = Instant::now();
        while server.closed.try_recv().is_err() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "connection never closed"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn command_code_fixture_is_fetched_with_the_key_and_stored_unselected() {
        let server = MockServer::start(vec![Reply::Respond(response(
            200,
            &[("Content-Type", "application/json")],
            COMMAND_CODE_FIXTURE,
        ))]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        let base = server.url("/provider/v1").to_string();
        insert_provider(&storage, "command-code", "chat_completions", &base);

        let summary = fetch(&storage, &key_store(), 1_000).await.unwrap();
        assert_eq!(
            summary,
            ModelFetchSummary {
                complete: true,
                incomplete_reason: None,
                listed: 87,
                added: 87,
                updated: 0,
                marked_missing: 0,
                pruned: 0,
                skipped_invalid: 0,
                fetched_at_ms: 1_000,
            }
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /provider/v1/models HTTP/1.1"));
        assert!(sent_bearer(&server));

        let mut models = all_models(&storage);
        let selected_page = list_provider_models(
            &storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: None,
                limit: 200,
                filter: "selected".to_owned(),
                query: None,
            },
        )
        .unwrap();
        assert!(
            selected_page.items.is_empty(),
            "new models are not selected"
        );
        assert_eq!(models.len(), 87);
        assert!(
            models
                .iter()
                .all(|model| model.source == ModelSource::Fetched
                    && model.upstream_state == UpstreamState::Listed
                    && model.last_seen_at_ms == Some(1_000))
        );
        assert_eq!(
            models
                .iter()
                .filter(|model| model.model_id.contains('/'))
                .count(),
            65
        );
        let supported = models
            .iter()
            .filter(|model| model.route_support == RouteSupport::Supported)
            .count();
        assert_eq!(supported, 76);
        models.retain(|model| model.route_support == RouteSupport::Unsupported);
        assert_eq!(models.len(), 11);

        let status = fetch_status(&storage, &ModelFetchRegistry::new(), PROVIDER).unwrap();
        assert_eq!(
            status,
            ModelFetchStatus {
                running: false,
                last_fetch: Some(LastModelFetch {
                    fetched_at_ms: 1_000,
                    complete: true,
                    listed_count: 87
                }),
            }
        );
    }

    #[tokio::test]
    async fn deepseek_list_maps_metadata() {
        let server = MockServer::start(vec![ok_json(&deepseek_body(&[
            "deepseek-v4-flash",
            "deepseek-v4-pro",
        ]))]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );

        let summary = fetch(&storage, &key_store(), 10).await.unwrap();
        assert_eq!(
            (summary.listed, summary.added, summary.complete),
            (2, 2, true)
        );
        assert!(server.requests()[0].starts_with("GET /models HTTP/1.1"));
        assert!(sent_bearer(&server));
        let models = all_models(&storage);
        let flash = model(&models, "deepseek-v4-flash").unwrap();
        assert_eq!(flash.context_window, Some(1_000_000));
        assert_eq!(flash.max_output_tokens, Some(384_000));
        assert_eq!(
            flash.input_modalities.as_deref(),
            Some(&["text".to_owned()][..])
        );
        assert_eq!(flash.route_support, RouteSupport::NotApplicable);
    }

    #[tokio::test]
    async fn openrouter_pages_are_requested_with_computed_offsets() {
        let server = MockServer::start(vec![
            ok_json(&openrouter_body(&["a/one", "a/two"], true)),
            ok_json(&openrouter_body(&["b/three", "a/one"], true)),
            ok_json(&openrouter_body(&["c/four"], false)),
        ]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "openrouter",
            "chat_completions",
            server.url("/api/v1").as_str(),
        );

        let summary = fetch(&storage, &key_store(), 10).await.unwrap();
        assert_eq!(
            (summary.listed, summary.added, summary.complete),
            (4, 4, true)
        );
        assert_eq!(summary.skipped_invalid, 1, "the repeated a/one is counted");
        let lines: Vec<String> = server
            .requests()
            .iter()
            .map(|head| head.lines().next().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(
            lines,
            [
                "GET /api/v1/models?offset=0&limit=500 HTTP/1.1",
                "GET /api/v1/models?offset=2&limit=500 HTTP/1.1",
                "GET /api/v1/models?offset=4&limit=500 HTTP/1.1",
            ]
        );
        assert!(sent_bearer(&server));
        let models = all_models(&storage);
        let one = model(&models, "a/one").unwrap();
        assert_eq!(one.upstream_name.as_deref(), Some("Name a/one"));
        assert_eq!(one.max_output_tokens, Some(4096));
        assert_eq!(
            one.output_modalities.as_deref(),
            Some(&["text".to_owned()][..])
        );
    }

    #[tokio::test]
    async fn openrouter_page_cap_makes_the_fetch_incomplete_and_prunes_nothing() {
        let replies = (0..10)
            .map(|page| ok_json(&openrouter_body(&[&format!("m/{page}")], true)))
            .collect();
        let server = MockServer::start(replies);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "openrouter",
            "chat_completions",
            server.url("/api/v1").as_str(),
        );
        storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO provider_model (provider_id, model_id, source, selected,
                     created_at, updated_at, last_seen_at)
                 VALUES (?1, 'old/unselected', 'fetched', 0, 1, 1, 1)",
                [PROVIDER],
            )
            .unwrap();

        let summary = fetch(&storage, &key_store(), 10).await.unwrap();
        assert_eq!(server.requests().len(), 10);
        assert!(!summary.complete);
        assert_eq!(summary.incomplete_reason, Some(IncompleteReason::PageLimit));
        assert_eq!(
            (summary.added, summary.pruned, summary.marked_missing),
            (10, 0, 0)
        );
        let models = all_models(&storage);
        let old = model(&models, "old/unselected").expect("kept by an incomplete fetch");
        assert_eq!(old.upstream_state, UpstreamState::Listed);
        let status = fetch_status(&storage, &ModelFetchRegistry::new(), PROVIDER).unwrap();
        assert_eq!(status.last_fetch.map(|last| last.complete), Some(false));
    }

    #[tokio::test]
    async fn merging_across_fetches_keeps_selected_and_manual_models() {
        let server = MockServer::start(vec![
            ok_json(&deepseek_body(&["a", "b", "c", "x"])),
            ok_json(&deepseek_body(&["a", "d"])),
            ok_json(&deepseek_body(&["a", "b", "d"])),
            ok_json(&deepseek_body(&[])),
        ]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let store = key_store();
        for (id, alias) in [("x", Some("Mine")), ("never", None)] {
            add_manual_model(
                &storage,
                &AddManualModelRequest {
                    provider_id: PROVIDER.to_owned(),
                    model_id: id.to_owned(),
                    alias: alias.map(str::to_owned),
                },
                5,
            )
            .unwrap();
        }

        // Fetch 1: x is listed and gets metadata, but stays manual with its alias.
        let first = fetch(&storage, &store, 100).await.unwrap();
        assert_eq!((first.listed, first.added, first.updated), (4, 3, 1));
        let models = all_models(&storage);
        let x = model(&models, "x").unwrap();
        assert_eq!(x.source, ModelSource::Manual);
        assert_eq!(x.alias.as_deref(), Some("Mine"));
        assert!(x.selected);
        assert_eq!(x.context_window, Some(1_000_000));
        assert_eq!(x.upstream_state, UpstreamState::Listed);
        assert!(
            !model(&models, "a").unwrap().selected,
            "new models are not selected"
        );
        select(&storage, &["a", "b"]);

        // Fetch 2 lists only a and d.
        let second = fetch(&storage, &store, 200).await.unwrap();
        assert_eq!(
            (
                second.added,
                second.updated,
                second.marked_missing,
                second.pruned
            ),
            (1, 1, 2, 1)
        );
        let models = all_models(&storage);
        let ids: Vec<&str> = models.iter().map(|model| model.model_id.as_str()).collect();
        assert_eq!(
            ids,
            ["a", "b", "d", "never", "x"],
            "unselected c was pruned"
        );
        let b = model(&models, "b").unwrap();
        assert!(b.selected, "a selected model is never deleted");
        assert_eq!(b.upstream_state, UpstreamState::Missing);
        assert_eq!(b.missing_since_ms, Some(200));
        assert_eq!(
            model(&models, "x").unwrap().upstream_state,
            UpstreamState::Missing
        );
        let never = model(&models, "never").unwrap();
        assert_eq!(never.upstream_state, UpstreamState::NeverListed);
        assert_eq!(
            never.updated_at_ms, 5,
            "a never-listed manual row is untouched"
        );
        assert!(!model(&models, "d").unwrap().selected);

        // Fetch 3: b is back, x stays missing since its first absence.
        let third = fetch(&storage, &store, 300).await.unwrap();
        assert_eq!(
            (third.added, third.updated, third.marked_missing),
            (0, 3, 0)
        );
        let models = all_models(&storage);
        let b = model(&models, "b").unwrap();
        assert_eq!(
            (b.upstream_state, b.missing_since_ms, b.last_seen_at_ms),
            (UpstreamState::Listed, None, Some(300))
        );
        assert!(b.selected);
        assert_eq!(model(&models, "x").unwrap().missing_since_ms, Some(200));

        // Fetch 4: an empty list is incomplete and changes nothing.
        let fourth = fetch(&storage, &store, 400).await.unwrap();
        assert_eq!(fourth.incomplete_reason, Some(IncompleteReason::EmptyList));
        assert_eq!((fourth.pruned, fourth.marked_missing), (0, 0));
        assert_eq!(all_models(&storage), models);
    }

    #[tokio::test]
    async fn cancelling_closes_the_request_and_writes_nothing() {
        let server = MockServer::start(vec![Reply::Hang]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let store = key_store();
        let registry = ModelFetchRegistry::new();
        let fetcher = fetcher();

        let started = Instant::now();
        let (result, cancelled) = tokio::join!(
            fetch_provider_models(&storage, &store, &fetcher, &registry, PROVIDER, 10),
            async {
                wait_for_requests(&server, 1).await;
                assert!(registry.is_running(&provider_id()));
                // A second fetch for the same provider is refused meanwhile.
                let second =
                    fetch_provider_models(&storage, &store, &fetcher, &registry, PROVIDER, 10)
                        .await;
                assert_eq!(second, Err(ModelError::ModelFetchInProgress));
                registry.cancel(&provider_id())
            }
        );
        assert!(cancelled, "the download was still running");
        assert_eq!(result, Err(ModelError::ModelFetchCancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
        wait_for_close(&server).await;
        assert_nothing_written(&storage);
        assert!(
            !registry.is_running(&provider_id()),
            "the registration was released"
        );
        assert!(!registry.cancel(&provider_id()), "nothing left to cancel");
    }

    #[tokio::test]
    async fn the_overall_deadline_stops_a_hanging_fetch() {
        let server = MockServer::start(vec![Reply::Hang]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let started = Instant::now();
        let result = fetch_provider_models(
            &storage,
            &key_store(),
            &ModelFetcher::for_mock_server(Duration::from_millis(300)),
            &ModelFetchRegistry::new(),
            PROVIDER,
            10,
        )
        .await;
        assert_eq!(result, Err(ModelError::RequestTimedOut));
        assert!(started.elapsed() < Duration::from_secs(5));
        wait_for_close(&server).await;
        assert_nothing_written(&storage);
    }

    #[tokio::test]
    async fn error_statuses_and_bad_bodies_map_to_codes_without_writes() {
        for (reply, expected) in [
            (
                Reply::Respond(response(401, &[], b"")),
                ModelError::AuthRejected,
            ),
            (
                Reply::Respond(response(403, &[], b"")),
                ModelError::AuthRejected,
            ),
            (
                Reply::Respond(response(402, &[], b"")),
                ModelError::InsufficientBalance,
            ),
            (
                Reply::Respond(response(429, &[], b"")),
                ModelError::RateLimited,
            ),
            (
                Reply::Respond(response(500, &[], b"")),
                ModelError::UpstreamUnavailable,
            ),
            (
                Reply::Respond(response(503, &[], b"")),
                ModelError::UpstreamUnavailable,
            ),
            (
                Reply::Respond(response(302, &[("Location", "http://127.0.0.1:9/")], b"")),
                ModelError::UpstreamResponseInvalid,
            ),
            (
                Reply::Respond(response(200, &[], b"<html>not json</html>")),
                ModelError::UpstreamResponseInvalid,
            ),
            (Reply::Close, ModelError::ConnectionFailed),
        ] {
            let server = MockServer::start(vec![reply]);
            let directory = TestDirectory::new();
            let storage = directory.open();
            insert_provider(
                &storage,
                "deepseek",
                "chat_completions",
                server.url("").as_str(),
            );
            let error = fetch(&storage, &key_store(), 10).await.unwrap_err();
            assert_eq!(error, expected);
            for text in [
                error.to_string(),
                format!("{error:?}"),
                serde_json::to_string(&error).unwrap(),
            ] {
                assert!(!text.contains(KEY), "{text}");
            }
            assert_nothing_written(&storage);
        }
    }

    #[tokio::test]
    async fn settings_changed_during_the_fetch_discard_the_result() {
        for change in [
            "UPDATE provider_instance SET revision = 2",
            "UPDATE provider_instance SET base_url = 'https://other.example.com'",
            "DELETE FROM provider_instance",
        ] {
            let server = MockServer::start(vec![ok_json(&deepseek_body(&["a"]))]);
            let directory = TestDirectory::new();
            let storage = directory.open();
            insert_provider(
                &storage,
                "deepseek",
                "chat_completions",
                server.url("").as_str(),
            );
            let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
            let (_sender, cancel) = oneshot::channel();
            let catalog = download_catalog(&fetcher(), &prepared, cancel)
                .await
                .unwrap();
            storage.lock().unwrap().execute(change, []).unwrap();

            let result = merge_catalog(&storage, &prepared.snapshot, &catalog, 10);
            let expected = if change.starts_with("DELETE") {
                ModelError::NotFound
            } else {
                ModelError::ModelFetchStale
            };
            assert_eq!(result, Err(expected), "{change}");
            assert_nothing_written(&storage);
        }
    }

    #[tokio::test]
    async fn a_missing_or_unreadable_key_stops_before_any_request() {
        let server = MockServer::start(vec![]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );

        // A reference without a stored key.
        assert_eq!(
            fetch(&storage, &FakeStore::default(), 10).await,
            Err(ModelError::SecretMissing)
        );
        let locked = key_store();
        locked.fail_loads(Some(CredentialError::AccessDenied));
        assert_eq!(
            fetch(&storage, &locked, 10).await,
            Err(ModelError::CredentialStoreAccessDenied)
        );
        locked.fail_loads(Some(CredentialError::Unavailable));
        assert_eq!(
            fetch(&storage, &locked, 10).await,
            Err(ModelError::CredentialStoreUnavailable)
        );
        // No reference at all (a provider saved before keys existed).
        storage
            .lock()
            .unwrap()
            .execute("DELETE FROM provider_credential", [])
            .unwrap();
        assert_eq!(
            fetch(&storage, &key_store(), 10).await,
            Err(ModelError::SecretMissing)
        );
        // Unknown provider and malformed ID.
        storage
            .lock()
            .unwrap()
            .execute("DELETE FROM provider_instance", [])
            .unwrap();
        assert_eq!(
            fetch(&storage, &key_store(), 10).await,
            Err(ModelError::NotFound)
        );
        let malformed = fetch_provider_models(
            &storage,
            &key_store(),
            &fetcher(),
            &ModelFetchRegistry::new(),
            "not-an-id",
            10,
        )
        .await;
        assert_eq!(malformed, Err(ModelError::InvalidRequest));
        assert!(server.requests().is_empty());
        assert_nothing_written(&storage);
    }

    #[tokio::test]
    async fn an_unusable_stored_base_url_is_a_stored_data_error() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, "deepseek", "chat_completions", "not a url");
        assert_eq!(
            fetch(&storage, &key_store(), 10).await,
            Err(ModelError::InvalidStoredProvider)
        );
        assert_nothing_written(&storage);
    }

    #[tokio::test]
    async fn the_key_never_reaches_the_database_or_any_output() {
        let server = MockServer::start(vec![
            ok_json(&deepseek_body(&["a"])),
            Reply::Respond(response(401, &[], KEY.as_bytes())),
        ]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let summary = fetch(&storage, &key_store(), 10).await.unwrap();
        // The second response echoes the key in its body; the body is never read.
        let error = fetch(&storage, &key_store(), 20).await.unwrap_err();
        assert_eq!(error, ModelError::AuthRejected);
        let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
        drop(storage);

        let database = directory.database_bytes();
        assert!(
            !database
                .windows(KEY.len())
                .any(|window| window == KEY.as_bytes())
        );
        for text in [
            format!("{summary:?}"),
            serde_json::to_string(&summary).unwrap(),
            format!("{error:?} {error}"),
            format!("{prepared:?}"),
        ] {
            assert!(!text.contains(KEY), "{text}");
        }
    }

    #[test]
    fn the_registry_allows_one_fetch_per_provider_and_stops_cancel_once_merging() {
        let registry = ModelFetchRegistry::new();
        let other = ProviderId::parse("fedcba9876543210fedcba9876543210").unwrap();
        assert!(!registry.cancel(&provider_id()));
        let (registration, mut cancel) = registry.begin(&provider_id()).unwrap();
        assert_eq!(
            registry.begin(&provider_id()).map(|_| ()),
            Err(ModelError::ModelFetchInProgress)
        );
        assert!(
            registry.begin(&other).is_ok(),
            "other providers are independent"
        );
        assert!(
            registry.clone().is_running(&provider_id()),
            "clones share state"
        );

        registration.enter_merge();
        assert!(!registry.cancel(&provider_id()), "too late once merging");
        assert!(cancel.try_recv().is_err());
        drop(registration);
        assert!(!registry.is_running(&provider_id()));

        let (_registration, mut cancel) = registry.begin(&provider_id()).unwrap();
        assert!(registry.cancel(&provider_id()));
        assert_eq!(cancel.try_recv(), Ok(()));
        assert!(
            !registry.cancel(&provider_id()),
            "a signal is sent only once"
        );
    }

    #[test]
    fn network_errors_keep_their_http_codes() {
        for error in [
            HttpError::ClientUnavailable,
            HttpError::InvalidUrl,
            HttpError::SecretInvalid,
            HttpError::ConnectionFailed,
            HttpError::TlsFailed,
            HttpError::RequestTimedOut,
            HttpError::AuthRejected,
            HttpError::InsufficientBalance,
            HttpError::RateLimited,
            HttpError::UpstreamUnavailable,
            HttpError::UpstreamResponseInvalid,
            HttpError::ResponseTooLarge,
            HttpError::Cancelled,
        ] {
            let model_error = ModelError::from(error);
            assert_eq!(model_error.code(), error.code());
            assert_eq!(serde_json::to_value(model_error).unwrap(), error.code());
        }
    }
}
