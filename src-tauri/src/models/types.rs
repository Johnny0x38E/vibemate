//! Saved-model requests, records, source states, and stable IPC errors.

use crate::credentials::CredentialError;
use crate::http_client::HttpError;
use crate::model_catalog::{CatalogError, RouteSupport};
use serde::{Deserialize, Serialize};

/// Largest page of models.
pub const MAX_MODEL_PAGE_SIZE: u32 = 200;

/// Most model IDs accepted by one selection change.
pub const MAX_SELECTION_BATCH: usize = 500;

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
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Fetched => "fetched",
            Self::Manual => "manual",
        }
    }

    pub(super) fn from_value(value: &str) -> Option<Self> {
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
