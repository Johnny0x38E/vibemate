//! Provider-specific input validation, without persistence or credential writes.

use super::types::*;
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
