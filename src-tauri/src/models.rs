//! Saved-model domain facade. Local queries, selection/edit transactions, and
//! cursor validation retain their existing public entrypoints and wire formats.

mod types;
use crate::model_catalog::{RouteSupport, is_valid_model_id, route_support};
use crate::model_search::{SearchQuery, sort_ranked};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol, validate_display_name};
use crate::storage::Storage;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::BTreeSet;
pub use types::{
    AddManualModelRequest, DeleteManualModelRequest, ListProviderModelsRequest,
    MAX_MODEL_PAGE_SIZE, MAX_SELECTION_BATCH, ModelError, ModelSource, ProviderModel,
    ProviderModelPage, SaveProviderModelEntry, SaveProviderModelSelectionsRequest,
    SetModelsSelectedRequest, UpstreamState,
};

/// Longest cursor: a 256-character model ID is at most 1024 UTF-8 bytes.
const MAX_CURSOR_LEN: usize = 2048;

/// The columns read for a `ProviderModel`, in `StoredModel::read` order.
const MODEL_COLUMNS: &str = "provider_id, model_id, source, selected, alias, upstream_name, \
     context_window, max_output_tokens, input_modalities, output_modalities, \
     supported_endpoints, created_at, updated_at, last_seen_at, missing_since";

/// Encode a model ID as a cursor: lowercase hex of its UTF-8 bytes.
pub fn encode_cursor(model_id: &str) -> String {
    model_id.bytes().map(|byte| format!("{byte:02x}")).collect()
}

/// Decode a cursor made by `encode_cursor`. Anything else is `InvalidRequest`.
pub fn decode_cursor(cursor: &str) -> Result<String, ModelError> {
    let well_formed = !cursor.is_empty()
        && cursor.len() <= MAX_CURSOR_LEN
        && cursor.len().is_multiple_of(2)
        && cursor
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !well_formed {
        return Err(ModelError::InvalidRequest);
    }
    let bytes = (0..cursor.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&cursor[index..index + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| ModelError::InvalidRequest)?;
    let model_id = String::from_utf8(bytes).map_err(|_| ModelError::InvalidRequest)?;
    if is_valid_model_id(&model_id) {
        Ok(model_id)
    } else {
        Err(ModelError::InvalidRequest)
    }
}

/// List a provider's models, optionally only selected ones, either ordered by
/// model ID (no query) or ranked by a fuzzy query (see `crate::model_search`).
///
/// # Errors
/// `InvalidRequest` for a bad provider ID, limit, filter, or cursor, a query over
/// `MAX_QUERY_CHARS` characters, or a query together with a cursor; `NotFound`
/// when the provider does not exist; storage and stored-data errors otherwise.
pub fn list_provider_models(
    storage: &Storage,
    request: &ListProviderModelsRequest,
) -> Result<ProviderModelPage, ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    if request.limit == 0 || request.limit > MAX_MODEL_PAGE_SIZE {
        return Err(ModelError::InvalidRequest);
    }
    let only_selected = match request.filter.as_str() {
        "all" => false,
        "selected" => true,
        _ => return Err(ModelError::InvalidRequest),
    };
    let query = match request.query.as_deref() {
        Some(raw) => SearchQuery::parse(raw).map_err(|_| ModelError::InvalidRequest)?,
        None => None,
    };
    if let Some(query) = query {
        if request.after.is_some() {
            return Err(ModelError::InvalidRequest);
        }
        return search_provider_models(storage, &provider_id, only_selected, &query, request.limit);
    }
    let after = request.after.as_deref().map(decode_cursor).transpose()?;

    let connection = lock(storage)?;
    let provider = read_provider(&connection, &provider_id)?;
    // SQLite compares TEXT byte by byte (BINARY collation), the same order as
    // Rust's `str`, so the cursor comparison matches `ORDER BY model_id`.
    let mut statement = connection
        .prepare(&format!(
            "SELECT {MODEL_COLUMNS} FROM provider_model
             WHERE provider_id = ?1
               AND (?2 IS NULL OR model_id > ?2)
               AND (?3 = 0 OR selected = 1)
             ORDER BY model_id
             LIMIT ?4"
        ))
        .map_err(|_| ModelError::ReadFailed)?;
    let rows = statement
        .query_map(
            params![
                provider_id.as_str(),
                after,
                only_selected,
                i64::from(request.limit) + 1
            ],
            StoredModel::read,
        )
        .map_err(|_| ModelError::ReadFailed)?;
    let mut items = Vec::new();
    for row in rows {
        let stored = row.map_err(|_| ModelError::ReadFailed)?;
        items.push(stored.into_model(provider)?);
    }
    let page_size = usize::try_from(request.limit).map_err(|_| ModelError::InvalidRequest)?;
    let next_cursor = if items.len() > page_size {
        items.truncate(page_size);
        items.last().map(|model| encode_cursor(&model.model_id))
    } else {
        None
    };
    Ok(ProviderModelPage {
        items,
        next_cursor,
        total_matches: None,
    })
}

/// Rank every model of the provider (after the selection filter) in memory and
/// return the best `limit`. SQLite only filters; it never sees the query.
fn search_provider_models(
    storage: &Storage,
    provider_id: &ProviderId,
    only_selected: bool,
    query: &SearchQuery,
    limit: u32,
) -> Result<ProviderModelPage, ModelError> {
    let connection = lock(storage)?;
    let provider = read_provider(&connection, provider_id)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {MODEL_COLUMNS} FROM provider_model
             WHERE provider_id = ?1 AND (?2 = 0 OR selected = 1)"
        ))
        .map_err(|_| ModelError::ReadFailed)?;
    let rows = statement
        .query_map(
            params![provider_id.as_str(), only_selected],
            StoredModel::read,
        )
        .map_err(|_| ModelError::ReadFailed)?;
    let mut scored = Vec::new();
    for row in rows {
        let model = row
            .map_err(|_| ModelError::ReadFailed)?
            .into_model(provider)?;
        let fields = [
            Some(model.model_id.as_str()),
            model.upstream_name.as_deref(),
            model.alias.as_deref(),
        ];
        if let Some(score) = query.score(fields.into_iter().flatten()) {
            scored.push((score, model));
        }
    }
    drop(statement);
    drop(connection);
    sort_ranked(&mut scored, |model| model.model_id.as_str());
    let total_matches = scored.len();
    let limit = usize::try_from(limit).map_err(|_| ModelError::InvalidRequest)?;
    let items = scored
        .into_iter()
        .take(limit)
        .map(|(_, model)| model)
        .collect();
    Ok(ProviderModelPage {
        items,
        next_cursor: None,
        total_matches: Some(total_matches),
    })
}

/// Select or unselect models in one transaction: either every listed model
/// changes or none does. Returns the changed models ordered by model ID.
///
/// # Errors
/// `InvalidRequest` for an empty or oversized batch or a malformed ID;
/// `NotFound` for an unknown provider; `ModelNotFound` when any model is missing;
/// `ModelRouteNotSupported` when selecting a fetched model whose routes do not
/// include the instance's protocol.
pub fn set_models_selected(
    storage: &Storage,
    request: &SetModelsSelectedRequest,
    now_ms: i64,
) -> Result<Vec<ProviderModel>, ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    let model_ids: BTreeSet<&str> = request.model_ids.iter().map(String::as_str).collect();
    if model_ids.is_empty()
        || request.model_ids.len() > MAX_SELECTION_BATCH
        || !model_ids.iter().all(|id| is_valid_model_id(id))
    {
        return Err(ModelError::InvalidRequest);
    }

    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;
    let provider = read_provider(&transaction, &provider_id)?;
    let mut changed = Vec::with_capacity(model_ids.len());
    for model_id in model_ids {
        let model = read_model(&transaction, &provider_id, model_id)?
            .ok_or(ModelError::ModelNotFound)?
            .into_model(provider)?;
        let blocked = request.selected
            && model.source == ModelSource::Fetched
            && !matches!(
                model.route_support,
                RouteSupport::Supported | RouteSupport::NotApplicable
            );
        if blocked {
            return Err(ModelError::ModelRouteNotSupported);
        }
        if model.selected != request.selected {
            transaction
                .execute(
                    "UPDATE provider_model SET selected = ?3, updated_at = max(updated_at, ?4)
                     WHERE provider_id = ?1 AND model_id = ?2",
                    params![provider_id.as_str(), model_id, request.selected, now_ms],
                )
                .map_err(|_| ModelError::WriteFailed)?;
        }
        let updated = read_model(&transaction, &provider_id, model_id)?
            .ok_or(ModelError::ModelNotFound)?
            .into_model(provider)?;
        changed.push(updated);
    }
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(changed)
}

/// Add a model by hand. It is saved as `manual` and selected.
///
/// # Errors
/// `ModelIdInvalid`, `ModelAliasInvalid`, `NotFound`, `ModelAlreadyExists`, or a
/// storage error.
pub fn add_manual_model(
    storage: &Storage,
    request: &AddManualModelRequest,
    now_ms: i64,
) -> Result<ProviderModel, ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    let model_id = request.model_id.trim();
    if !is_valid_model_id(model_id) {
        return Err(ModelError::ModelIdInvalid);
    }
    let alias = request
        .alias
        .as_deref()
        .map(|alias| validate_display_name(alias).map_err(|_| ModelError::ModelAliasInvalid))
        .transpose()?;

    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;
    let provider = read_provider(&transaction, &provider_id)?;
    if read_model(&transaction, &provider_id, model_id)?.is_some() {
        return Err(ModelError::ModelAlreadyExists);
    }
    transaction
        .execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, alias, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?5)",
            params![
                provider_id.as_str(),
                model_id,
                ModelSource::Manual.as_str(),
                alias,
                now_ms
            ],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    let model = read_model(&transaction, &provider_id, model_id)?
        .ok_or(ModelError::WriteFailed)?
        .into_model(provider)?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(model)
}

/// Delete a model that was added by hand.
///
/// # Errors
/// `ModelNotFound` when no such model exists; `InvalidRequest` when the model was
/// fetched (fetched rows follow the vendor's list and are never deleted by hand).
pub fn delete_manual_model(
    storage: &Storage,
    request: &DeleteManualModelRequest,
) -> Result<(), ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    if !is_valid_model_id(&request.model_id) {
        return Err(ModelError::InvalidRequest);
    }
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;
    read_provider(&transaction, &provider_id)?;
    let stored = read_model(&transaction, &provider_id, &request.model_id)?
        .ok_or(ModelError::ModelNotFound)?;
    if stored.source != ModelSource::Manual.as_str() {
        return Err(ModelError::InvalidRequest);
    }
    transaction
        .execute(
            "DELETE FROM provider_model WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id.as_str(), request.model_id],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)
}

/// Apply a Save on the Models tab: delete unchecked rows and upsert checked ones.
///
/// Only rows the user keeps selected remain in `provider_model`. Manual rows follow
/// the same rule when the user removes them from the selected list.
///
/// # Errors
/// `InvalidRequest` for malformed IDs, overlapping remove/add sets, or oversized
/// batches; `ModelRouteNotSupported` when an added fetched model does not support
/// the instance protocol; storage errors otherwise.
pub fn save_provider_model_selections(
    storage: &Storage,
    request: &SaveProviderModelSelectionsRequest,
    now_ms: i64,
) -> Result<(), ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    let remove: BTreeSet<&str> = request
        .remove_model_ids
        .iter()
        .map(String::as_str)
        .collect();
    if remove.len() != request.remove_model_ids.len()
        || remove.len() > MAX_SELECTION_BATCH
        || !remove.iter().all(|id| is_valid_model_id(id))
    {
        return Err(ModelError::InvalidRequest);
    }
    if request.add.len() > MAX_SELECTION_BATCH {
        return Err(ModelError::InvalidRequest);
    }
    let mut add_ids = BTreeSet::new();
    for entry in &request.add {
        if !is_valid_model_id(&entry.model_id) || !add_ids.insert(entry.model_id.as_str()) {
            return Err(ModelError::InvalidRequest);
        }
        if remove.contains(entry.model_id.as_str()) {
            return Err(ModelError::InvalidRequest);
        }
    }
    if remove.is_empty() && request.add.is_empty() {
        return Err(ModelError::InvalidRequest);
    }

    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;
    let provider = read_provider(&transaction, &provider_id)?;

    for model_id in remove {
        transaction
            .execute(
                "DELETE FROM provider_model WHERE provider_id = ?1 AND model_id = ?2",
                params![provider_id.as_str(), model_id],
            )
            .map_err(|_| ModelError::WriteFailed)?;
    }

    for entry in &request.add {
        let endpoints = json_column(&entry.supported_endpoints)?;
        let route = route_support(
            provider.kind,
            provider.protocol,
            entry.supported_endpoints.as_deref(),
        );
        if !matches!(route, RouteSupport::Supported | RouteSupport::NotApplicable) {
            return Err(ModelError::ModelRouteNotSupported);
        }
        let input = json_column(&entry.input_modalities)?;
        let output = json_column(&entry.output_modalities)?;
        let existing = read_model(&transaction, &provider_id, &entry.model_id)?;
        if let Some(stored) = existing {
            if stored.source == ModelSource::Manual.as_str() {
                transaction
                    .execute(
                        "UPDATE provider_model SET
                             selected = 1, upstream_name = ?3, context_window = ?4,
                             max_output_tokens = ?5, input_modalities = ?6,
                             output_modalities = ?7, supported_endpoints = ?8,
                             last_seen_at = ?9, missing_since = NULL,
                             updated_at = max(updated_at, ?9)
                         WHERE provider_id = ?1 AND model_id = ?2",
                        params![
                            provider_id.as_str(),
                            entry.model_id,
                            entry.upstream_name,
                            entry.context_window,
                            entry.max_output_tokens,
                            input,
                            output,
                            endpoints,
                            now_ms
                        ],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
            } else {
                transaction
                    .execute(
                        "UPDATE provider_model SET
                             selected = 1, upstream_name = ?3, context_window = ?4,
                             max_output_tokens = ?5, input_modalities = ?6,
                             output_modalities = ?7, supported_endpoints = ?8,
                             last_seen_at = ?9, missing_since = NULL,
                             updated_at = max(updated_at, ?9)
                         WHERE provider_id = ?1 AND model_id = ?2",
                        params![
                            provider_id.as_str(),
                            entry.model_id,
                            entry.upstream_name,
                            entry.context_window,
                            entry.max_output_tokens,
                            input,
                            output,
                            endpoints,
                            now_ms
                        ],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
            }
        } else {
            transaction
                .execute(
                    "INSERT INTO provider_model
                         (provider_id, model_id, source, selected, upstream_name, context_window,
                          max_output_tokens, input_modalities, output_modalities,
                          supported_endpoints, created_at, updated_at, last_seen_at)
                     VALUES (?1, ?2, 'fetched', 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?9)",
                    params![
                        provider_id.as_str(),
                        entry.model_id,
                        entry.upstream_name,
                        entry.context_window,
                        entry.max_output_tokens,
                        input,
                        output,
                        endpoints,
                        now_ms
                    ],
                )
                .map_err(|_| ModelError::WriteFailed)?;
        }
    }

    transaction.commit().map_err(|_| ModelError::WriteFailed)
}

fn json_column(items: &Option<Vec<String>>) -> Result<Option<String>, ModelError> {
    items
        .as_ref()
        .map(|values| serde_json::to_string(values).map_err(|_| ModelError::WriteFailed))
        .transpose()
}

/// The provider fields that decide how its models are shown.
#[derive(Debug, Clone, Copy)]
struct ProviderContext {
    kind: ProviderKind,
    protocol: ProviderProtocol,
}

pub(crate) fn parse_provider_id(value: &str) -> Result<ProviderId, ModelError> {
    ProviderId::parse(value).ok_or(ModelError::InvalidRequest)
}

pub(crate) fn lock(storage: &Storage) -> Result<std::sync::MutexGuard<'_, Connection>, ModelError> {
    storage.lock().map_err(|_| ModelError::StorageUnavailable)
}

fn read_provider(
    connection: &Connection,
    provider_id: &ProviderId,
) -> Result<ProviderContext, ModelError> {
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT kind, protocol FROM provider_instance WHERE id = ?1",
            [provider_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    let (kind, protocol) = row.ok_or(ModelError::NotFound)?;
    Ok(ProviderContext {
        kind: ProviderKind::from_value(&kind).ok_or(ModelError::InvalidStoredProvider)?,
        protocol: ProviderProtocol::from_value(&protocol)
            .ok_or(ModelError::InvalidStoredProvider)?,
    })
}

fn read_model(
    connection: &Connection,
    provider_id: &ProviderId,
    model_id: &str,
) -> Result<Option<StoredModel>, ModelError> {
    connection
        .query_row(
            &format!(
                "SELECT {MODEL_COLUMNS} FROM provider_model
                 WHERE provider_id = ?1 AND model_id = ?2"
            ),
            params![provider_id.as_str(), model_id],
            StoredModel::read,
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)
}

/// A row exactly as stored, before validation.
struct StoredModel {
    provider_id: String,
    model_id: String,
    source: String,
    selected: i64,
    alias: Option<String>,
    upstream_name: Option<String>,
    context_window: Option<i64>,
    max_output_tokens: Option<i64>,
    input_modalities: Option<String>,
    output_modalities: Option<String>,
    supported_endpoints: Option<String>,
    created_at: i64,
    updated_at: i64,
    last_seen_at: Option<i64>,
    missing_since: Option<i64>,
}

impl StoredModel {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            provider_id: row.get(0)?,
            model_id: row.get(1)?,
            source: row.get(2)?,
            selected: row.get(3)?,
            alias: row.get(4)?,
            upstream_name: row.get(5)?,
            context_window: row.get(6)?,
            max_output_tokens: row.get(7)?,
            input_modalities: row.get(8)?,
            output_modalities: row.get(9)?,
            supported_endpoints: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
            last_seen_at: row.get(13)?,
            missing_since: row.get(14)?,
        })
    }

    /// Validate the row and add the values derived from the provider.
    fn into_model(self, provider: ProviderContext) -> Result<ProviderModel, ModelError> {
        let invalid = ModelError::InvalidStoredModel;
        let source = ModelSource::from_value(&self.source).ok_or(invalid)?;
        let selected = match self.selected {
            0 => false,
            1 => true,
            _ => return Err(invalid),
        };
        if !is_valid_model_id(&self.model_id) {
            return Err(invalid);
        }
        let supported_endpoints = json_list(self.supported_endpoints)?;
        let route_support = route_support(
            provider.kind,
            provider.protocol,
            supported_endpoints.as_deref(),
        );
        let upstream_state = match (self.last_seen_at, self.missing_since) {
            (None, _) => UpstreamState::NeverListed,
            (Some(_), None) => UpstreamState::Listed,
            (Some(_), Some(_)) => UpstreamState::Missing,
        };
        Ok(ProviderModel {
            provider_id: self.provider_id,
            model_id: self.model_id,
            source,
            selected,
            alias: self.alias,
            upstream_name: self.upstream_name,
            context_window: self.context_window,
            max_output_tokens: self.max_output_tokens,
            input_modalities: json_list(self.input_modalities)?,
            output_modalities: json_list(self.output_modalities)?,
            supported_endpoints,
            route_support,
            upstream_state,
            last_seen_at_ms: self.last_seen_at,
            missing_since_ms: self.missing_since,
            created_at_ms: self.created_at,
            updated_at_ms: self.updated_at,
        })
    }
}

/// Read a JSON string array column.
fn json_list(value: Option<String>) -> Result<Option<Vec<String>>, ModelError> {
    value
        .map(|text| serde_json::from_str(&text).map_err(|_| ModelError::InvalidStoredModel))
        .transpose()
}

#[cfg(test)]
mod tests;
