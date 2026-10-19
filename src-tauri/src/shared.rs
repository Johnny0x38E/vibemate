//! Small domain-neutral text, identity, and clock helpers. Business modules map
//! failures into their own stable error codes; this module owns no persistence.

use std::time::{SystemTime, UNIX_EPOCH};

/// Text violates the caller's length or visibility constraints.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvalidText;

/// Trim a display name and reject empty, hidden, control, or overlong content.
/// Joiners inside natural text and emoji remain accepted.
pub(crate) fn validate_display_name(input: &str, max_chars: usize) -> Result<String, InvalidText> {
    let name = input.trim();
    let length = name.chars().count();
    if length == 0 || length > max_chars {
        return Err(InvalidText);
    }
    if name
        .chars()
        .any(|c| c.is_control() || is_hidden_format_character(c))
    {
        return Err(InvalidText);
    }
    let has_visible_character = name
        .chars()
        .any(|c| !c.is_whitespace() && !is_allowed_joiner(c));
    if !has_visible_character {
        return Err(InvalidText);
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

/// Check a bounded text field without trimming or altering its stored value.
pub(crate) fn bounded_text(value: &str, max: usize, nonempty: bool) -> bool {
    (!nonempty || !value.trim().is_empty())
        && value.chars().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() || is_hidden_format_character(c))
}

/// Prefix vibemate puts inside the parenthesised part of every model name.
pub(crate) const AGENT_MODEL_NAME_PREFIX: &str = "vibemate";

/// Longest display name vibemate writes into an Agent's model picker.
///
/// A Provider display name can hold 64 characters and a model ID 256, so the
/// combined text needs its own bound. 160 characters fit a typical picker row
/// and match the Grok alias limit.
pub(crate) const MAX_AGENT_MODEL_NAME_CHARS: usize = 160;

/// Build the display name an Agent file shows for a saved model.
///
/// The format follows the status line the Agent prints for a model:
/// `(vibemate-<Provider>) <model ID>` — for example
/// `(vibemate-Command-Code-GOAT) deepseek/deepseek-v4.1-flash`.
///
/// - Inside the parentheses the Provider display name is converted to a
///   hyphenated token, the same shape as the Provider configuration key, so the
///   marker stays one readable unit.
/// - After the closing parenthesis sits one space and the model ID unchanged.
/// - Control and invisible format characters (including bidi overrides) are
///   removed from both parts; whitespace runs collapse to one space so the name
///   stays one line.
/// - The model part is shortened with an ellipsis when the whole name would
///   exceed `max_chars`.
///
/// A reasoning level is deliberately not part of this name: it is a request
/// setting written separately, and callers append it when they write one.
///
/// The model ID itself still decides identity; this text is display only.
pub(crate) fn agent_model_display_name(
    provider_name: &str,
    model_id: &str,
    max_chars: usize,
) -> String {
    let provider = provider_token(provider_name);
    let label = format!("({AGENT_MODEL_NAME_PREFIX}-{provider})");
    let label_chars = label.chars().count();
    // One space sits between the parenthesised marker and the model ID.
    let available = max_chars.saturating_sub(label_chars + 1).max(8);
    let model = one_line(model_id);
    let model = if model.chars().count() > available {
        let mut shortened: String = model.chars().take(available.saturating_sub(1)).collect();
        shortened.push('…');
        shortened
    } else {
        model
    };
    format!("{label} {model}")
}

/// Turn a Provider display name into the hyphenated token used in the marker.
///
/// Letters and digits are kept as written; runs of other characters become one
/// hyphen. An empty result falls back to `provider`, so the marker never ends up
/// as a bare `vibemate-`.
fn provider_token(provider_name: &str) -> String {
    let mut token = String::new();
    let mut pending_hyphen = false;
    for character in provider_name.chars() {
        if character.is_control() || is_hidden_format_character(character) {
            continue;
        }
        if character.is_alphanumeric() {
            if pending_hyphen && !token.is_empty() {
                token.push('-');
            }
            pending_hyphen = false;
            token.push(character);
        } else {
            pending_hyphen = !token.is_empty();
        }
    }
    if token.is_empty() {
        String::from("provider")
    } else {
        token
    }
}

/// Collapse whitespace to single spaces and drop invisible format characters.
fn one_line(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for character in text.chars() {
        // Whitespace is checked first: a tab or newline is also a control
        // character, but it separates words and should become one space.
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if character.is_control() || is_hidden_format_character(character) {
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(character);
    }
    out
}

/// Accept exactly the lowercase 128-bit hexadecimal format generated by SQLite.
/// This checks representation only; each domain owns its identity and references.
pub(crate) fn is_random_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The system clock cannot be represented as nonnegative Unix milliseconds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvalidTimestamp;

/// Read Unix milliseconds; reject a pre-epoch clock or an i64 overflow.
/// Callers map clock failures to their existing domain errors.
pub(crate) fn current_unix_millis() -> Result<i64, InvalidTimestamp> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| InvalidTimestamp)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| InvalidTimestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_read_as_a_provider_marker_then_the_model_id() {
        assert_eq!(
            agent_model_display_name(
                "Command Code GOAT",
                "deepseek/deepseek-v4.1-flash",
                MAX_AGENT_MODEL_NAME_CHARS
            ),
            "(vibemate-Command-Code-GOAT) deepseek/deepseek-v4.1-flash"
        );
        // Punctuation and repeated separators collapse to one hyphen.
        assert_eq!(
            agent_model_display_name("  My  Provider / v2 ", "m", 64),
            "(vibemate-My-Provider-v2) m"
        );
        // A name without letters or digits still yields a usable marker.
        assert_eq!(
            agent_model_display_name("---", "m", 64),
            "(vibemate-provider) m"
        );
    }

    #[test]
    fn display_names_drop_invisible_characters_and_stay_bounded() {
        // A bidi override would make two different names look identical.
        assert_eq!(
            agent_model_display_name("a\u{202E}b", "x\u{202E}y", 64),
            "(vibemate-ab) xy"
        );
        let long_model = "m".repeat(256);
        let name = agent_model_display_name("Provider", &long_model, 60);
        assert_eq!(name.chars().count(), 60);
        assert!(name.starts_with("(vibemate-Provider) m"));
        assert!(name.ends_with('…'));
    }
}
