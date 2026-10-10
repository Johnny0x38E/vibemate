//! Models of a provider instance: the vendor's fetched list plus models the user
//! added by hand, each with the user's "selected" tick (P12).
//!
//! Rows live in the `provider_model` table (schema version 7). This module reads
//! and writes them; it sends no network requests and never sees an API key.
//! Merging a fetched list comes in P12.a.4 and ranked search in P12.a.5.
//!
//! Rules kept here:
//! - A model is identified by (provider ID, model ID). The model ID is the vendor's
//!   exact text and may contain `/`, `.` or `:`.
//! - Only the user changes `selected`. A fetched Command Code model can be selected
//!   only when it lists the route of the instance's protocol (decision D9).
//! - Only manual rows can be deleted; fetched rows follow the vendor's list.
//! - Lists use keyset pagination ordered by model ID. The cursor is the last model
//!   ID as lowercase hex of its UTF-8 bytes, so IDs with `/`, `.` or `:` cannot be
//!   confused with a separator.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::credentials::CredentialError;
use crate::http_client::HttpError;
use crate::model_catalog::{CatalogError, RouteSupport, is_valid_model_id, route_support};
use crate::model_search::{SearchQuery, sort_ranked};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol, validate_display_name};
use crate::storage::Storage;

/// Largest page of models.
pub const MAX_MODEL_PAGE_SIZE: u32 = 200;

/// Most model IDs accepted by one selection change.
pub const MAX_SELECTION_BATCH: usize = 500;

/// Longest cursor: a 256-character model ID is at most 1024 UTF-8 bytes.
const MAX_CURSOR_LEN: usize = 2048;

/// Why a model operation failed. Codes carry no payload, so no model text, path,
/// or SQL ever crosses the IPC boundary through an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelError {
    /// Storage failed at startup or after a panic; nothing was read or written.
    StorageUnavailable,
    /// SQLite could not read model data.
    ReadFailed,
    /// SQLite rejected the write; the committed data is unchanged.
    WriteFailed,
    /// The request is malformed (provider ID, cursor, page size, filter, or batch).
    InvalidRequest,
    /// No provider instance has the requested ID.
    NotFound,
    /// The provider row holds a kind or protocol this build does not accept.
    InvalidStoredProvider,
    /// A stored model row holds a value this build does not accept.
    InvalidStoredModel,
    /// A manual model ID is empty, too long, or has whitespace or control characters.
    ModelIdInvalid,
    /// The alias is empty after trimming, too long, or has control characters.
    ModelAliasInvalid,
    /// The provider already has a model with this ID.
    ModelAlreadyExists,
    /// A requested model does not exist (the list may have changed).
    ModelNotFound,
    /// A fetched model does not support the instance's protocol, so it cannot be selected.
    ModelRouteNotSupported,
    /// The operation could not finish (for example the HTTP client could not be built).
    OperationFailed,
    /// The provider has no API key yet, so its model list is not requested.
    SecretMissing,
    /// The stored key cannot be sent as an HTTP header value.
    SecretInvalid,
    /// The stored base URL cannot be requested by this client.
    BaseUrlInvalid,
    /// This device has no usable OS credential store.
    CredentialStoreUnavailable,
    /// The OS credential store is locked or refused access.
    CredentialStoreAccessDenied,
    /// The OS credential store reported another error.
    CredentialStoreFailed,
    /// A fetch for this provider is already running.
    ModelFetchInProgress,
    /// The user cancelled the fetch; nothing was saved.
    ModelFetchCancelled,
    /// The provider's settings changed during the fetch; the result was not saved.
    ModelFetchStale,
    /// DNS, TCP, or proxy connection failure.
    ConnectionFailed,
    /// The TLS handshake or certificate check failed.
    TlsFailed,
    /// A request or the whole fetch took too long.
    RequestTimedOut,
    /// HTTP 401 or 403. Its absence does not prove the key is valid.
    AuthRejected,
    /// HTTP 402.
    InsufficientBalance,
    /// HTTP 429.
    RateLimited,
    /// HTTP 5xx.
    UpstreamUnavailable,
    /// The vendor's response is not the documented model list, or an unexpected status.
    UpstreamResponseInvalid,
    /// The response body exceeded the size cap.
    ResponseTooLarge,
}

impl ModelError {
    /// The stable snake_case code used over IPC.
    pub fn code(self) -> &'static str {
        match self {
            Self::StorageUnavailable => "storage_unavailable",
            Self::ReadFailed => "read_failed",
            Self::WriteFailed => "write_failed",
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::InvalidStoredProvider => "invalid_stored_provider",
            Self::InvalidStoredModel => "invalid_stored_model",
            Self::ModelIdInvalid => "model_id_invalid",
            Self::ModelAliasInvalid => "model_alias_invalid",
            Self::ModelAlreadyExists => "model_already_exists",
            Self::ModelNotFound => "model_not_found",
            Self::ModelRouteNotSupported => "model_route_not_supported",
            Self::OperationFailed => "operation_failed",
            Self::SecretMissing => "secret_missing",
            Self::SecretInvalid => "secret_invalid",
            Self::BaseUrlInvalid => "base_url_invalid",
            Self::CredentialStoreUnavailable => "credential_store_unavailable",
            Self::CredentialStoreAccessDenied => "credential_store_access_denied",
            Self::CredentialStoreFailed => "credential_store_failed",
            Self::ModelFetchInProgress => "model_fetch_in_progress",
            Self::ModelFetchCancelled => "model_fetch_cancelled",
            Self::ModelFetchStale => "model_fetch_stale",
            Self::ConnectionFailed => "connection_failed",
            Self::TlsFailed => "tls_failed",
            Self::RequestTimedOut => "request_timed_out",
            Self::AuthRejected => "auth_rejected",
            Self::InsufficientBalance => "insufficient_balance",
            Self::RateLimited => "rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::UpstreamResponseInvalid => "upstream_response_invalid",
            Self::ResponseTooLarge => "response_too_large",
        }
    }
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ModelError {}

impl From<HttpError> for ModelError {
    /// Every network failure keeps the code `HttpError::code` gives it.
    fn from(error: HttpError) -> Self {
        match error {
            HttpError::ClientUnavailable => Self::OperationFailed,
            HttpError::InvalidUrl => Self::BaseUrlInvalid,
            HttpError::SecretInvalid => Self::SecretInvalid,
            HttpError::ConnectionFailed => Self::ConnectionFailed,
            HttpError::TlsFailed => Self::TlsFailed,
            HttpError::RequestTimedOut => Self::RequestTimedOut,
            HttpError::AuthRejected => Self::AuthRejected,
            HttpError::InsufficientBalance => Self::InsufficientBalance,
            HttpError::RateLimited => Self::RateLimited,
            HttpError::UpstreamUnavailable => Self::UpstreamUnavailable,
            HttpError::UpstreamResponseInvalid => Self::UpstreamResponseInvalid,
            HttpError::ResponseTooLarge => Self::ResponseTooLarge,
            HttpError::Cancelled => Self::ModelFetchCancelled,
        }
    }
}

impl From<CatalogError> for ModelError {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::UpstreamResponseInvalid => Self::UpstreamResponseInvalid,
            CatalogError::InvalidStoredProvider => Self::InvalidStoredProvider,
        }
    }
}

impl From<CredentialError> for ModelError {
    /// The same mapping as `ProviderError`, so the frontend reuses its messages.
    fn from(error: CredentialError) -> Self {
        match error {
            CredentialError::Unavailable => Self::CredentialStoreUnavailable,
            CredentialError::AccessDenied => Self::CredentialStoreAccessDenied,
            CredentialError::OperationFailed
            | CredentialError::InvalidReference
            | CredentialError::CorruptValue => Self::CredentialStoreFailed,
        }
    }
}

/// Where a model row came from first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    Fetched,
    Manual,
}

impl ModelSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fetched => "fetched",
            Self::Manual => "manual",
        }
    }

    fn from_value(value: &str) -> Option<Self> {
        match value {
            "fetched" => Some(Self::Fetched),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Whether the vendor's latest complete list contained the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamState {
    /// The latest complete fetch listed it.
    Listed,
    /// An earlier fetch listed it, the latest complete fetch did not.
    Missing,
    /// No fetch has listed it (only manual rows).
    NeverListed,
}

/// One model as shown to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub provider_id: String,
    pub model_id: String,
    pub source: ModelSource,
    pub selected: bool,
    pub alias: Option<String>,
    pub upstream_name: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub supported_endpoints: Option<Vec<String>>,
    pub route_support: RouteSupport,
    pub upstream_state: UpstreamState,
    pub last_seen_at_ms: Option<i64>,
    pub missing_since_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// A page request. `filter` is `"all"` or `"selected"`.
///
/// Without a `query` (or one that is empty after normalization) the list is
/// ordered by model ID and paged with `after`. With a query the best `limit`
/// matches are returned in rank order, with no cursor; `after` must then be `None`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListProviderModelsRequest {
    pub provider_id: String,
    /// The `next_cursor` of the previous page, or `None` for the first page.
    pub after: Option<String>,
    /// Page size (or, with a query, result count) from 1 to `MAX_MODEL_PAGE_SIZE`.
    pub limit: u32,
    pub filter: String,
    /// Fuzzy search text, at most `MAX_QUERY_CHARS` characters.
    #[serde(default)]
    pub query: Option<String>,
}

/// One page of models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPage {
    pub items: Vec<ProviderModel>,
    /// Cursor for the next page; always `None` for a search.
    pub next_cursor: Option<String>,
    /// For a search, how many models matched (`items` holds at most `limit`);
    /// `None` without a query.
    pub total_matches: Option<usize>,
}

/// Select or unselect several models at once.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetModelsSelectedRequest {
    pub provider_id: String,
    pub model_ids: Vec<String>,
    pub selected: bool,
}

/// Add a model by hand.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddManualModelRequest {
    pub provider_id: String,
    pub model_id: String,
    pub alias: Option<String>,
}

/// Delete a model that was added by hand.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteManualModelRequest {
    pub provider_id: String,
    pub model_id: String,
}

/// Upstream metadata for a model the user wants to keep selected (P12.c.5).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveProviderModelEntry {
    pub model_id: String,
    pub upstream_name: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub supported_endpoints: Option<Vec<String>>,
}

/// Atomically remove deselected models and upsert newly selected ones.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveProviderModelSelectionsRequest {
    pub provider_id: String,
    pub remove_model_ids: Vec<String>,
    pub add: Vec<SaveProviderModelEntry>,
}

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
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A unique temporary app-data folder, removed when the test ends.
    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("vibemate-models-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn open(&self) -> Storage {
            Storage::open_in_directory(&self.0).expect("open storage")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const PROVIDER: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "fedcba9876543210fedcba9876543210";

    /// Insert a provider row directly, so tests choose kind and protocol freely.
    fn insert_provider(storage: &Storage, id: &str, kind: &str, protocol: &str) {
        storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO provider_instance
                     (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
                 VALUES (?1, ?2, 'Test', 'https://example.com', ?3, 1, 1, 1)",
                params![id, kind, protocol],
            )
            .unwrap();
    }

    /// Insert a fetched model row as a later merge (P12.a.4) would.
    fn insert_fetched(storage: &Storage, provider: &str, model_id: &str, endpoints: Option<&str>) {
        storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO provider_model
                     (provider_id, model_id, source, selected, upstream_name, context_window,
                      supported_endpoints, created_at, updated_at, last_seen_at)
                 VALUES (?1, ?2, 'fetched', 0, 'Upstream', 1000, ?3, 5, 5, 5)",
                params![provider, model_id, endpoints],
            )
            .unwrap();
    }

    fn list(
        storage: &Storage,
        after: Option<String>,
        limit: u32,
        filter: &str,
    ) -> ProviderModelPage {
        list_provider_models(
            storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after,
                limit,
                filter: filter.to_owned(),
                query: None,
            },
        )
        .expect("list")
    }

    fn add(
        storage: &Storage,
        model_id: &str,
        alias: Option<&str>,
    ) -> Result<ProviderModel, ModelError> {
        add_manual_model(
            storage,
            &AddManualModelRequest {
                provider_id: PROVIDER.to_owned(),
                model_id: model_id.to_owned(),
                alias: alias.map(str::to_owned),
            },
            100,
        )
    }

    fn select(
        storage: &Storage,
        ids: &[&str],
        selected: bool,
    ) -> Result<Vec<ProviderModel>, ModelError> {
        set_models_selected(
            storage,
            &SetModelsSelectedRequest {
                provider_id: PROVIDER.to_owned(),
                model_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
                selected,
            },
            200,
        )
    }

    #[test]
    fn save_selections_deletes_removed_rows_and_inserts_new_ones() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        insert_fetched(&storage, PROVIDER, "keep-me", None);
        select(&storage, &["keep-me"], true).expect("select");
        insert_fetched(&storage, PROVIDER, "drop-me", None);
        select(&storage, &["drop-me"], true).expect("select");

        save_provider_model_selections(
            &storage,
            &SaveProviderModelSelectionsRequest {
                provider_id: PROVIDER.to_owned(),
                remove_model_ids: vec!["drop-me".to_owned()],
                add: vec![SaveProviderModelEntry {
                    model_id: "new-model".to_owned(),
                    upstream_name: Some("New".to_owned()),
                    context_window: Some(1000),
                    max_output_tokens: None,
                    input_modalities: None,
                    output_modalities: None,
                    supported_endpoints: None,
                }],
            },
            300,
        )
        .expect("save");

        let page = list(&storage, None, 50, "selected");
        let ids: Vec<_> = page.items.iter().map(|m| m.model_id.as_str()).collect();
        assert_eq!(ids, ["keep-me", "new-model"]);
    }

    #[test]
    fn cursor_round_trips_ids_with_separators() {
        for id in [
            "mistral/mistral-large-4",
            "ling-3.1-flash:free",
            "模型/甲",
            "a",
        ] {
            let cursor = encode_cursor(id);
            assert!(cursor.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(decode_cursor(&cursor).as_deref(), Ok(id));
        }
        assert_eq!(encode_cursor("a/b"), "612f62");
    }

    #[test]
    fn malformed_cursors_are_rejected() {
        let too_long = "61".repeat(MAX_CURSOR_LEN);
        for cursor in [
            "",
            "6",
            "zz",
            "612F62", // uppercase is not what `encode_cursor` makes
            "ff",     // not UTF-8
            "6120",   // "a " contains whitespace
            "07",     // control character
            too_long.as_str(),
            "1722.abc", // the provider cursor format
        ] {
            assert_eq!(
                decode_cursor(cursor),
                Err(ModelError::InvalidRequest),
                "{cursor}"
            );
        }
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        let result = list_provider_models(
            &storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: Some("not-a-cursor".to_owned()),
                limit: 10,
                filter: "all".to_owned(),
                query: None,
            },
        );
        assert_eq!(result, Err(ModelError::InvalidRequest));
    }

    #[test]
    fn pages_are_ordered_by_model_id_and_cross_ids_with_separators() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
        insert_provider(&storage, OTHER, "openrouter", "chat_completions");
        let ids = [
            "a.b",
            "a/b",
            "a:b",
            "a",
            "ab",
            "mistral/mistral-large-4",
            "openai/gpt-4",
            "z",
        ];
        for id in ids {
            insert_fetched(&storage, PROVIDER, id, None);
        }
        insert_fetched(&storage, OTHER, "only-other", None);

        let mut seen = Vec::new();
        let mut cursor = None;
        let mut pages = 0;
        loop {
            let page = list(&storage, cursor, 3, "all");
            pages += 1;
            seen.extend(page.items.iter().map(|model| model.model_id.clone()));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let mut expected: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
        expected.sort(); // byte order, the same as SQLite's BINARY collation
        assert_eq!(seen, expected);
        assert_eq!(pages, 3);

        // A row inserted before the cursor is not repeated; one after it appears.
        let first = list(&storage, None, 3, "all");
        insert_fetched(&storage, PROVIDER, "0-early", None);
        insert_fetched(&storage, PROVIDER, "zz-late", None);
        let mut rest = Vec::new();
        let mut cursor = first.next_cursor;
        while let Some(next) = cursor {
            let page = list(&storage, Some(next), 3, "all");
            rest.extend(page.items.into_iter().map(|model| model.model_id));
            cursor = page.next_cursor;
        }
        assert!(!rest.contains(&"0-early".to_owned()));
        assert_eq!(rest.last().map(String::as_str), Some("zz-late"));
    }

    #[test]
    fn list_validates_the_request_and_the_provider() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        let request = |provider_id: &str, limit: u32, filter: &str| ListProviderModelsRequest {
            provider_id: provider_id.to_owned(),
            after: None,
            limit,
            filter: filter.to_owned(),
            query: None,
        };
        for (bad, expected) in [
            (request("nope", 10, "all"), ModelError::InvalidRequest),
            (request(PROVIDER, 0, "all"), ModelError::InvalidRequest),
            (
                request(PROVIDER, MAX_MODEL_PAGE_SIZE + 1, "all"),
                ModelError::InvalidRequest,
            ),
            (request(PROVIDER, 10, "ticked"), ModelError::InvalidRequest),
            (request(OTHER, 10, "all"), ModelError::NotFound),
        ] {
            assert_eq!(list_provider_models(&storage, &bad), Err(expected));
        }
        let empty = list(&storage, None, MAX_MODEL_PAGE_SIZE, "all");
        assert!(empty.items.is_empty());
        assert_eq!(empty.next_cursor, None);
    }

    #[test]
    fn manual_models_are_validated_selected_and_unique() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "command-code", "chat_completions");

        let added = add(&storage, "  mistral/mistral-large-4  ", Some("  Large  ")).unwrap();
        assert_eq!(added.model_id, "mistral/mistral-large-4");
        assert_eq!(added.alias.as_deref(), Some("Large"));
        assert_eq!(added.source, ModelSource::Manual);
        assert!(added.selected);
        assert_eq!(added.upstream_state, UpstreamState::NeverListed);
        // A manual Command Code model has no routes: selectable, but unverified.
        assert_eq!(added.route_support, RouteSupport::Unknown);
        assert_eq!((added.created_at_ms, added.updated_at_ms), (100, 100));

        assert_eq!(
            add(&storage, "mistral/mistral-large-4", None),
            Err(ModelError::ModelAlreadyExists)
        );
        // IDs are case-sensitive.
        assert!(add(&storage, "Mistral/Mistral-Large-4", None).is_ok());
        for invalid in [
            "",
            "   ",
            "has space",
            "tab\tin",
            "zero\u{200b}width",
            &"m".repeat(257),
        ] {
            assert_eq!(
                add(&storage, invalid, None),
                Err(ModelError::ModelIdInvalid),
                "{invalid:?}"
            );
        }
        for alias in ["   ", "line\nbreak", &"a".repeat(65)] {
            assert_eq!(
                add(&storage, "fresh", Some(alias)),
                Err(ModelError::ModelAliasInvalid)
            );
        }
        let missing_provider = add_manual_model(
            &storage,
            &AddManualModelRequest {
                provider_id: OTHER.to_owned(),
                model_id: "x".to_owned(),
                alias: None,
            },
            100,
        );
        assert_eq!(missing_provider, Err(ModelError::NotFound));
    }

    #[test]
    fn selection_is_all_or_nothing_and_respects_routes() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "command-code", "chat_completions");
        insert_fetched(
            &storage,
            PROVIDER,
            "deepseek/deepseek-v4-flash",
            Some(r#"["/chat/completions","/responses"]"#),
        );
        insert_fetched(
            &storage,
            PROVIDER,
            "claude-sonnet-5",
            Some(r#"["/messages"]"#),
        );
        insert_fetched(&storage, PROVIDER, "no-routes", None);
        add(&storage, "manual-model", None).unwrap();

        let changed = select(&storage, &["deepseek/deepseek-v4-flash"], true).unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed[0].selected);
        assert_eq!(changed[0].route_support, RouteSupport::Supported);
        assert_eq!(changed[0].updated_at_ms, 200);

        // Fetched models that do not list the protocol's route cannot be selected,
        // and the whole batch is rolled back.
        assert_eq!(
            select(&storage, &["manual-model", "claude-sonnet-5"], false)
                .map(|models| models.len()),
            Ok(2)
        );
        assert_eq!(
            select(&storage, &["manual-model", "claude-sonnet-5"], true),
            Err(ModelError::ModelRouteNotSupported)
        );
        assert_eq!(
            select(&storage, &["no-routes"], true),
            Err(ModelError::ModelRouteNotSupported)
        );
        let selected = list(&storage, None, 10, "selected");
        assert_eq!(
            selected
                .items
                .iter()
                .map(|model| model.model_id.as_str())
                .collect::<Vec<_>>(),
            ["deepseek/deepseek-v4-flash"],
            "the failed batch left manual-model unselected"
        );

        // Unselecting is always allowed; an unknown ID fails the whole batch.
        assert_eq!(
            select(&storage, &["deepseek/deepseek-v4-flash", "gone"], false),
            Err(ModelError::ModelNotFound)
        );
        assert_eq!(list(&storage, None, 10, "selected").items.len(), 1);
        assert_eq!(select(&storage, &[], true), Err(ModelError::InvalidRequest));
        let too_many: Vec<String> = (0..=MAX_SELECTION_BATCH).map(|i| format!("m{i}")).collect();
        let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();
        assert_eq!(
            select(&storage, &too_many, true),
            Err(ModelError::InvalidRequest)
        );
    }

    #[test]
    fn other_vendors_do_not_use_routes() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
        insert_fetched(&storage, PROVIDER, "openai/gpt-4", None);
        let changed = select(&storage, &["openai/gpt-4"], true).unwrap();
        assert_eq!(changed[0].route_support, RouteSupport::NotApplicable);
        assert_eq!(changed[0].upstream_state, UpstreamState::Listed);
        assert_eq!(changed[0].upstream_name.as_deref(), Some("Upstream"));
        assert_eq!(changed[0].context_window, Some(1000));
    }

    #[test]
    fn only_manual_models_can_be_deleted() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        insert_fetched(&storage, PROVIDER, "deepseek-flash", None);
        add(&storage, "my/model", None).unwrap();
        let delete = |model_id: &str| {
            delete_manual_model(
                &storage,
                &DeleteManualModelRequest {
                    provider_id: PROVIDER.to_owned(),
                    model_id: model_id.to_owned(),
                },
            )
        };
        assert_eq!(delete("deepseek-flash"), Err(ModelError::InvalidRequest));
        assert_eq!(delete("missing"), Err(ModelError::ModelNotFound));
        assert_eq!(delete("bad id"), Err(ModelError::InvalidRequest));
        assert_eq!(delete("my/model"), Ok(()));
        let remaining: Vec<String> = list(&storage, None, 10, "all")
            .items
            .into_iter()
            .map(|model| model.model_id)
            .collect();
        assert_eq!(remaining, ["deepseek-flash"]);
    }

    #[test]
    fn corrupt_stored_rows_are_reported_not_guessed() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        // Valid JSON, but not a string array.
        storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO provider_model
                     (provider_id, model_id, source, selected, input_modalities, created_at, updated_at)
                 VALUES (?1, 'odd', 'fetched', 0, '{\"a\":1}', 1, 1)",
                [PROVIDER],
            )
            .unwrap();
        let result = list_provider_models(
            &storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: None,
                limit: 10,
                filter: "all".to_owned(),
                query: None,
            },
        );
        assert_eq!(result, Err(ModelError::InvalidStoredModel));
    }

    /// Insert a fetched row with a chosen upstream name and selection.
    fn insert_named(storage: &Storage, model_id: &str, name: Option<&str>, selected: bool) {
        storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO provider_model
                     (provider_id, model_id, source, selected, upstream_name,
                      created_at, updated_at, last_seen_at)
                 VALUES (?1, ?2, 'fetched', ?3, ?4, 5, 5, 5)",
                params![PROVIDER, model_id, selected, name],
            )
            .unwrap();
    }

    fn search(
        storage: &Storage,
        query: &str,
        limit: u32,
        filter: &str,
    ) -> Result<ProviderModelPage, ModelError> {
        list_provider_models(
            storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: None,
                limit,
                filter: filter.to_owned(),
                query: Some(query.to_owned()),
            },
        )
    }

    fn ids(page: &ProviderModelPage) -> Vec<&str> {
        page.items
            .iter()
            .map(|model| model.model_id.as_str())
            .collect()
    }

    #[test]
    fn search_ranks_across_id_name_and_alias() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
        insert_named(&storage, "deepseek-chat", Some("深度求索 Chat"), false);
        insert_named(
            &storage,
            "deepseek-reasoner",
            Some("深度求索 Reasoner"),
            false,
        );
        insert_named(
            &storage,
            "mistral/mistral-large-4",
            Some("Mistral Large"),
            false,
        );
        add(&storage, "my-local", Some("我的模型")).unwrap();

        let page = search(&storage, "ds chat", 10, "all").unwrap();
        assert_eq!(ids(&page), ["deepseek-chat"]);
        assert_eq!(
            (page.total_matches, page.next_cursor.as_deref()),
            (Some(1), None)
        );
        // Chinese words in the upstream name: contains, subsequence, exact.
        for query in ["求索", "深求", "深度求索"] {
            let page = search(&storage, query, 10, "all").unwrap();
            assert_eq!(
                ids(&page),
                ["deepseek-chat", "deepseek-reasoner"],
                "{query}"
            );
        }
        assert_eq!(
            ids(&search(&storage, "我的", 10, "all").unwrap()),
            ["my-local"]
        );
        assert_eq!(
            ids(&search(&storage, "MISTRAL large", 10, "all").unwrap()),
            ["mistral/mistral-large-4"]
        );
        // `%` and `_` are not wildcards here: nothing contains `%`.
        assert_eq!(
            search(&storage, "%", 10, "all").unwrap().total_matches,
            Some(0)
        );
        assert_eq!(
            search(&storage, "'; DROP TABLE provider_model; --", 10, "all")
                .unwrap()
                .total_matches,
            Some(0)
        );
        assert_eq!(
            list(&storage, None, 10, "all").items.len(),
            4,
            "the table is intact"
        );
    }

    #[test]
    fn search_truncates_to_the_limit_and_reports_the_total() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
        for id in ["b/chat", "a/chat", "chat", "c/chatty", "d/other"] {
            insert_named(&storage, id, None, false);
        }
        let page = search(&storage, "chat", 2, "all").unwrap();
        assert_eq!(
            ids(&page),
            ["chat", "a/chat"],
            "full match first, then ties by ID"
        );
        assert_eq!(page.total_matches, Some(4));
        assert_eq!(page.next_cursor, None);
        let all = search(&storage, "chat", MAX_MODEL_PAGE_SIZE, "all").unwrap();
        assert_eq!(ids(&all), ["chat", "a/chat", "b/chat", "c/chatty"]);
    }

    #[test]
    fn search_combines_with_the_selected_filter() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        insert_named(&storage, "deepseek-chat", None, true);
        insert_named(&storage, "deepseek-reasoner", None, false);
        let selected = search(&storage, "deepseek", 10, "selected").unwrap();
        assert_eq!(
            (ids(&selected), selected.total_matches),
            (vec!["deepseek-chat"], Some(1))
        );
        assert_eq!(
            search(&storage, "deepseek", 10, "all")
                .unwrap()
                .total_matches,
            Some(2)
        );
    }

    #[test]
    fn empty_queries_list_normally_and_bad_queries_are_rejected() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
        for id in ["a", "b", "c"] {
            insert_named(&storage, id, None, false);
        }
        for blank in ["", "   ", " - _ / . "] {
            let page = search(&storage, blank, 2, "all").unwrap();
            assert_eq!(ids(&page), ["a", "b"], "{blank:?} is no query");
            assert_eq!(page.total_matches, None);
            assert!(page.next_cursor.is_some(), "paging still works");
        }
        assert!(search(&storage, &"深".repeat(200), 10, "all").is_ok());
        assert_eq!(
            search(&storage, &"深".repeat(201), 10, "all"),
            Err(ModelError::InvalidRequest)
        );
        let with_cursor = list_provider_models(
            &storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: Some(encode_cursor("a")),
                limit: 10,
                filter: "all".to_owned(),
                query: Some("b".to_owned()),
            },
        );
        assert_eq!(with_cursor, Err(ModelError::InvalidRequest));
        // A blank query with a cursor is just the next page.
        let blank_with_cursor = list_provider_models(
            &storage,
            &ListProviderModelsRequest {
                provider_id: PROVIDER.to_owned(),
                after: Some(encode_cursor("a")),
                limit: 10,
                filter: "all".to_owned(),
                query: Some("  ".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(ids(&blank_with_cursor), ["b", "c"]);
        // Limits and filters are still checked for searches.
        assert_eq!(
            search(&storage, "a", 0, "all"),
            Err(ModelError::InvalidRequest)
        );
        assert_eq!(
            search(&storage, "a", 10, "ticked"),
            Err(ModelError::InvalidRequest)
        );
    }

    #[test]
    fn the_request_accepts_an_absent_query_but_no_unknown_fields() {
        let without: ListProviderModelsRequest = serde_json::from_value(serde_json::json!({
            "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all"
        }))
        .unwrap();
        assert_eq!(without.query, None);
        let with: ListProviderModelsRequest = serde_json::from_value(serde_json::json!({
            "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all", "query": "ds"
        }))
        .unwrap();
        assert_eq!(with.query.as_deref(), Some("ds"));
        assert!(
            serde_json::from_value::<ListProviderModelsRequest>(serde_json::json!({
                "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all", "search": "ds"
            }))
            .is_err()
        );
    }

    #[test]
    fn searching_five_thousand_models_is_fast_enough() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
        {
            let mut connection = storage.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            for index in 0..5000 {
                transaction
                    .execute(
                        "INSERT INTO provider_model
                             (provider_id, model_id, source, selected, upstream_name,
                              created_at, updated_at, last_seen_at)
                         VALUES (?1, ?2, 'fetched', ?3, ?4, 5, 5, 5)",
                        params![
                            PROVIDER,
                            format!("vendor-{}/model-{index:04}-chat", index % 50),
                            index % 7 == 0,
                            format!("Vendor {} Model {index} 深度求索", index % 50)
                        ],
                    )
                    .unwrap();
            }
            transaction.commit().unwrap();
        }
        let started = std::time::Instant::now();
        let broad = search(&storage, "chat", MAX_MODEL_PAGE_SIZE, "all").unwrap();
        let narrow = search(&storage, "vendor 7 model 0007", 10, "all").unwrap();
        let none = search(&storage, "zzzz-not-there", 10, "all").unwrap();
        let elapsed = started.elapsed();
        eprintln!("5000-row search, three queries: {elapsed:?}");
        assert_eq!(broad.total_matches, Some(5000));
        assert_eq!(broad.items.len(), 200);
        assert_eq!(narrow.items[0].model_id, "vendor-7/model-0007-chat");
        assert_eq!(none.total_matches, Some(0));
        // A loose bound for slow, shared CI machines in debug builds; this guards
        // against an accidental quadratic algorithm, not against small slowdowns.
        assert!(elapsed < std::time::Duration::from_secs(10), "{elapsed:?}");
    }

    #[test]
    fn models_serialize_with_camel_case_fields_and_snake_case_values() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(&storage, PROVIDER, "command-code", "chat_completions");
        let model = add(&storage, "a/b", None).unwrap();
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["modelId"], "a/b");
        assert_eq!(json["routeSupport"], "unknown");
        assert_eq!(json["upstreamState"], "never_listed");
        assert_eq!(json["source"], "manual");
        assert_eq!(
            serde_json::to_value(ModelError::ModelRouteNotSupported).unwrap(),
            "model_route_not_supported"
        );
        assert_eq!(
            ModelError::InvalidStoredModel.to_string(),
            "invalid_stored_model"
        );
    }
}
