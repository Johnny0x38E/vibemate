//! Provider identities, requests, records, templates, and stable error codes.

use super::provider_templates;
use crate::credentials::{CredentialError, InvalidSecret, Secret};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest accepted display name, counted in Unicode characters after trimming.
pub const MAX_DISPLAY_NAME_CHARS: usize = 64;

/// Longest accepted base URL, in bytes, before and after normalization.
pub const MAX_BASE_URL_LEN: usize = 2048;

/// A supported vendor. The wire and database values are the configuration
/// identities recorded in `docs/integrations/providers.md`.
///
/// Requests carry the kind as a plain string and `parse_kind` converts it, so an
/// unknown value produces the stable `KindNotSupported` code instead of a raw
/// deserialization message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProviderKind {
    /// Command Code's provider API. The recorded evidence covers the GOAT plan only;
    /// see `docs/integrations/providers.md`.
    #[serde(rename = "command-code")]
    CommandCode,
    /// The DeepSeek API used directly.
    #[serde(rename = "deepseek")]
    DeepSeek,
    /// The OpenRouter API.
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl ProviderKind {
    /// The value used on the wire and in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandCode => "command-code",
            Self::DeepSeek => "deepseek",
            Self::OpenRouter => "openrouter",
        }
    }

    /// Convert a stored or requested value, returning `None` for anything unknown.
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "command-code" => Some(Self::CommandCode),
            "deepseek" => Some(Self::DeepSeek),
            "openrouter" => Some(Self::OpenRouter),
            _ => None,
        }
    }

    /// The built-in template that describes this kind's defaults and limits.
    pub fn template(self) -> &'static ProviderTemplate {
        match self {
            Self::CommandCode => &provider_templates()[0],
            Self::DeepSeek => &provider_templates()[1],
            Self::OpenRouter => &provider_templates()[2],
        }
    }
}

/// The request protocol (API family) an instance uses.
///
/// All three routes are documented for Command Code, but each kind's template
/// decides which ones vibemate currently allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    /// OpenAI-style `POST /chat/completions`.
    ChatCompletions,
    /// OpenAI-style `POST /responses`.
    Responses,
    /// Anthropic-style `POST /messages`.
    AnthropicMessages,
}

impl ProviderProtocol {
    /// The value used on the wire and in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    /// Convert a stored or requested value, returning `None` for anything unknown.
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "chat_completions" => Some(Self::ChatCompletions),
            "responses" => Some(Self::Responses),
            "anthropic_messages" => Some(Self::AnthropicMessages),
            _ => None,
        }
    }
}

/// The stable identity of one provider instance: 32 lowercase hexadecimal digits
/// (128 random bits). Rust creates it once; the frontend can only send it back.
///
/// `#[serde(transparent)]` serializes the inner string directly, so the frontend
/// receives `"0f3a..."` rather than an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    /// Accept only the exact format vibemate generates, returning `None` otherwise.
    pub fn parse(value: &str) -> Option<Self> {
        let well_formed = value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        well_formed.then(|| Self(value.to_string()))
    }

    /// Borrow the identifier as text, for SQL parameters and cursors.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The OS credential-store entry name for this instance's API key:
    /// `provider-<id>`. It is built only from the ID, never from the display name,
    /// and the `provider_credential` table stores exactly this value.
    pub fn credential_reference(&self) -> String {
        format!("provider-{}", self.0)
    }
}

/// Defaults and limits for one kind, shown by the form and enforced by validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTemplate {
    /// Which vendor this template describes.
    pub kind: ProviderKind,
    /// Brand name shown as-is in every language.
    pub brand_name: &'static str,
    /// Documented base URL; users may replace it with another HTTPS URL.
    pub default_base_url: &'static str,
    /// Protocols an instance of this kind may use today.
    pub protocols: &'static [ProviderProtocol],
    /// Protocol preselected in a new form.
    pub default_protocol: ProviderProtocol,
    /// Allowed extension keys. Empty until evidence documents a non-secret field.
    pub extension_fields: &'static [&'static str],
}

/// Stable IPC error codes for provider operations.
///
/// Each code maps to one user-facing message in the frontend's translations, so
/// codes exist only where the interface needs a different explanation. No SQL,
/// path, URL, or user input is carried across the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderError {
    /// Storage failed at startup or after a panic; nothing was read or written.
    StorageUnavailable,
    /// SQLite could not read provider data.
    ReadFailed,
    /// SQLite rejected the write, so the previously committed data is unchanged.
    WriteFailed,
    /// The operation could not finish and its outcome is unknown; reload first.
    OperationFailed,
    /// A stored row holds a value this build does not accept; it is left unchanged.
    InvalidStoredProvider,
    /// No provider instance has the requested ID.
    NotFound,
    /// The instance changed since the form loaded it; reload before editing again.
    RevisionConflict,
    /// The request itself is malformed (ID, cursor, page size, or revision).
    /// The user cannot fix this by editing the form, so one message covers it.
    InvalidRequest,
    /// The requested kind is not one of the supported vendors.
    KindNotSupported,
    /// The protocol is unknown or not allowed for this kind.
    ProtocolNotSupported,
    /// The display name is empty, too long, or contains control characters.
    DisplayNameInvalid,
    /// The base URL cannot be parsed, is too long, or has a query or fragment.
    BaseUrlInvalid,
    /// The base URL does not use HTTPS.
    BaseUrlNotHttps,
    /// The base URL contains a user name or password, which could leak a secret.
    BaseUrlHasCredentials,
    /// An extension key is not allowed for this kind (currently every key).
    ExtensionFieldNotSupported,
    /// The API key is empty, contains a control character such as a line break, or
    /// is longer than the credential store allows.
    SecretInvalid,
    /// This device has no usable OS credential store, so nothing was saved. The key
    /// is never stored as plain text instead.
    CredentialStoreUnavailable,
    /// The OS credential store is locked or refused access.
    CredentialStoreAccessDenied,
    /// The OS credential store reported another error, including a denied access
    /// prompt on macOS; nothing was changed.
    CredentialStoreFailed,
    /// Creating failed and the new credential entry could not be deleted again, so
    /// whether anything was saved is unknown. Any leftover entry has no reference
    /// and is never read; refresh the list before adding the provider again.
    CreateOutcomeUnknown,
    /// Replacing failed and the previous key could not be restored, so the store
    /// may already hold the new key. Refresh the key status before acting.
    SecretOutcomeUnknown,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => "Provider storage is unavailable.",
            Self::ReadFailed => "Provider settings could not be read.",
            Self::WriteFailed => "Provider settings could not be saved.",
            Self::OperationFailed => "The provider operation could not finish.",
            Self::InvalidStoredProvider => "Stored provider settings are invalid.",
            Self::NotFound => "The provider was not found.",
            Self::RevisionConflict => "The provider changed after it was loaded.",
            Self::InvalidRequest => "The provider request is invalid.",
            Self::KindNotSupported => "The provider type is not supported.",
            Self::ProtocolNotSupported => "The protocol is not supported for this provider.",
            Self::DisplayNameInvalid => "The provider name is invalid.",
            Self::BaseUrlInvalid => "The base URL is invalid.",
            Self::BaseUrlNotHttps => "The base URL must use HTTPS.",
            Self::BaseUrlHasCredentials => "The base URL must not contain credentials.",
            Self::ExtensionFieldNotSupported => "An extension field is not supported.",
            Self::SecretInvalid => "The API key is invalid.",
            Self::CredentialStoreUnavailable => "No OS credential store is available.",
            Self::CredentialStoreAccessDenied => "The OS credential store refused access.",
            Self::CredentialStoreFailed => "The OS credential store reported an error.",
            Self::CreateOutcomeUnknown => "Whether the provider was saved is unknown.",
            Self::SecretOutcomeUnknown => "Whether the API key was replaced is unknown.",
        })
    }
}

impl std::error::Error for ProviderError {}

impl From<CredentialError> for ProviderError {
    /// Map a credential-store failure to the code shown to the user.
    ///
    /// The interface offers the same advice for a store error, a rejected entry
    /// name, and an unreadable value (try again), so those share one code.
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

impl From<InvalidSecret> for ProviderError {
    /// A rejected API key always maps to `SecretInvalid`; like `InvalidSecret`,
    /// the code carries no copy of the value.
    fn from(_: InvalidSecret) -> Self {
        Self::SecretInvalid
    }
}

/// Fields a user submits to create an instance.
///
/// `kind` and `protocol` stay strings until validation so unknown values receive
/// stable error codes. `deny_unknown_fields` rejects misspelled or extra fields.
///
/// The API key is required: every saved instance has one. `Secret` prints as
/// `<redacted>`, so the derived `Debug` never shows it, and it has no `Clone`, so
/// neither does this request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProviderRequest {
    /// One of the `ProviderKind` wire values.
    pub kind: String,
    /// Label shown in lists; not an identity.
    pub display_name: String,
    /// Base URL; normalized before storage.
    pub base_url: String,
    /// One of the kind's allowed `ProviderProtocol` values.
    pub protocol: String,
    /// Kind-specific settings; every key is rejected until evidence adds one.
    pub extensions: BTreeMap<String, String>,
    /// The API key, saved only in the OS credential store.
    pub secret: Secret,
}

/// Fields a user submits to edit an instance.
///
/// There is deliberately no `kind`: changing the vendor would invalidate models
/// and credentials attached to this identity. Because unknown fields are denied,
/// a request that tries to send `kind` fails instead of being silently ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateProviderRequest {
    /// The instance to edit.
    pub id: String,
    /// The `revision` the form loaded; a different stored revision is a conflict.
    pub expected_revision: i64,
    /// New label.
    pub display_name: String,
    /// New base URL.
    pub base_url: String,
    /// New protocol, checked against the stored kind.
    pub protocol: String,
    /// New extension settings.
    pub extensions: BTreeMap<String, String>,
}

/// One saved instance as returned to the frontend. It never contains secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRecord {
    /// Stable identity.
    pub id: ProviderId,
    /// Vendor; fixed after creation.
    pub kind: ProviderKind,
    /// Trimmed label.
    pub display_name: String,
    /// Normalized HTTPS base URL.
    pub base_url: String,
    /// Selected protocol.
    pub protocol: ProviderProtocol,
    /// Saved extension settings; always empty until a key is supported.
    pub extensions: BTreeMap<String, String>,
    /// Starts at 1 and increases by 1 with every successful edit.
    pub revision: i64,
    /// Creation time in Unix epoch milliseconds; never changes.
    pub created_at_ms: i64,
    /// Last edit time in Unix epoch milliseconds.
    pub updated_at_ms: i64,
    /// Models the user has selected for this configuration.
    pub selected_model_count: u32,
}

/// Validated, normalized editable fields shared by create and update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettings {
    /// Trimmed display name.
    pub display_name: String,
    /// Normalized base URL.
    pub base_url: String,
    /// Protocol allowed for the kind.
    pub protocol: ProviderProtocol,
    /// Allowed extension settings.
    pub extensions: BTreeMap<String, String>,
}

/// Largest page size `list_providers` accepts.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Ask for one page of instances, ordered by `created_at` and then `id`.
///
/// `after` is the opaque `next_cursor` of the previous page. Keyset ("cursor")
/// pagination continues after the last row seen, so an instance added between two
/// requests can never cause a skipped or repeated row, unlike an offset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListProvidersRequest {
    /// Cursor from the previous page, or `None` for the first page.
    pub after: Option<String>,
    /// Page size from 1 to `MAX_PAGE_SIZE`. Required, so no hidden default applies.
    pub limit: u32,
}

/// One bounded page of instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPage {
    /// At most `limit` instances in stable order.
    pub items: Vec<ProviderRecord>,
    /// Pass this as `after` to read the next page; `None` means this is the last page.
    pub next_cursor: Option<String>,
}
