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
    CommitFailure, CredentialError, CredentialStore, InvalidSecret, SaveNewError, Secret,
    save_new_then_commit, validate_secret,
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
        Ok(()) => Ok(ProviderRecord {
            id,
            kind,
            display_name: settings.display_name,
            base_url: settings.base_url,
            protocol: settings.protocol,
            extensions: settings.extensions,
            revision: 1,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        }),
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
/// `StorageUnavailable`, `ReadFailed`, or `WriteFailed`. No error leaves a
/// partial change.
pub fn update_provider(
    storage: &Storage,
    request: &UpdateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let id = ProviderId::parse(&request.id).ok_or(ProviderError::InvalidRequest)?;
    if request.expected_revision < 1 {
        return Err(ProviderError::InvalidRequest);
    }
    let mut connection = storage
        .lock()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ProviderError::WriteFailed)?;
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
    // The revision was checked above, but the transaction only takes SQLite's write
    // lock at this UPDATE. Another connection (for example a second app process)
    // could have edited the row between the SELECT and here; then `WHERE revision`
    // matches nothing. Report that as a conflict instead of returning stale data.
    // Returning early drops the transaction, which rolls it back.
    if changed != 1 {
        return Err(ProviderError::RevisionConflict);
    }
    // Read the row back inside the transaction so the response shows exactly what
    // will be committed. A failure here rolls back the update, so it is a write failure.
    let record = read_record(&transaction, &id)
        .map_err(|_| ProviderError::WriteFailed)?
        .ok_or(ProviderError::WriteFailed)?;
    transaction
        .commit()
        .map_err(|_| ProviderError::WriteFailed)?;
    Ok(record)
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
    read_record(&connection, &id)?.ok_or(ProviderError::NotFound)
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
            "SELECT {RECORD_COLUMNS} FROM provider_instance
             WHERE ?1 IS NULL OR (created_at, id) > (?1, ?2)
             ORDER BY created_at, id
             LIMIT ?3"
        ))
        .map_err(|_| ProviderError::ReadFailed)?;
    let rows = statement
        .query_map(
            params![after_created_at, after_id, i64::from(request.limit) + 1],
            StoredRow::read,
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
        })
    }
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
mod tests {
    use super::*;
    use crate::credentials::fake::FakeStore;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn no_extensions() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn templates_use_the_documented_defaults_and_only_chat_completions() {
        let expected = [
            (
                ProviderKind::CommandCode,
                "https://api.commandcode.ai/provider/v1",
            ),
            (ProviderKind::DeepSeek, "https://api.deepseek.com"),
            (ProviderKind::OpenRouter, "https://openrouter.ai/api/v1"),
        ];
        assert_eq!(provider_templates().len(), expected.len());
        for (template, (kind, url)) in provider_templates().iter().zip(expected) {
            assert_eq!(template.kind, kind);
            assert_eq!(kind.template(), template);
            assert_eq!(template.default_base_url, url);
            assert_eq!(template.protocols, &[ProviderProtocol::ChatCompletions]);
            assert!(template.extension_fields.is_empty());
            // A default must survive normalization unchanged, or a fresh form would
            // save a different URL than the one it displayed.
            assert_eq!(normalize_base_url(url).as_deref(), Ok(url));
            assert_eq!(ProviderKind::from_value(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn credential_failures_map_to_stable_codes() {
        for (error, code) in [
            (
                CredentialError::Unavailable,
                ProviderError::CredentialStoreUnavailable,
            ),
            (
                CredentialError::AccessDenied,
                ProviderError::CredentialStoreAccessDenied,
            ),
            (
                CredentialError::OperationFailed,
                ProviderError::CredentialStoreFailed,
            ),
            (
                CredentialError::InvalidReference,
                ProviderError::CredentialStoreFailed,
            ),
            (
                CredentialError::CorruptValue,
                ProviderError::CredentialStoreFailed,
            ),
        ] {
            assert_eq!(ProviderError::from(error), code);
        }
        assert_eq!(
            ProviderError::from(InvalidSecret),
            ProviderError::SecretInvalid
        );
    }

    #[test]
    fn every_kind_picks_its_own_template() {
        // `template()` selects by array index; this catches a reordered TEMPLATES.
        for kind in [
            ProviderKind::CommandCode,
            ProviderKind::DeepSeek,
            ProviderKind::OpenRouter,
        ] {
            assert_eq!(kind.template().kind, kind);
        }
    }

    #[test]
    fn unknown_kinds_receive_a_stable_code() {
        for value in [
            "",
            "custom",
            "DeepSeek",
            "command_code",
            "Command Code",
            " deepseek",
        ] {
            assert_eq!(parse_kind(value), Err(ProviderError::KindNotSupported));
        }
        assert_eq!(parse_kind("openrouter"), Ok(ProviderKind::OpenRouter));
    }

    #[test]
    fn protocols_outside_the_kind_template_are_rejected() {
        for kind in [
            ProviderKind::CommandCode,
            ProviderKind::DeepSeek,
            ProviderKind::OpenRouter,
        ] {
            assert_eq!(
                parse_protocol_for(kind, "chat_completions"),
                Ok(ProviderProtocol::ChatCompletions)
            );
            for value in ["responses", "anthropic_messages", "openai-completions", ""] {
                assert_eq!(
                    parse_protocol_for(kind, value),
                    Err(ProviderError::ProtocolNotSupported)
                );
            }
        }
        for protocol in [
            ProviderProtocol::ChatCompletions,
            ProviderProtocol::Responses,
            ProviderProtocol::AnthropicMessages,
        ] {
            assert_eq!(
                ProviderProtocol::from_value(protocol.as_str()),
                Some(protocol)
            );
        }
    }

    #[test]
    fn display_names_are_trimmed_and_limited_by_characters() {
        assert_eq!(validate_display_name("  Work  ").as_deref(), Ok("Work"));
        let chinese_limit = "名".repeat(MAX_DISPLAY_NAME_CHARS);
        assert_eq!(
            validate_display_name(&chinese_limit),
            Ok(chinese_limit.clone())
        );
        let too_long = format!("{chinese_limit}名");
        for invalid in ["", "   ", too_long.as_str(), "line\nbreak", "bell\u{7}"] {
            assert_eq!(
                validate_display_name(invalid),
                Err(ProviderError::DisplayNameInvalid)
            );
        }
    }

    #[test]
    fn display_names_reject_invisible_and_bidi_characters() {
        for invalid in [
            "\u{200B}",
            "\u{200B}\u{200D}\u{200C}",
            " \u{200D} ",
            "\u{FEFF}Work",
            "Wo\u{2060}rk",
            "soft\u{00AD}hyphen",
            "a\u{202E}b",
            "a\u{202A}b",
            "a\u{2066}b\u{2069}",
            "a\u{200E}b",
            "a\u{200F}b",
            "a\u{061C}b",
        ] {
            assert_eq!(
                validate_display_name(invalid),
                Err(ProviderError::DisplayNameInvalid),
                "input: {invalid:?}"
            );
        }
        // Joiners that real text needs stay allowed next to visible characters.
        for valid in [
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} Family",
            "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
            "工作 DeepSeek",
        ] {
            assert_eq!(validate_display_name(valid).as_deref(), Ok(valid));
        }
    }

    #[test]
    fn base_urls_are_normalized_without_adding_paths() {
        for (input, expected) in [
            ("HTTPS://API.DeepSeek.com/", "https://api.deepseek.com"),
            (
                "  https://openrouter.ai/api/v1/  ",
                "https://openrouter.ai/api/v1",
            ),
            (
                "https://example.com:8443/base",
                "https://example.com:8443/base",
            ),
            ("https://例子.测试/v1", "https://xn--fsqu00a.xn--0zwm56d/v1"),
            // Only one trailing slash is removed.
            ("https://a.com/v1//", "https://a.com/v1/"),
            // Loopback and private hosts stay allowed (decided in P12).
            ("https://127.0.0.1:8443/v1", "https://127.0.0.1:8443/v1"),
            ("https://192.168.1.10/v1/", "https://192.168.1.10/v1"),
            ("\t https://example.com/v1 \n", "https://example.com/v1"),
        ] {
            assert_eq!(normalize_base_url(input).as_deref(), Ok(expected));
        }
    }

    #[test]
    fn base_urls_with_unsafe_or_unsupported_parts_are_rejected() {
        let too_long = format!("https://example.com/{}", "a".repeat(MAX_BASE_URL_LEN));
        for (input, error) in [
            ("", ProviderError::BaseUrlInvalid),
            ("not a url", ProviderError::BaseUrlInvalid),
            ("api.deepseek.com", ProviderError::BaseUrlInvalid),
            // `url` would silently delete these; vibemate rejects them instead.
            (
                "https://api.example\n.com/v1",
                ProviderError::BaseUrlInvalid,
            ),
            (
                "https://api.example.com/v\t1",
                ProviderError::BaseUrlInvalid,
            ),
            (
                "https://api.example.com/v1\r/x",
                ProviderError::BaseUrlInvalid,
            ),
            (
                "https://api.example.com/my path",
                ProviderError::BaseUrlInvalid,
            ),
            (
                "https://api.example.com/v1\u{7F}",
                ProviderError::BaseUrlInvalid,
            ),
            (too_long.as_str(), ProviderError::BaseUrlInvalid),
            (
                "https://example.com/v1?key=x",
                ProviderError::BaseUrlInvalid,
            ),
            ("https://example.com/v1#part", ProviderError::BaseUrlInvalid),
            ("http://api.deepseek.com", ProviderError::BaseUrlNotHttps),
            ("ftp://example.com", ProviderError::BaseUrlNotHttps),
            (
                "https://user:secret@example.com",
                ProviderError::BaseUrlHasCredentials,
            ),
            (
                "http://token@example.com",
                ProviderError::BaseUrlHasCredentials,
            ),
        ] {
            assert_eq!(normalize_base_url(input), Err(error), "input: {input}");
        }
    }

    #[test]
    fn any_extension_key_is_rejected_until_evidence_adds_one() {
        let mut extensions = no_extensions();
        assert_eq!(
            validate_extensions(ProviderKind::DeepSeek, &extensions),
            Ok(no_extensions())
        );
        extensions.insert("zdr".to_string(), "true".to_string());
        for kind in [
            ProviderKind::CommandCode,
            ProviderKind::DeepSeek,
            ProviderKind::OpenRouter,
        ] {
            assert_eq!(
                validate_extensions(kind, &extensions),
                Err(ProviderError::ExtensionFieldNotSupported)
            );
        }
    }

    #[test]
    fn settings_validation_reports_the_first_invalid_field_in_order() {
        let valid = validate_settings(
            ProviderKind::DeepSeek,
            " Personal ",
            "https://api.deepseek.com/",
            "chat_completions",
            &no_extensions(),
        );
        assert_eq!(
            valid,
            Ok(ProviderSettings {
                display_name: "Personal".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                protocol: ProviderProtocol::ChatCompletions,
                extensions: no_extensions(),
            })
        );
        // Both the name and the URL are invalid; the name is reported first.
        assert_eq!(
            validate_settings(
                ProviderKind::DeepSeek,
                "",
                "http://x",
                "chat_completions",
                &no_extensions()
            ),
            Err(ProviderError::DisplayNameInvalid)
        );
        assert_eq!(
            validate_settings(
                ProviderKind::DeepSeek,
                "Name",
                "https://api.deepseek.com",
                "responses",
                &no_extensions()
            ),
            Err(ProviderError::ProtocolNotSupported)
        );
    }

    #[test]
    fn provider_ids_accept_only_the_generated_format() {
        let valid = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            ProviderId::parse(valid).map(|id| id.as_str().to_string()),
            Some(valid.to_string())
        );
        for invalid in [
            "",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789abcdef0123456789abcdeg",
            "Personal",
        ] {
            assert_eq!(ProviderId::parse(invalid), None, "input: {invalid}");
        }
    }

    /// A unique temporary app-data folder, removed when the test ends.
    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("vibemate-providers-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create unique test directory");
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

    fn create_request(kind: &str, name: &str) -> CreateProviderRequest {
        CreateProviderRequest {
            kind: kind.to_string(),
            display_name: name.to_string(),
            base_url: kind_default_url(kind).to_string(),
            protocol: "chat_completions".to_string(),
            extensions: no_extensions(),
            secret: Secret::new(SYNTHETIC_KEY.to_string()),
        }
    }

    /// A made-up key that is easy to search for in files and messages.
    const SYNTHETIC_KEY: &str = "vibemate-synthetic-key-7f3a9c";

    /// Create with a throwaway in-memory credential store, for tests about SQLite.
    fn create(
        storage: &Storage,
        request: CreateProviderRequest,
        now_ms: i64,
    ) -> Result<ProviderRecord, ProviderError> {
        create_provider(storage, &FakeStore::default(), request, now_ms)
    }

    /// Count rows in `table` (a fixed name from the test, never user input).
    fn table_rows(storage: &Storage, table: &str) -> u32 {
        storage
            .lock()
            .expect("lock")
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rows")
    }

    fn kind_default_url(kind: &str) -> &'static str {
        ProviderKind::from_value(kind).map_or("https://example.com", |kind| {
            kind.template().default_base_url
        })
    }

    fn update_request(record: &ProviderRecord, name: &str, url: &str) -> UpdateProviderRequest {
        UpdateProviderRequest {
            id: record.id.as_str().to_string(),
            expected_revision: record.revision,
            display_name: name.to_string(),
            base_url: url.to_string(),
            protocol: "chat_completions".to_string(),
            extensions: no_extensions(),
        }
    }

    fn page(storage: &Storage, after: Option<String>, limit: u32) -> ProviderPage {
        list_providers(storage, &ListProvidersRequest { after, limit }).expect("list page")
    }

    fn row_count(storage: &Storage) -> u32 {
        storage
            .lock()
            .expect("lock")
            .query_row("SELECT COUNT(*) FROM provider_instance", [], |row| {
                row.get(0)
            })
            .expect("count rows")
    }

    #[test]
    fn providers_created_instance_is_read_back_after_restart() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let mut request = create_request("deepseek", "  Personal  ");
        request.base_url = "https://API.deepseek.com/".to_string();
        let created = create(&storage, request, 1_000).expect("create");
        assert_eq!(created.display_name, "Personal");
        assert_eq!(created.base_url, "https://api.deepseek.com");
        assert_eq!(created.kind, ProviderKind::DeepSeek);
        assert_eq!(
            (
                created.revision,
                created.created_at_ms,
                created.updated_at_ms
            ),
            (1, 1_000, 1_000)
        );
        drop(storage);

        let reopened = directory.open();
        assert_eq!(
            get_provider(&reopened, created.id.as_str()),
            Ok(created.clone())
        );
        assert_eq!(page(&reopened, None, 10).items, vec![created]);
    }

    #[test]
    fn providers_with_the_same_kind_and_name_are_separate_instances() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let first = create(&storage, create_request("openrouter", "Work"), 5).expect("first");
        let second = create(&storage, create_request("openrouter", "Work"), 5).expect("second");
        assert_ne!(first.id, second.id);
        assert_eq!(first.id.as_str().len(), 32);
        assert_eq!(row_count(&storage), 2);
        // Editing one instance must not touch the other, even with the same name.
        let edited = update_provider(
            &storage,
            &update_request(&first, "Work", "https://openrouter.ai/api/v2"),
            6,
        )
        .expect("edit first");
        assert_eq!(get_provider(&storage, second.id.as_str()), Ok(second));
        assert_eq!(edited.id, first.id);
    }

    #[test]
    fn providers_edit_keeps_identity_and_increments_the_revision() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created =
            create(&storage, create_request("command-code", "Team"), 100).expect("create");
        let edited = update_provider(
            &storage,
            &update_request(
                &created,
                "Renamed",
                "https://api.commandcode.ai/provider/v1/",
            ),
            200,
        )
        .expect("edit");
        assert_eq!(edited.id, created.id);
        assert_eq!(edited.kind, ProviderKind::CommandCode);
        assert_eq!(edited.display_name, "Renamed");
        assert_eq!(edited.base_url, "https://api.commandcode.ai/provider/v1");
        assert_eq!(
            (edited.revision, edited.created_at_ms, edited.updated_at_ms),
            (2, 100, 200)
        );
        drop(storage);
        assert_eq!(
            get_provider(&directory.open(), created.id.as_str()),
            Ok(edited)
        );
    }

    #[test]
    fn providers_edit_with_a_clock_before_creation_keeps_timestamps_ordered() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "A"), 500).expect("create");
        let edited = update_provider(
            &storage,
            &update_request(&created, "B", "https://api.deepseek.com"),
            10,
        )
        .expect("edit despite clock change");
        assert_eq!((edited.created_at_ms, edited.updated_at_ms), (500, 500));
    }

    #[test]
    fn providers_stale_missing_or_malformed_edits_change_nothing() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "Original"), 1).expect("create");
        let current = update_provider(
            &storage,
            &update_request(&created, "Newer", "https://api.deepseek.com"),
            2,
        )
        .expect("first edit");

        // `created` still carries revision 1, as a form opened before the first edit would.
        let stale = update_request(&created, "Stale", "https://api.deepseek.com");
        assert_eq!(
            update_provider(&storage, &stale, 3),
            Err(ProviderError::RevisionConflict)
        );

        let mut missing = update_request(&current, "Missing", "https://api.deepseek.com");
        missing.id = "f".repeat(32);
        assert_eq!(
            update_provider(&storage, &missing, 3),
            Err(ProviderError::NotFound)
        );
        assert_eq!(
            get_provider(&storage, &"f".repeat(32)),
            Err(ProviderError::NotFound)
        );

        let mut by_name = update_request(&current, "Missing", "https://api.deepseek.com");
        by_name.id = "Newer".to_string();
        assert_eq!(
            update_provider(&storage, &by_name, 3),
            Err(ProviderError::InvalidRequest)
        );
        assert_eq!(
            get_provider(&storage, "Newer"),
            Err(ProviderError::InvalidRequest)
        );

        let mut zero_revision = update_request(&current, "Zero", "https://api.deepseek.com");
        zero_revision.expected_revision = 0;
        assert_eq!(
            update_provider(&storage, &zero_revision, 3),
            Err(ProviderError::InvalidRequest)
        );

        let invalid_url = update_request(&current, "Bad URL", "http://api.deepseek.com");
        assert_eq!(
            update_provider(&storage, &invalid_url, 3),
            Err(ProviderError::BaseUrlNotHttps)
        );
        let mut wrong_protocol =
            update_request(&current, "Bad protocol", "https://api.deepseek.com");
        wrong_protocol.protocol = "responses".to_string();
        assert_eq!(
            update_provider(&storage, &wrong_protocol, 3),
            Err(ProviderError::ProtocolNotSupported)
        );
        let mut extension = update_request(&current, "Extension", "https://api.deepseek.com");
        extension
            .extensions
            .insert("zdr".to_string(), "true".to_string());
        assert_eq!(
            update_provider(&storage, &extension, 3),
            Err(ProviderError::ExtensionFieldNotSupported)
        );

        assert_eq!(get_provider(&storage, created.id.as_str()), Ok(current));
    }

    #[test]
    fn providers_edit_that_updates_no_row_is_a_conflict() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "Original"), 1).expect("create");
        // Simulate another writer winning the race between the revision check and
        // the UPDATE: this test-only trigger makes SQLite skip the row, so the
        // UPDATE reports zero changed rows just as `WHERE revision = ?` would.
        storage
            .lock()
            .expect("lock")
            .execute_batch(
                "CREATE TEMP TRIGGER skip_update BEFORE UPDATE ON provider_instance
                 BEGIN SELECT RAISE(IGNORE); END;",
            )
            .expect("trigger");
        assert_eq!(
            update_provider(
                &storage,
                &update_request(&created, "Lost", "https://api.deepseek.com"),
                2
            ),
            Err(ProviderError::RevisionConflict)
        );
        assert_eq!(get_provider(&storage, created.id.as_str()), Ok(created));
    }

    #[test]
    fn providers_create_saves_the_key_and_its_reference_together() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        let mut request = create_request("deepseek", "Keyed");
        request.secret = Secret::new(format!("  {SYNTHETIC_KEY}\n"));

        let created = create_provider(&storage, &store, request, 42).expect("create");

        let reference = format!("provider-{}", created.id.as_str());
        assert_eq!(created.id.credential_reference(), reference);
        // The key is trimmed and stored only under the ID-based entry name.
        assert_eq!(store.stored(&reference).as_deref(), Some(SYNTHETIC_KEY));
        let (stored_ref, updated_at): (String, i64) = storage
            .lock()
            .expect("lock")
            .query_row(
                "SELECT credential_ref, updated_at FROM provider_credential WHERE provider_id = ?1",
                [created.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("reference row");
        assert_eq!((stored_ref, updated_at), (reference, 42));
    }

    #[test]
    fn providers_invalid_keys_are_rejected_before_any_store_is_touched() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        for key in ["", "   ", "line\nbreak", "tab\tinside"] {
            let mut request = create_request("deepseek", "Keyed");
            request.secret = Secret::new(key.to_string());
            assert_eq!(
                create_provider(&storage, &store, request, 1),
                Err(ProviderError::SecretInvalid),
                "key: {key:?}"
            );
        }
        // Other fields are checked first, so the form shows their error first.
        let mut both_invalid = create_request("deepseek", " ");
        both_invalid.secret = Secret::new(String::new());
        assert_eq!(
            create_provider(&storage, &store, both_invalid, 1),
            Err(ProviderError::DisplayNameInvalid)
        );
        assert_eq!(store.call_count(), 0);
        assert_eq!(table_rows(&storage, "provider_instance"), 0);
    }

    #[test]
    fn providers_credential_store_failure_saves_nothing() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        for (failure, code) in [
            (
                CredentialError::Unavailable,
                ProviderError::CredentialStoreUnavailable,
            ),
            (
                CredentialError::AccessDenied,
                ProviderError::CredentialStoreAccessDenied,
            ),
            (
                CredentialError::OperationFailed,
                ProviderError::CredentialStoreFailed,
            ),
        ] {
            let store = FakeStore::default();
            store.fail_saves(Some(failure));
            assert_eq!(
                create_provider(&storage, &store, create_request("deepseek", "A"), 1),
                Err(code)
            );
        }
        assert_eq!(table_rows(&storage, "provider_instance"), 0);
        assert_eq!(table_rows(&storage, "provider_credential"), 0);
    }

    /// Make the reference insert fail, the second write in the create transaction.
    fn fail_reference_inserts(storage: &Storage) {
        storage
            .lock()
            .expect("lock")
            .execute_batch(
                "CREATE TEMP TRIGGER fail_reference BEFORE INSERT ON provider_credential
                 BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;",
            )
            .expect("trigger");
    }

    #[test]
    fn providers_database_failure_after_the_key_write_removes_the_key() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        fail_reference_inserts(&storage);

        assert_eq!(
            create_provider(&storage, &store, create_request("deepseek", "A"), 1),
            Err(ProviderError::WriteFailed)
        );
        // The instance row was inserted first; one transaction rolled both back.
        assert_eq!(table_rows(&storage, "provider_instance"), 0);
        assert_eq!(table_rows(&storage, "provider_credential"), 0);
        // save, then the compensating delete.
        assert_eq!(store.call_count(), 2);
        assert!(!store.holds_value(SYNTHETIC_KEY));
    }

    #[test]
    fn providers_failed_cleanup_after_a_database_failure_is_an_unknown_outcome() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        fail_reference_inserts(&storage);
        store.fail_deletes(Some(CredentialError::OperationFailed));

        assert_eq!(
            create_provider(&storage, &store, create_request("deepseek", "A"), 1),
            Err(ProviderError::CreateOutcomeUnknown)
        );
        assert_eq!(table_rows(&storage, "provider_instance"), 0);
        // The unreferenced entry remains; no row points at it, so it is never read.
        // The generated ID is not returned on failure, so look for the value.
        assert!(store.holds_value(SYNTHETIC_KEY));
    }

    #[test]
    fn providers_failed_commit_keeps_the_key_and_reports_an_unknown_outcome() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        fail_commits_after_reference_writes(&storage);

        assert_eq!(
            create_provider(&storage, &store, create_request("deepseek", "A"), 1),
            Err(ProviderError::CreateOutcomeUnknown)
        );
        // COMMIT failed, so no compensating delete ran: save is the only call.
        assert_eq!(store.call_count(), 1);
        assert!(store.holds_value(SYNTHETIC_KEY));
        // In this simulation SQLite rolled back; the kept entry is unreferenced.
        assert_eq!(table_rows(&storage, "provider_instance"), 0);
    }

    #[test]
    fn providers_key_never_reaches_the_database_file_or_debug_output() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let request = create_request("deepseek", "Searchable-name");
        assert!(!format!("{request:?}").contains(SYNTHETIC_KEY));
        create(&storage, request, 1).expect("create");
        drop(storage);

        let mut saw_name = false;
        for entry in fs::read_dir(&directory.0).expect("list app-data folder") {
            let bytes = fs::read(entry.expect("entry").path()).expect("read file");
            assert!(
                !bytes
                    .windows(SYNTHETIC_KEY.len())
                    .any(|window| window == SYNTHETIC_KEY.as_bytes()),
                "the key was written to a database file"
            );
            saw_name |= bytes
                .windows("Searchable-name".len())
                .any(|window| window == b"Searchable-name");
        }
        // Proves the scan read the real database contents.
        assert!(saw_name);
    }

    #[test]
    fn providers_invalid_creations_store_nothing() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let mut unknown_kind = create_request("custom", "Custom");
        unknown_kind.base_url = "https://example.com".to_string();
        assert_eq!(
            create(&storage, unknown_kind, 1),
            Err(ProviderError::KindNotSupported)
        );
        let mut credentials = create_request("deepseek", "Leaky");
        credentials.base_url = "https://key:secret@api.deepseek.com".to_string();
        assert_eq!(
            create(&storage, credentials, 1),
            Err(ProviderError::BaseUrlHasCredentials)
        );
        assert_eq!(
            create(&storage, create_request("deepseek", " "), 1),
            Err(ProviderError::DisplayNameInvalid)
        );
        assert_eq!(row_count(&storage), 0);
    }

    #[test]
    fn providers_pages_are_bounded_and_ordered_by_creation_then_id() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        assert_eq!(
            page(&storage, None, 3),
            ProviderPage {
                items: Vec::new(),
                next_cursor: None
            }
        );
        // Three instances share one millisecond, so only the ID can order them.
        let mut expected: Vec<ProviderRecord> = [20, 10, 10, 10, 30]
            .into_iter()
            .map(|now| create(&storage, create_request("deepseek", "Same"), now).expect("create"))
            .collect();
        expected.sort_by(|left, right| {
            (left.created_at_ms, left.id.as_str()).cmp(&(right.created_at_ms, right.id.as_str()))
        });

        let mut seen = Vec::new();
        let mut cursor = None;
        let mut page_sizes = Vec::new();
        loop {
            let current = page(&storage, cursor, 2);
            page_sizes.push(current.items.len());
            seen.extend(current.items);
            match current.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(page_sizes, vec![2, 2, 1]);
        assert_eq!(seen, expected);

        // A page that exactly fits the remaining rows reports no further page.
        assert_eq!(page(&storage, None, 5).next_cursor, None);
        assert_eq!(page(&storage, None, MAX_PAGE_SIZE).items.len(), 5);
    }

    #[test]
    fn providers_instances_added_between_pages_are_not_repeated() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        for now in [1, 2, 3] {
            create(&storage, create_request("openrouter", "Item"), now).expect("create");
        }
        let first = page(&storage, None, 2);
        // An older instance (another window with a slow clock) and a newer one.
        create(&storage, create_request("openrouter", "Older"), 0).expect("older");
        let newer = create(&storage, create_request("openrouter", "Newer"), 9).expect("newer");
        let second = page(&storage, first.next_cursor, 2);
        let first_ids: Vec<_> = first.items.iter().map(|record| record.id.clone()).collect();
        assert!(
            second
                .items
                .iter()
                .all(|record| !first_ids.contains(&record.id))
        );
        assert_eq!(second.items.last(), Some(&newer));
        assert_eq!(second.items.len(), 2);
    }

    #[test]
    fn providers_page_size_and_cursor_are_validated() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let valid_id = "a".repeat(32);
        for (after, limit) in [
            (None, 0),
            (None, MAX_PAGE_SIZE + 1),
            (Some(String::new()), 10),
            (Some("12".to_string()), 10),
            (Some(format!("x.{valid_id}")), 10),
            (Some("12.Personal".to_string()), 10),
            (Some(format!("+5.{valid_id}")), 10),
            (Some(format!("-5.{valid_id}")), 10),
            (Some(format!(" 5.{valid_id}")), 10),
            (Some(format!("５.{valid_id}")), 10),
            (Some(format!(".{valid_id}")), 10),
            (Some(format!("99999999999999999999.{valid_id}")), 10),
        ] {
            assert_eq!(
                list_providers(&storage, &ListProvidersRequest { after, limit }),
                Err(ProviderError::InvalidRequest)
            );
        }
        assert!(
            list_providers(
                &storage,
                &ListProvidersRequest {
                    after: Some(format!("12.{valid_id}")),
                    limit: 1
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn providers_invalid_stored_rows_are_reported_and_left_unchanged() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "Kept"), 1).expect("create");
        storage
            .lock()
            .expect("lock")
            .execute_batch("PRAGMA ignore_check_constraints = ON; UPDATE provider_instance SET kind = 'future-kind'; PRAGMA ignore_check_constraints = OFF;")
            .expect("simulate a row from a newer build");
        assert_eq!(
            get_provider(&storage, created.id.as_str()),
            Err(ProviderError::InvalidStoredProvider)
        );
        assert_eq!(
            list_providers(
                &storage,
                &ListProvidersRequest {
                    after: None,
                    limit: 10
                }
            ),
            Err(ProviderError::InvalidStoredProvider)
        );
        assert_eq!(
            update_provider(
                &storage,
                &update_request(&created, "Overwrite", "https://api.deepseek.com"),
                2
            ),
            Err(ProviderError::InvalidStoredProvider)
        );
        let kind: String = storage
            .lock()
            .expect("lock")
            .query_row("SELECT kind FROM provider_instance", [], |row| row.get(0))
            .expect("stored kind");
        assert_eq!(kind, "future-kind");
    }

    #[test]
    fn providers_failed_writes_keep_the_previous_data() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "Before"), 1).expect("create");
        // query_only makes SQLite reject writes without platform-specific permissions.
        storage
            .lock()
            .expect("lock")
            .pragma_update(None, "query_only", true)
            .expect("read-only");
        assert_eq!(
            create(&storage, create_request("deepseek", "Second"), 2),
            Err(ProviderError::WriteFailed)
        );
        assert_eq!(
            update_provider(
                &storage,
                &update_request(&created, "After", "https://api.deepseek.com"),
                2
            ),
            Err(ProviderError::WriteFailed)
        );
        assert_eq!(get_provider(&storage, created.id.as_str()), Ok(created));
        assert_eq!(row_count(&storage), 1);
    }

    #[test]
    fn providers_poisoned_storage_rejects_every_operation() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let created = create(&storage, create_request("deepseek", "A"), 1).expect("create");
        std::thread::scope(|scope| {
            let result = scope
                .spawn(|| {
                    let _connection = storage.lock().expect("lock");
                    panic!("synthetic interrupted operation");
                })
                .join();
            assert!(result.is_err());
        });
        let unavailable = Some(ProviderError::StorageUnavailable);
        assert_eq!(
            create(&storage, create_request("deepseek", "B"), 2).err(),
            unavailable
        );
        assert_eq!(
            update_provider(
                &storage,
                &update_request(&created, "C", "https://api.deepseek.com"),
                2
            )
            .err(),
            unavailable
        );
        assert_eq!(
            get_provider(&storage, created.id.as_str()).err(),
            unavailable
        );
        assert_eq!(
            list_providers(
                &storage,
                &ListProvidersRequest {
                    after: None,
                    limit: 1
                }
            )
            .err(),
            unavailable
        );
    }

    #[test]
    fn providers_schema_rejects_rows_that_bypass_validation() {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let connection = storage.lock().expect("lock");
        let valid_id = "b".repeat(32);
        for (id, kind, protocol, revision) in [
            ("Personal", "deepseek", "chat_completions", 1),
            (valid_id.as_str(), "custom", "chat_completions", 1),
            (valid_id.as_str(), "deepseek", "openai-completions", 1),
            (valid_id.as_str(), "deepseek", "chat_completions", 0),
        ] {
            assert!(
                connection
                    .execute(
                        "INSERT INTO provider_instance VALUES (?1, ?2, 'Name', 'https://x', ?3, ?4, 1, 1)",
                        params![id, kind, protocol, revision],
                    )
                    .is_err(),
                "row: {id} {kind} {protocol} {revision}"
            );
        }
    }

    #[test]
    fn providers_update_request_rejects_a_kind_field() {
        use serde::de::value::{Error, MapDeserializer};
        // A kind sent with an edit must fail loudly instead of being ignored.
        let input = MapDeserializer::<_, Error>::new(vec![("kind", "openrouter")].into_iter());
        let error = UpdateProviderRequest::deserialize(input).expect_err("kind is not editable");
        assert!(
            error.to_string().contains("unknown field `kind`"),
            "{error}"
        );
    }
}
