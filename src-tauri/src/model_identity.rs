//! Model ID grammar and protocol route checks shared by parsing, saved rows, and browse.
//!
//! This module does not fetch, store, or interpret a vendor response. Callers pass
//! an already extracted ID or an already normalized endpoint list.

use crate::providers::{ProviderKind, ProviderProtocol};
use crate::shared::is_hidden_format_character;
use serde::Serialize;

/// Longest accepted model ID, in Unicode characters.
pub const MAX_MODEL_ID_CHARS: usize = 256;

/// Preference order when a Command Code model lists several known routes and the
/// instance preferred protocol is not among them.
const PROTOCOL_FALLBACK: [ProviderProtocol; 3] = [
    ProviderProtocol::ChatCompletions,
    ProviderProtocol::Responses,
    ProviderProtocol::AnthropicMessages,
];

/// Whether a fetched model lists a vibemate-known route (Command Code) or routes
/// do not apply (other vendors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSupport {
    /// The model lists at least one route vibemate can use.
    Supported,
    /// The model lists routes, but none match a vibemate protocol.
    Unsupported,
    /// The model does not say which routes serve it.
    Unknown,
    /// The vendor does not publish routes per model (DeepSeek, OpenRouter).
    NotApplicable,
}

impl RouteSupport {
    /// The value used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Check a model ID from a vendor or from the user.
///
/// The ID must already be trimmed, be 1 to 256 characters, and contain no
/// whitespace, control characters, or invisible format characters. Everything
/// else is allowed, including `/` (`mistral/mistral-large-4`), `.` and `:`.
pub fn is_valid_model_id(model_id: &str) -> bool {
    !model_id.is_empty()
        && model_id.chars().count() <= MAX_MODEL_ID_CHARS
        && !model_id
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || is_hidden_format_character(c))
}

/// Remove an optional `/v1` prefix from a Command Code route.
///
/// The documentation writes routes as `/v1/chat/completions`, while the live
/// list returns `/chat/completions`; both normalize to the second form.
pub fn normalize_endpoint(endpoint: &str) -> &str {
    match endpoint.strip_prefix("/v1") {
        Some(rest) if rest.starts_with('/') => rest,
        _ => endpoint,
    }
}

/// The normalized route that serves a protocol.
pub fn protocol_endpoint(protocol: ProviderProtocol) -> &'static str {
    match protocol {
        ProviderProtocol::ChatCompletions => "/chat/completions",
        ProviderProtocol::Responses => "/responses",
        ProviderProtocol::AnthropicMessages => "/messages",
    }
}

/// True when the normalized endpoint list contains the route for `protocol`.
fn endpoints_include(endpoints: &[String], protocol: ProviderProtocol) -> bool {
    let wanted = protocol_endpoint(protocol);
    endpoints.iter().any(|endpoint| endpoint == wanted)
}

/// Decide whether a fetched model can be selected for an instance.
///
/// Vendors that publish per-model routes (Command Code today): any vibemate-known
/// route is enough. The instance protocol is only a preference when resolving
/// which route to use; see [`resolved_protocol`]. Other vendors do not publish
/// routes per model (`NotApplicable`). `supported_endpoints` must already be
/// normalized.
pub fn route_support(kind: ProviderKind, supported_endpoints: Option<&[String]>) -> RouteSupport {
    if kind != ProviderKind::CommandCode {
        return RouteSupport::NotApplicable;
    }
    match supported_endpoints {
        None => RouteSupport::Unknown,
        Some(endpoints) => {
            if PROTOCOL_FALLBACK
                .iter()
                .any(|protocol| endpoints_include(endpoints, *protocol))
            {
                RouteSupport::Supported
            } else {
                RouteSupport::Unsupported
            }
        }
    }
}

/// Protocol to use for a model on this instance (P15).
///
/// DeepSeek and OpenRouter always use the instance protocol. Command Code picks
/// the instance preference when the model lists that route; otherwise the first
/// known route in [`PROTOCOL_FALLBACK`] order. Missing or unknown routes yield
/// `None`.
pub fn resolved_protocol(
    kind: ProviderKind,
    preferred: ProviderProtocol,
    supported_endpoints: Option<&[String]>,
) -> Option<ProviderProtocol> {
    if kind != ProviderKind::CommandCode {
        return Some(preferred);
    }
    let endpoints = supported_endpoints?;
    if endpoints_include(endpoints, preferred) {
        return Some(preferred);
    }
    PROTOCOL_FALLBACK
        .into_iter()
        .find(|protocol| endpoints_include(endpoints, *protocol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_code_any_known_route_is_selectable() {
        let chat_and_responses = ["/chat/completions".to_owned(), "/responses".to_owned()];
        let messages_only = ["/messages".to_owned()];
        let unknown_only = ["/other".to_owned()];

        assert_eq!(
            route_support(ProviderKind::CommandCode, Some(&chat_and_responses)),
            RouteSupport::Supported
        );
        assert_eq!(
            route_support(ProviderKind::CommandCode, Some(&messages_only)),
            RouteSupport::Supported
        );
        assert_eq!(
            route_support(ProviderKind::CommandCode, Some(&unknown_only)),
            RouteSupport::Unsupported
        );
        assert_eq!(
            route_support(ProviderKind::CommandCode, None),
            RouteSupport::Unknown
        );
        assert_eq!(
            route_support(ProviderKind::DeepSeek, None),
            RouteSupport::NotApplicable
        );
    }

    #[test]
    fn resolved_protocol_prefers_instance_then_fallback_order() {
        let both = ["/chat/completions".to_owned(), "/responses".to_owned()];
        let messages = ["/messages".to_owned()];

        assert_eq!(
            resolved_protocol(
                ProviderKind::CommandCode,
                ProviderProtocol::Responses,
                Some(&both),
            ),
            Some(ProviderProtocol::Responses)
        );
        assert_eq!(
            resolved_protocol(
                ProviderKind::CommandCode,
                ProviderProtocol::AnthropicMessages,
                Some(&both),
            ),
            Some(ProviderProtocol::ChatCompletions)
        );
        assert_eq!(
            resolved_protocol(
                ProviderKind::CommandCode,
                ProviderProtocol::ChatCompletions,
                Some(&messages),
            ),
            Some(ProviderProtocol::AnthropicMessages)
        );
        assert_eq!(
            resolved_protocol(
                ProviderKind::CommandCode,
                ProviderProtocol::ChatCompletions,
                None,
            ),
            None
        );
        assert_eq!(
            resolved_protocol(
                ProviderKind::OpenRouter,
                ProviderProtocol::ChatCompletions,
                None,
            ),
            Some(ProviderProtocol::ChatCompletions)
        );
    }
}
