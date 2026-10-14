//! Local model queries, ranked searches, and validated SQLite row conversion.
//! Internal transaction readers are shared only with local edit operations.

use super::types::*;
use super::validation::{decode_cursor, encode_cursor};
use super::{ModelError, parse_provider_id};
use crate::model_catalog::{
    CapabilityOverrides, ThinkingEffort, accepted_limit, decode_thinking_effort,
    encode_thinking_effort,
};
use crate::model_identity::{is_valid_model_id, resolved_protocol, route_support};
use crate::model_search::{SearchQuery, sort_ranked};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol};
use crate::storage::Storage;
use rusqlite::{Connection, OptionalExtension, params};

/// The columns read for a `ProviderModel`, in `StoredModel::read` order.
const MODEL_COLUMNS: &str = "provider_id, model_id, source, selected, alias, upstream_name, \
     context_window, max_output_tokens, input_modalities, output_modalities, \
     supported_endpoints, capabilities, capability_overrides, thinking_effort, \
     created_at, updated_at, \
     last_seen_at, missing_since";

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

/// Count models the user has added (勾选 / `selected = 1`) across all providers.
/// Unselected catalog-fetch cache rows are excluded.
///
/// # Errors
/// `StorageUnavailable` when the database lock fails; `ReadFailed` when SQLite
/// cannot run the count.
pub fn count_provider_models(storage: &Storage) -> Result<ProviderModelCount, ModelError> {
    let connection = lock(storage)?;
    let total: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM provider_model WHERE selected = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ModelError::ReadFailed)?;
    Ok(ProviderModelCount { total })
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

/// The provider fields that decide how its models are shown.
#[derive(Debug, Clone, Copy)]
pub(super) struct ProviderContext {
    pub(super) kind: ProviderKind,
    pub(super) protocol: ProviderProtocol,
}

/// Lock local model storage while preserving startup and poison error handling.
pub(super) fn lock(storage: &Storage) -> Result<std::sync::MutexGuard<'_, Connection>, ModelError> {
    storage.lock().map_err(|_| ModelError::StorageUnavailable)
}

/// Read the kind/protocol needed to validate local model rows and selections.
/// Missing or unsupported provider data returns the existing model error codes.
pub(super) fn read_provider(
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

/// Read one optional raw row inside the caller's connection or transaction.
pub(super) fn read_model(
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
/// Raw SQLite fields, validated before they become a public ProviderModel.
pub(super) struct StoredModel {
    provider_id: String,
    model_id: String,
    pub(super) source: String,
    selected: i64,
    alias: Option<String>,
    upstream_name: Option<String>,
    context_window: Option<i64>,
    max_output_tokens: Option<i64>,
    input_modalities: Option<String>,
    output_modalities: Option<String>,
    supported_endpoints: Option<String>,
    capabilities: String,
    capability_overrides: Option<String>,
    thinking_effort: Option<String>,
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
            capabilities: row.get(11)?,
            capability_overrides: row.get(12)?,
            thinking_effort: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            last_seen_at: row.get(16)?,
            missing_since: row.get(17)?,
        })
    }

    /// Validate the row and add the values derived from the provider.
    pub(super) fn into_model(self, provider: ProviderContext) -> Result<ProviderModel, ModelError> {
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
        let endpoints = supported_endpoints.as_deref();
        // Capability states and sources must stay paired: an unknown state may not
        // claim a source, and a definitive state must name one.
        let capabilities: crate::model_catalog::ModelCapabilities =
            serde_json::from_str(&self.capabilities).map_err(|_| ModelError::InvalidStoredModel)?;
        if !capabilities.has_valid_sources() {
            return Err(invalid);
        }
        // `NULL` means no user declarations. A stored JSON value must parse into
        // the strict override shape; anything else invalidates the whole row.
        let capability_overrides = self
            .capability_overrides
            .map(|text| {
                serde_json::from_str::<CapabilityOverrides>(&text)
                    .map_err(|_| ModelError::InvalidStoredModel)
            })
            .transpose()?
            .unwrap_or_default();
        let thinking_effort = decode_thinking_effort(self.thinking_effort.as_deref())
            .ok_or(ModelError::InvalidStoredModel)?;
        let route_support = route_support(provider.kind, endpoints);
        let resolved_protocol = resolved_protocol(provider.kind, provider.protocol, endpoints);
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
            capabilities,
            capability_overrides,
            thinking_effort,
            route_support,
            resolved_protocol,
            upstream_state,
            last_seen_at_ms: self.last_seen_at,
            missing_since_ms: self.missing_since,
            created_at_ms: self.created_at,
            updated_at_ms: self.updated_at,
        })
    }
}

/// Limits and effort text ready to bind in a manual save.
pub(super) struct PreparedParameters {
    pub(super) context_window: Option<i64>,
    pub(super) max_output_tokens: Option<i64>,
    pub(super) thinking_effort: Option<String>,
}

/// Check limits and canonicalize a thinking-effort statement for a manual save.
pub(super) fn prepare_model_parameters(
    context_window: Option<i64>,
    max_output_tokens: Option<i64>,
    thinking_effort: Option<ThinkingEffort>,
) -> Result<PreparedParameters, ModelError> {
    let context_window =
        accepted_limit(context_window).ok_or(ModelError::ModelParametersInvalid)?;
    let max_output_tokens =
        accepted_limit(max_output_tokens).ok_or(ModelError::ModelParametersInvalid)?;
    let thinking_effort = match thinking_effort {
        None => None,
        Some(effort) => Some(
            effort
                .canonical()
                .ok_or(ModelError::ModelParametersInvalid)?,
        ),
    };
    let thinking_effort =
        encode_thinking_effort(thinking_effort.as_ref()).map_err(|_| ModelError::WriteFailed)?;
    Ok(PreparedParameters {
        context_window,
        max_output_tokens,
        thinking_effort,
    })
}

/// Serialize capability declarations for storage: an empty set becomes NULL,
/// the same "no declarations" form that fresh and migrated rows use.
pub(super) fn stored_overrides(
    overrides: &CapabilityOverrides,
) -> Result<Option<String>, ModelError> {
    if overrides.is_empty() {
        return Ok(None);
    }
    let json = serde_json::to_string(overrides).map_err(|_| ModelError::WriteFailed)?;
    Ok(Some(json))
}

/// Read a JSON string array column.
fn json_list(value: Option<String>) -> Result<Option<Vec<String>>, ModelError> {
    value
        .map(|text| serde_json::from_str(&text).map_err(|_| ModelError::InvalidStoredModel))
        .transpose()
}
