//! Refresh the upstream metadata of already saved models (P16.c).
//!
//! Unlike `merge_catalog`, this path never inserts a row that the user has not
//! saved and never deletes one. It exists so a saved model's name, limits,
//! modalities, routing and capability evidence can be updated from the provider's
//! current catalog without turning the Models tab into a full catalog mirror.
//! `source`, `selected` and `alias` are preserved: a manual row stays manual, and
//! a user's display alias is never overwritten.
//!
//! Only a complete catalog can establish absence: when the whole list was seen, a
//! saved row that is missing from it gets `missing_since`. An incomplete fetch
//! (page cap, model cap or an empty list) only updates metadata.
//!
//! The provider is rechecked inside the transaction, so a settings change or a
//! deletion during the download makes the refresh fail with `model_fetch_stale`
//! or `not_found` and writes nothing.

use super::repository::lock;
use super::types::{ModelRefreshSummary, ProviderSnapshot};
use crate::model_catalog::Catalog;
use crate::model_error::ModelError;
use crate::storage::Storage;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::collections::{HashMap, HashSet};

/// One saved row, as far as a refresh needs it.
struct SavedRow {
    listed_before: bool,
    missing: bool,
}

/// Update only the rows that are already saved, in one transaction.
///
/// # Errors
/// `NotFound` when the provider was deleted, `ModelFetchStale` when its revision or
/// base URL changed, or a storage code. On any error nothing is written.
pub fn refresh_saved_models(
    storage: &Storage,
    snapshot: &ProviderSnapshot,
    catalog: &Catalog,
    now_ms: i64,
) -> Result<ModelRefreshSummary, ModelError> {
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

    let mut saved = HashMap::new();
    {
        let mut statement = transaction
            .prepare(
                "SELECT model_id, last_seen_at, missing_since
                 FROM provider_model WHERE provider_id = ?1",
            )
            .map_err(|_| ModelError::ReadFailed)?;
        let rows = statement
            .query_map([provider_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    SavedRow {
                        listed_before: row.get::<_, Option<i64>>(1)?.is_some(),
                        missing: row.get::<_, Option<i64>>(2)?.is_some(),
                    },
                ))
            })
            .map_err(|_| ModelError::ReadFailed)?;
        for row in rows {
            let (model_id, state) = row.map_err(|_| ModelError::ReadFailed)?;
            saved.insert(model_id, state);
        }
    }

    let complete = catalog.incomplete.is_none();
    let mut summary = ModelRefreshSummary {
        complete,
        incomplete_reason: catalog.incomplete,
        refreshed: 0,
        marked_missing: 0,
        skipped_invalid: catalog.skipped_invalid + catalog.duplicates,
        fetched_at_ms: now_ms,
    };

    let mut listed = HashSet::with_capacity(catalog.models.len());
    for model in &catalog.models {
        if !saved.contains_key(&model.model_id) {
            continue;
        }
        listed.insert(model.model_id.as_str());
        let json = |list: &Option<Vec<String>>| -> Result<Option<String>, ModelError> {
            list.as_ref()
                .map(|items| serde_json::to_string(items).map_err(|_| ModelError::WriteFailed))
                .transpose()
        };
        let input = json(&model.input_modalities)?;
        let output = json(&model.output_modalities)?;
        let endpoints = json(&model.supported_endpoints)?;
        let capabilities =
            serde_json::to_string(&model.capabilities).map_err(|_| ModelError::WriteFailed)?;
        // `source`, `selected` and `alias` are deliberately absent from this
        // statement: a refresh must not change membership or user labels.
        transaction
            .execute(
                "UPDATE provider_model SET
                     upstream_name = ?3, context_window = ?4, max_output_tokens = ?5,
                     input_modalities = ?6, output_modalities = ?7,
                     supported_endpoints = ?8, capabilities = ?9,
                     last_seen_at = ?10, missing_since = NULL,
                     updated_at = max(updated_at, ?10)
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
                    capabilities,
                    now_ms
                ],
            )
            .map_err(|_| ModelError::WriteFailed)?;
        summary.refreshed += 1;
    }

    if complete {
        for (model_id, row) in &saved {
            // Only a row an earlier catalog listed and the current complete one no
            // longer lists becomes upstream-missing. A never-listed manual row is
            // not "missing"; it was never upstream.
            if listed.contains(model_id.as_str()) || row.missing || !row.listed_before {
                continue;
            }
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

    let listed_count = i64::try_from(catalog.models.len()).map_err(|_| ModelError::WriteFailed)?;
    transaction
        .execute(
            "INSERT INTO provider_model_fetch (provider_id, fetched_at, complete, listed_count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (provider_id) DO UPDATE SET
                 fetched_at = excluded.fetched_at,
                 complete = excluded.complete,
                 listed_count = excluded.listed_count",
            params![provider_id, now_ms, complete, listed_count],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(summary)
}
