//! Model ID grammar and protocol route checks shared by parsing, saved rows, and browse.
//!
//! This module does not fetch, store, or interpret a vendor response. Callers pass
//! an already extracted ID or an already normalized endpoint list.

use crate::providers::{ProviderKind, ProviderProtocol};
use crate::shared::is_hidden_format_character;
use serde::Serialize;

/// Longest accepted model ID, in Unicode characters.
pub const MAX_MODEL_ID_CHARS: usize = 256;

/// Whether a model can be used with the instance's protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSupport {
    /// The model lists the route of the instance's protocol.
    Supported,
    /// The model lists routes, but not the one the instance uses.
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

/// Decide whether a fetched model can be selected for an instance.
///
/// `supported_endpoints` must be the normalized list from a catalog model.
pub fn route_support(
    kind: ProviderKind,
    protocol: ProviderProtocol,
    supported_endpoints: Option<&[String]>,
) -> RouteSupport {
    if kind != ProviderKind::CommandCode {
        return RouteSupport::NotApplicable;
    }
    match supported_endpoints {
        None => RouteSupport::Unknown,
        Some(endpoints) => {
            let wanted = protocol_endpoint(protocol);
            if endpoints.iter().any(|endpoint| endpoint == wanted) {
                RouteSupport::Supported
            } else {
                RouteSupport::Unsupported
            }
        }
    }
}
