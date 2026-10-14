//! Stable model IPC errors shared by saved-model edits and model-list fetch.
//!
//! Saved rows and fetch each keep their own modules. Both report failures through
//! this enum so neither one owns the other's error type. Codes carry no payload.

use crate::credentials::CredentialError;
use crate::model_catalog::CatalogError;
use crate::model_http::HttpError;
use crate::providers::ProviderId;
use serde::Serialize;

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
    /// A context window, output limit, or thinking-effort value is not usable.
    ModelParametersInvalid,
    /// The provider already has a model with this ID.
    ModelAlreadyExists,
    /// A requested model does not exist (the list may have changed).
    ModelNotFound,
    /// Saved model content changed after the editor read it; reload before retrying.
    ModelChanged,
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
            Self::ModelParametersInvalid => "model_parameters_invalid",
            Self::ModelAlreadyExists => "model_already_exists",
            Self::ModelNotFound => "model_not_found",
            Self::ModelChanged => "model_changed",
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

/// Parse a provider identity and map invalid input to the stable model error.
pub(crate) fn parse_provider_id(value: &str) -> Result<ProviderId, ModelError> {
    ProviderId::parse(value).ok_or(ModelError::InvalidRequest)
}
