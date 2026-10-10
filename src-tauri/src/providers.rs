//! Provider account instances: validated, non-secret settings saved by the user.
//!
//! A provider instance is one account configuration for a supported vendor, called
//! its "kind". Several instances may share a kind and even a display name; each one
//! is identified only by its stable random `ProviderId`, never by its name.
//!
//! This module validates user input and stores instances in the private SQLite
//! database (`provider_instance`, schema version 5, and the `provider_credential`
//! reference table from version 6). It has no Tauri dependency and sends no network
//! requests. Creating an instance also saves its required API key, but only through
//! a `CredentialStore` (see `crate::credentials`): the key never enters SQLite,
//! a response, an error, or `Debug` output.
//!
//! Template values (default base URL and allowed protocols) come from the evidence
//! in `docs/integrations/providers.md`. Do not add a value here without evidence.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::credentials::{
    CommitFailure, CredentialError, CredentialStore, InvalidSecret, ReplaceError, SaveNewError,
    Secret, replace_then_commit, save_new_then_commit, validate_secret,
};
use crate::storage::Storage;

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
            Self::CommandCode => &TEMPLATES[0],
            Self::DeepSeek => &TEMPLATES[1],
            Self::OpenRouter => &TEMPLATES[2],
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

/// Built-in templates in the order of `docs/integrations/providers.md`.
///
/// Every kind allows only Chat Completions for now. DeepSeek and OpenRouter use it
/// as the documented first path. Command Code routes by model, so a per-model
/// override is left to the model tasks (P12/P15) rather than guessed here.
const TEMPLATES: [ProviderTemplate; 3] = [
    ProviderTemplate {
        kind: ProviderKind::CommandCode,
        brand_name: "Command Code",
        default_base_url: "https://api.commandcode.ai/provider/v1",
        protocols: &[ProviderProtocol::ChatCompletions],
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
    ProviderTemplate {
        kind: ProviderKind::DeepSeek,
        brand_name: "DeepSeek",
        default_base_url: "https://api.deepseek.com",
        protocols: &[ProviderProtocol::ChatCompletions],
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
    ProviderTemplate {
        kind: ProviderKind::OpenRouter,
        brand_name: "OpenRouter",
        default_base_url: "https://openrouter.ai/api/v1",
        protocols: &[ProviderProtocol::ChatCompletions],
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
];

/// Return every built-in template. This reads constants only and cannot fail.
pub fn provider_templates() -> &'static [ProviderTemplate] {
    &TEMPLATES
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

/// Convert a requested kind string.
///
/// # Errors
/// Returns `KindNotSupported` for any value other than the three wire values.
pub fn parse_kind(value: &str) -> Result<ProviderKind, ProviderError> {
    ProviderKind::from_value(value).ok_or(ProviderError::KindNotSupported)
}

/// Validate every editable field for `kind`, in a fixed order, stopping at the
/// first problem: name, then base URL, then protocol, then extensions.
///
/// # Errors
/// Returns the code of the first invalid field.
pub fn validate_settings(
    kind: ProviderKind,
    display_name: &str,
    base_url: &str,
    protocol: &str,
    extensions: &BTreeMap<String, String>,
) -> Result<ProviderSettings, ProviderError> {
    Ok(ProviderSettings {
        display_name: validate_display_name(display_name)?,
        base_url: normalize_base_url(base_url)?,
        protocol: parse_protocol_for(kind, protocol)?,
        extensions: validate_extensions(kind, extensions)?,
    })
}

/// Trim a display name and check its length and characters.
///
/// Length is counted in characters (`chars()`), which matches SQLite's
/// `length()` for text, so Chinese names get the same limit as English ones.
///
/// # Errors
/// Returns `DisplayNameInvalid` for an empty or whitespace-only name, more than
/// `MAX_DISPLAY_NAME_CHARS` characters, any control character such as a newline,
/// any character listed by `is_hidden_format_character`, or a name with nothing
/// visible left (for example only spaces and zero-width joiners).
pub fn validate_display_name(input: &str) -> Result<String, ProviderError> {
    let name = input.trim();
    let length = name.chars().count();
    if length == 0 || length > MAX_DISPLAY_NAME_CHARS {
        return Err(ProviderError::DisplayNameInvalid);
    }
    if name
        .chars()
        .any(|c| c.is_control() || is_hidden_format_character(c))
    {
        return Err(ProviderError::DisplayNameInvalid);
    }
    let has_visible_character = name
        .chars()
        .any(|c| !c.is_whitespace() && !is_allowed_joiner(c));
    if !has_visible_character {
        return Err(ProviderError::DisplayNameInvalid);
    }
    Ok(name.to_string())
}

/// Zero-width joiners that real text needs: U+200C (ZWNJ, used in Persian and
/// Indic scripts) and U+200D (ZWJ, used in emoji such as family sequences). They
/// are allowed inside a name but do not count as visible content.
fn is_allowed_joiner(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}')
}

/// Invisible Unicode format (category Cf) characters a name must not contain.
///
/// The standard library has no Unicode category lookup, and a dependency for one
/// check is not worth it, so this lists the Cf characters that can hide text or
/// reorder how it is displayed. Bidi controls are the important case: in
/// `"a\u{202E}b"` everything after U+202E is shown right to left, so two names can
/// look identical while being different. The list is deliberately not all of Cf:
/// the joiners above and emoji tag characters (U+E0020..=U+E007F, used in flags
/// such as Scotland's) stay allowed so normal names and emoji keep working.
pub(crate) fn is_hidden_format_character(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'                    // soft hyphen
            | '\u{061C}'              // Arabic letter mark (bidi)
            | '\u{180E}'              // Mongolian vowel separator
            | '\u{200B}'              // zero-width space
            | '\u{200E}'..='\u{200F}' // left-to-right and right-to-left marks (bidi)
            | '\u{202A}'..='\u{202E}' // bidi embeddings and overrides
            | '\u{2060}'..='\u{2064}' // word joiner and invisible math operators
            | '\u{2066}'..='\u{206F}' // bidi isolates and deprecated format controls
            | '\u{FEFF}'              // zero-width no-break space (byte order mark)
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation controls
    )
}

/// Parse and normalize a base URL without contacting it.
///
/// Normalization comes from the `url` crate (lowercase scheme and host, punycode
/// for international domains, percent-encoding) plus removing one trailing `/`.
/// `url` always prints a bare host as `https://host/`, so without that step the
/// documented `https://api.deepseek.com` would be stored differently from the
/// template. Only one `/` is removed, so `https://a.com/v1//` becomes
/// `https://a.com/v1/`. No path such as `/v1` is ever added. Loopback and
/// private-network hosts stay accepted: P12 (which absorbed the old P13) decided
/// to keep allowing them; see `docs/integrations/providers.md`.
///
/// # Errors
/// - `BaseUrlInvalid`: empty, longer than `MAX_BASE_URL_LEN`, contains whitespace
///   or a control character after trimming the ends, unparsable, or has a query
///   (`?`) or fragment (`#`).
/// - `BaseUrlHasCredentials`: contains `user:password@`; checked before the scheme so
///   the user learns not to put secrets in URLs.
/// - `BaseUrlNotHttps`: any scheme other than `https`.
pub fn normalize_base_url(input: &str) -> Result<String, ProviderError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_BASE_URL_LEN {
        return Err(ProviderError::BaseUrlInvalid);
    }
    // The WHATWG URL rules used by `url` silently delete tabs and newlines inside a
    // URL, so "https://api.example\n.com" would be saved as a different address
    // than the one shown. Reject such input instead of guessing what was meant.
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ProviderError::BaseUrlInvalid);
    }
    let url = Url::parse(trimmed).map_err(|_| ProviderError::BaseUrlInvalid)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ProviderError::BaseUrlHasCredentials);
    }
    if url.scheme() != "https" {
        return Err(ProviderError::BaseUrlNotHttps);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ProviderError::BaseUrlInvalid);
    }
    let serialized = url.as_str();
    let normalized = serialized.strip_suffix('/').unwrap_or(serialized);
    if normalized.len() > MAX_BASE_URL_LEN {
        return Err(ProviderError::BaseUrlInvalid);
    }
    Ok(normalized.to_string())
}

/// Convert a protocol string and check that the kind's template allows it.
fn parse_protocol_for(kind: ProviderKind, value: &str) -> Result<ProviderProtocol, ProviderError> {
    match ProviderProtocol::from_value(value) {
        Some(protocol) if kind.template().protocols.contains(&protocol) => Ok(protocol),
        _ => Err(ProviderError::ProtocolNotSupported),
    }
}

/// Accept only extension keys listed in the kind's template.
///
/// Every allowlist is empty today, so any key is rejected rather than stored or
/// silently dropped. When evidence documents a field, add its key to the template
/// and value validation and storage here.
fn validate_extensions(
    kind: ProviderKind,
    extensions: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ProviderError> {
    let allowed = kind.template().extension_fields;
    if extensions
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
    {
        return Err(ProviderError::ExtensionFieldNotSupported);
    }
    Ok(extensions.clone())
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

/// Read the system clock as Unix epoch milliseconds for `created_at`/`updated_at`.
///
/// Callers pass the result into `create_provider`/`update_provider`, which keeps
/// those functions deterministic in tests.
///
/// # Errors
/// Returns `OperationFailed` if the clock is set before 1970 or impossibly far ahead.
pub fn current_unix_millis() -> Result<i64, ProviderError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProviderError::OperationFailed)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ProviderError::OperationFailed)
}

/// Validate a new instance and its API key, save both, and return the instance.
///
/// The key goes to the OS credential store and the instance to SQLite. The two
/// stores share no transaction, so the steps are ordered to never leave half a
/// provider behind:
/// 1. Validate every field, the key last, before touching either store.
/// 2. Generate the ID with SQLite's `randomblob` (16 random bytes as lowercase
///    hex). This runs outside the insert transaction, and the connection is
///    unlocked again before step 3, because the credential store may show a
///    system prompt and wait for the user.
/// 3. Save the key under `provider-<id>`. If that fails, nothing was saved.
/// 4. In one transaction, insert the instance and its `provider_credential` row.
/// 5. If an `INSERT` in step 4 fails, nothing was committed: delete the key again
///    (`save_new_then_commit`) and report `WriteFailed`. If even that delete
///    fails, report `CreateOutcomeUnknown`: the leftover entry has no reference,
///    so the app never reads it, but the user must check the list.
/// 6. If `COMMIT` itself fails, the provider may have been saved anyway, so the
///    key is kept (deleting it could leave a saved provider without a key) and
///    the result is `CreateOutcomeUnknown`.
///
/// Random IDs, unlike reused integer IDs, cannot point at a credential left
/// behind by an older or restored database, and the primary key rejects the
/// (practically impossible) duplicate instead of overwriting a row.
///
/// Extensions are validated but not stored: every allowlist is empty today, so a
/// validated map is always empty. Storage arrives with the first supported key.
///
/// # Errors
/// Validation codes (including `SecretInvalid`), `StorageUnavailable`,
/// credential-store codes (nothing was saved), `WriteFailed` (nothing was saved),
/// or `CreateOutcomeUnknown`.
pub fn create_provider(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: CreateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let kind = parse_kind(&request.kind)?;
    let settings = validate_settings(
        kind,
        &request.display_name,
        &request.base_url,
        &request.protocol,
        &request.extensions,
    )?;
    let secret = validate_secret(request.secret)?;
    let id = generate_provider_id(storage)?;
    let reference = id.credential_reference();
    let saved = save_new_then_commit(store, &reference, &secret, || {
        insert_provider_with_reference(storage, &id, kind, &settings, &reference, now_ms)
    });
    match saved {
        Ok(()) => {
            let connection = storage
                .lock()
                .map_err(|_| ProviderError::StorageUnavailable)?;
            Ok(ProviderRecord {
                id: id.clone(),
                kind,
                display_name: settings.display_name,
                base_url: settings.base_url,
                protocol: settings.protocol,
                extensions: settings.extensions,
                revision: 1,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
                selected_model_count: read_selected_model_count(&connection, &id)?,
            })
        }
        Err(SaveNewError::Credential(error)) => Err(error.into()),
        // The key was deleted again, so the SQLite error describes the whole outcome.
        Err(SaveNewError::CommitFailed {
            source,
            cleaned_up: true,
        }) => Err(source),
        Err(SaveNewError::CommitFailed {
            cleaned_up: false, ..
        })
        | Err(SaveNewError::CommitUncertain { .. }) => Err(ProviderError::CreateOutcomeUnknown),
    }
}

/// Ask SQLite for a new random ID, holding the connection only for this query.
fn generate_provider_id(storage: &Storage) -> Result<ProviderId, ProviderError> {
    let connection = storage
        .lock()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    let generated: String = connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| ProviderError::WriteFailed)?;
    ProviderId::parse(&generated).ok_or(ProviderError::WriteFailed)
}

/// Insert a new instance and its credential reference in one transaction, so
/// either both rows exist afterwards or neither does.
///
/// Failures before `COMMIT` are `NotCommitted` (nothing was saved). A failed
/// `COMMIT` is `Uncertain`, because SQLite may report an I/O error after the
/// change already reached the file.
fn insert_provider_with_reference(
    storage: &Storage,
    id: &ProviderId,
    kind: ProviderKind,
    settings: &ProviderSettings,
    reference: &str,
    now_ms: i64,
) -> Result<(), CommitFailure<ProviderError>> {
    let not_committed = |_| CommitFailure::NotCommitted(ProviderError::WriteFailed);
    let mut connection = storage
        .lock()
        .map_err(|_| CommitFailure::NotCommitted(ProviderError::StorageUnavailable))?;
    // Dropping a transaction without `commit` rolls it back, so every early `?`
    // below leaves the database unchanged.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(not_committed)?;
    transaction
        .execute(
            "INSERT INTO provider_instance
                 (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            params![
                id.as_str(),
                kind.as_str(),
                settings.display_name,
                settings.base_url,
                settings.protocol.as_str(),
                now_ms
            ],
        )
        .map_err(not_committed)?;
    transaction
        .execute(
            "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
             VALUES (?1, ?2, ?3)",
            params![id.as_str(), reference, now_ms],
        )
        .map_err(not_committed)?;
    transaction
        .commit()
        .map_err(|_| CommitFailure::Uncertain(ProviderError::WriteFailed))
}

/// Edit one instance identified by `request.id`, keeping its ID, kind, and
/// creation time, and return the committed record.
///
/// The edit succeeds only when the stored `revision` still equals
/// `request.expected_revision` (optimistic concurrency): a form loaded before
/// another window's save cannot overwrite that newer save. The check and the
/// write run in one immediate transaction, so another app instance cannot slip
/// a change in between.
///
/// `updated_at` uses `max(now, created_at)`, so a clock that moved backwards
/// cannot make an edit look older than the creation.
///
/// # Errors
/// `InvalidRequest` for a malformed ID or revision, `NotFound`,
/// `RevisionConflict`, `InvalidStoredProvider`, validation codes,
/// `StorageUnavailable`, `ReadFailed`, or `WriteFailed` before commit. A failed
/// commit returns `OperationFailed`, because the outcome cannot be confirmed.
pub fn update_provider(
    storage: &Storage,
    request: &UpdateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    commit_provider_update(storage, request, None, now_ms).map_err(|error| match error {
        CommitFailure::NotCommitted(source) => source,
        CommitFailure::Uncertain(_) => ProviderError::OperationFailed,
    })
}

/// Save settings and optionally replace an API key through one coordinated operation.
///
/// `None` preserves the key and never accesses the credential store. A supplied key
/// is validated along with the settings before any OS prompt. The old key is kept
/// only in memory while settings and its reference commit in one SQLite transaction.
/// A concurrent settings change is rechecked after the OS call, and a definite
/// database failure restores the previous key. No database lock spans an OS prompt.
/// Callers must serialize credential writes within the app process.
///
/// # Errors
/// Returns validation, revision, storage or credential-store codes when nothing
/// changed (or was restored). Failed rollback or uncertain commit returns
/// `OperationFailed`: callers must reload and must not claim success or rollback.
/// As with other credential writes, separate app processes are not serialized.
pub fn update_provider_with_secret(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: &UpdateProviderRequest,
    secret: Option<Secret>,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let Some(secret) = secret else {
        return update_provider(storage, request, now_ms);
    };
    let record = get_provider(storage, &request.id)?;
    if request.expected_revision < 1 {
        return Err(ProviderError::InvalidRequest);
    }
    if request.expected_revision != record.revision {
        return Err(ProviderError::RevisionConflict);
    }
    validate_settings(
        record.kind,
        &request.display_name,
        &request.base_url,
        &request.protocol,
        &request.extensions,
    )?;
    let secret = validate_secret(secret)?;
    let reference = record.id.credential_reference();
    let result = replace_then_commit(store, &reference, &secret, || {
        commit_provider_update(storage, request, Some(&reference), now_ms)
    });
    match result {
        Ok(record) => Ok(record),
        Err(ReplaceError::Credential(error)) => Err(error.into()),
        Err(ReplaceError::CommitFailed {
            source,
            restored: true,
        }) => Err(source),
        Err(ReplaceError::CommitFailed {
            restored: false, ..
        })
        | Err(ReplaceError::CommitUncertain { .. }) => Err(ProviderError::OperationFailed),
    }
}

fn commit_provider_update(
    storage: &Storage,
    request: &UpdateProviderRequest,
    reference: Option<&str>,
    now_ms: i64,
) -> Result<ProviderRecord, CommitFailure<ProviderError>> {
    let mut connection = storage
        .lock()
        .map_err(|_| CommitFailure::NotCommitted(ProviderError::StorageUnavailable))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| CommitFailure::NotCommitted(ProviderError::WriteFailed))?;
    let record = update_in_transaction(&transaction, request, reference, now_ms)
        .map_err(CommitFailure::NotCommitted)?;
    transaction
        .commit()
        .map_err(|_| CommitFailure::Uncertain(ProviderError::WriteFailed))?;
    enrich_selected_model_count(&connection, record).map_err(CommitFailure::NotCommitted)
}

fn update_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    request: &UpdateProviderRequest,
    reference: Option<&str>,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let id = ProviderId::parse(&request.id).ok_or(ProviderError::InvalidRequest)?;
    if request.expected_revision < 1 {
        return Err(ProviderError::InvalidRequest);
    }
    let stored: Option<(String, i64)> = transaction
        .query_row(
            "SELECT kind, revision FROM provider_instance WHERE id = ?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| ProviderError::ReadFailed)?;
    let (stored_kind, stored_revision) = stored.ok_or(ProviderError::NotFound)?;
    if stored_revision != request.expected_revision {
        return Err(ProviderError::RevisionConflict);
    }
    // The protocol is checked against the stored kind, never a kind from the form.
    let kind =
        ProviderKind::from_value(&stored_kind).ok_or(ProviderError::InvalidStoredProvider)?;
    let settings = validate_settings(
        kind,
        &request.display_name,
        &request.base_url,
        &request.protocol,
        &request.extensions,
    )?;
    let changed = transaction
        .execute(
            "UPDATE provider_instance
             SET display_name = ?1, base_url = ?2, protocol = ?3,
                 revision = revision + 1, updated_at = max(?4, created_at)
             WHERE id = ?5 AND revision = ?6",
            params![
                settings.display_name,
                settings.base_url,
                settings.protocol.as_str(),
                now_ms,
                id.as_str(),
                request.expected_revision
            ],
        )
        .map_err(|_| ProviderError::WriteFailed)?;
    // The immediate transaction already excludes concurrent writers. Still check
    // the affected row count: a trigger or a future query change could suppress
    // the UPDATE. Never report stale data as a successful save. Returning early
    // drops the transaction and rolls back its changes.
    if changed != 1 {
        return Err(ProviderError::RevisionConflict);
    }
    if let Some(reference) = reference {
        // A legacy instance may have no key reference yet. Keep this upsert in
        // the same transaction as its settings, so either both rows commit or
        // both roll back before the credential helper restores the old key.
        transaction
            .execute(
                "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT (provider_id) DO UPDATE SET updated_at = excluded.updated_at",
                params![id.as_str(), reference, now_ms],
            )
            .map_err(|_| ProviderError::WriteFailed)?;
    }
    // Read before commit so a malformed response cannot leave a partial save.
    read_record(transaction, &id)
        .map_err(|_| ProviderError::WriteFailed)?
        .ok_or(ProviderError::WriteFailed)
}

/// Read one instance by ID.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `NotFound`, `StorageUnavailable`,
/// `ReadFailed`, or `InvalidStoredProvider`.
pub fn get_provider(storage: &Storage, id: &str) -> Result<ProviderRecord, ProviderError> {
    let id = ProviderId::parse(id).ok_or(ProviderError::InvalidRequest)?;
    let connection = storage
        .lock()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    let record = read_record(&connection, &id)?.ok_or(ProviderError::NotFound)?;
    enrich_selected_model_count(&connection, record)
}

/// Read one bounded page ordered by `created_at` ascending, then `id` ascending.
///
/// The ID breaks ties between instances created in the same millisecond, so the
/// order is total and stable. Editing never moves an instance, because edits do
/// not change `created_at`. One extra row is fetched to learn whether another
/// page exists. A row this build cannot read fails the whole page rather than
/// being skipped silently.
///
/// # Errors
/// `InvalidRequest` for a limit outside 1..=`MAX_PAGE_SIZE` or a malformed
/// cursor, `StorageUnavailable`, `ReadFailed`, or `InvalidStoredProvider`.
pub fn list_providers(
    storage: &Storage,
    request: &ListProvidersRequest,
) -> Result<ProviderPage, ProviderError> {
    if request.limit == 0 || request.limit > MAX_PAGE_SIZE {
        return Err(ProviderError::InvalidRequest);
    }
    let after = match &request.after {
        Some(cursor) => Some(parse_cursor(cursor)?),
        None => None,
    };
    let (after_created_at, after_id) = match &after {
        Some((created_at, id)) => (Some(*created_at), Some(id.as_str())),
        None => (None, None),
    };
    let connection = storage
        .lock()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    // `(a, b) > (x, y)` is SQLite's row-value comparison: a > x, or a = x and b > y.
    // With no cursor, `?1 IS NULL` is true and the comparison is not needed.
    let mut statement = connection
        .prepare(&format!(
            "SELECT {RECORD_COLUMNS},
                    COALESCE(
                        (SELECT COUNT(*) FROM provider_model m
                         WHERE m.provider_id = provider_instance.id AND m.selected = 1),
                        0
                    ) AS selected_model_count
             FROM provider_instance
             WHERE ?1 IS NULL OR (created_at, id) > (?1, ?2)
             ORDER BY created_at, id
             LIMIT ?3"
        ))
        .map_err(|_| ProviderError::ReadFailed)?;
    let rows = statement
        .query_map(
            params![after_created_at, after_id, i64::from(request.limit) + 1],
            ListedRow::read,
        )
        .map_err(|_| ProviderError::ReadFailed)?;
    let mut items = Vec::new();
    for row in rows {
        let stored = row.map_err(|_| ProviderError::ReadFailed)?;
        items.push(stored.into_record()?);
    }
    let page_size = usize::try_from(request.limit).map_err(|_| ProviderError::InvalidRequest)?;
    let next_cursor = if items.len() > page_size {
        items.truncate(page_size);
        items.last().map(cursor_after)
    } else {
        None
    };
    Ok(ProviderPage { items, next_cursor })
}

/// Columns read for a record, in the order `StoredRow::read` expects.
const RECORD_COLUMNS: &str =
    "id, kind, display_name, base_url, protocol, revision, created_at, updated_at";

/// Read one record, returning `None` when the ID does not exist.
fn read_record(
    connection: &Connection,
    id: &ProviderId,
) -> Result<Option<ProviderRecord>, ProviderError> {
    let stored = connection
        .query_row(
            &format!("SELECT {RECORD_COLUMNS} FROM provider_instance WHERE id = ?1"),
            [id.as_str()],
            StoredRow::read,
        )
        .optional()
        .map_err(|_| ProviderError::ReadFailed)?;
    stored.map(StoredRow::into_record).transpose()
}

/// Raw column values before they are checked against this build's types.
struct StoredRow {
    id: String,
    kind: String,
    display_name: String,
    base_url: String,
    protocol: String,
    revision: i64,
    created_at: i64,
    updated_at: i64,
}

impl StoredRow {
    /// Copy columns from a row selected with `RECORD_COLUMNS`.
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            kind: row.get(1)?,
            display_name: row.get(2)?,
            base_url: row.get(3)?,
            protocol: row.get(4)?,
            revision: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }

    /// Convert to a typed record. Unknown values are reported, never replaced.
    fn into_record(self) -> Result<ProviderRecord, ProviderError> {
        self.into_record_with_selected_count(0)
    }

    fn into_record_with_selected_count(
        self,
        selected_model_count: u32,
    ) -> Result<ProviderRecord, ProviderError> {
        let invalid = ProviderError::InvalidStoredProvider;
        Ok(ProviderRecord {
            id: ProviderId::parse(&self.id).ok_or(invalid)?,
            kind: ProviderKind::from_value(&self.kind).ok_or(invalid)?,
            display_name: self.display_name,
            base_url: self.base_url,
            protocol: ProviderProtocol::from_value(&self.protocol).ok_or(invalid)?,
            extensions: BTreeMap::new(),
            revision: self.revision,
            created_at_ms: self.created_at,
            updated_at_ms: self.updated_at,
            selected_model_count,
        })
    }
}

/// One list row including the selected-model count from the list query.
struct ListedRow {
    row: StoredRow,
    selected_model_count: i64,
}

impl ListedRow {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            row: StoredRow::read(row)?,
            selected_model_count: row.get(8)?,
        })
    }

    fn into_record(self) -> Result<ProviderRecord, ProviderError> {
        let count =
            u32::try_from(self.selected_model_count).map_err(|_| ProviderError::ReadFailed)?;
        self.row.into_record_with_selected_count(count)
    }
}

fn read_selected_model_count(
    connection: &Connection,
    id: &ProviderId,
) -> Result<u32, ProviderError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM provider_model WHERE provider_id = ?1 AND selected = 1",
            [id.as_str()],
            |row| row.get(0),
        )
        .map_err(|_| ProviderError::ReadFailed)?;
    u32::try_from(count).map_err(|_| ProviderError::ReadFailed)
}

fn enrich_selected_model_count(
    connection: &Connection,
    mut record: ProviderRecord,
) -> Result<ProviderRecord, ProviderError> {
    record.selected_model_count = read_selected_model_count(connection, &record.id)?;
    Ok(record)
}

/// Build the cursor that continues after `record`: `"<created_at_ms>.<id>"`.
///
/// The frontend treats the cursor as opaque text and only sends it back.
fn cursor_after(record: &ProviderRecord) -> String {
    format!("{}.{}", record.created_at_ms, record.id.as_str())
}

/// Split a cursor made by `cursor_after` back into its sort key.
fn parse_cursor(cursor: &str) -> Result<(i64, ProviderId), ProviderError> {
    let (created_at, id) = cursor
        .split_once('.')
        .ok_or(ProviderError::InvalidRequest)?;
    // Only plain ASCII digits: `parse` alone would also accept "+5" and "-5",
    // and `cursor_after` never produces a sign. Too many digits overflow `parse`.
    if created_at.is_empty() || !created_at.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProviderError::InvalidRequest);
    }
    let created_at = created_at
        .parse::<i64>()
        .map_err(|_| ProviderError::InvalidRequest)?;
    let id = ProviderId::parse(id).ok_or(ProviderError::InvalidRequest)?;
    Ok((created_at, id))
}

/// Test support shared with `provider_secrets`.
///
/// Make `COMMIT` fail after a reference row is inserted or updated, while every
/// statement before it succeeds.
///
/// A deferred foreign key is checked only at `COMMIT`. The test-only TEMP
/// trigger adds a row that breaks such a key, so `COMMIT` reports a constraint
/// error and SQLite rolls the transaction back. This is how a test can reach
/// the `COMMIT` failure path, which in real use comes from I/O errors.
#[cfg(test)]
pub(crate) fn fail_commits_after_reference_writes(storage: &Storage) {
    storage
        .lock()
        .expect("lock")
        .execute_batch(
            "CREATE TEMP TABLE commit_guard_parent (id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE commit_guard_child (
                 parent INTEGER REFERENCES commit_guard_parent (id)
                     DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TEMP TRIGGER fail_commit_on_insert AFTER INSERT ON provider_credential
             BEGIN INSERT INTO commit_guard_child VALUES (1); END;
             CREATE TEMP TRIGGER fail_commit_on_update AFTER UPDATE ON provider_credential
             BEGIN INSERT INTO commit_guard_child VALUES (1); END;",
        )
        .expect("commit guard");
}

#[cfg(test)]
mod tests;
