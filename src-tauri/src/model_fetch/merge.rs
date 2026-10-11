//! Catalog merge transaction: recheck the provider snapshot, retain manual/selected
//! rows, update metadata, prune only a complete catalog, and persist the summary.

use super::repository::lock;
use super::types::{ModelFetchSummary, ProviderSnapshot};
use crate::model_catalog::Catalog;
use crate::models::ModelError;
use crate::storage::Storage;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::collections::{HashMap, HashSet};

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
