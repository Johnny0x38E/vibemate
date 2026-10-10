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
mod tests;
