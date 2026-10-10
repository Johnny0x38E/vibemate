//! Provider-specific input validation, without persistence or credential writes.

use super::types::*;
pub(crate) use crate::shared::is_hidden_format_character;
use std::collections::BTreeMap;
use url::Url;

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
    crate::shared::validate_display_name(input, MAX_DISPLAY_NAME_CHARS)
        .map_err(|_| ProviderError::DisplayNameInvalid)
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
pub(super) fn parse_protocol_for(
    kind: ProviderKind,
    value: &str,
) -> Result<ProviderProtocol, ProviderError> {
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
pub(super) fn validate_extensions(
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
