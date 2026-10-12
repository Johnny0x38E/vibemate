//! Built-in provider capabilities and defaults; no network access.

use super::types::*;

/// All three vibemate protocols, for vendors that document them.
const ALL_PROTOCOLS: &[ProviderProtocol] = &[
    ProviderProtocol::ChatCompletions,
    ProviderProtocol::Responses,
    ProviderProtocol::AnthropicMessages,
];

const COMMAND_CODE_BASE: &str = "https://api.commandcode.ai/provider/v1";
const COMMAND_CODE_BASES: &[&str] = &[COMMAND_CODE_BASE, COMMAND_CODE_BASE, COMMAND_CODE_BASE];

const DEEPSEEK_OPENAI_BASE: &str = "https://api.deepseek.com";
const DEEPSEEK_ANTHROPIC_BASE: &str = "https://api.deepseek.com/anthropic";
const DEEPSEEK_BASES: &[&str] = &[
    DEEPSEEK_OPENAI_BASE,
    DEEPSEEK_OPENAI_BASE,
    DEEPSEEK_ANTHROPIC_BASE,
];

const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
const OPENROUTER_BASES: &[&str] = &[OPENROUTER_BASE, OPENROUTER_BASE, OPENROUTER_BASE];

/// Built-in templates in the order of `docs/integrations/providers.md`.
///
/// Each kind lists evidence-backed protocols. The instance `protocol` is a
/// preference when a model declares several routes (or the wire protocol when
/// the vendor has no per-model routes). Entries in `protocol_base_urls` that
/// differ mean the user should keep a separate configuration for that protocol.
const TEMPLATES: [ProviderTemplate; 3] = [
    ProviderTemplate {
        kind: ProviderKind::CommandCode,
        brand_name: "Command Code",
        default_base_url: COMMAND_CODE_BASE,
        protocols: ALL_PROTOCOLS,
        protocol_base_urls: COMMAND_CODE_BASES,
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
    ProviderTemplate {
        kind: ProviderKind::DeepSeek,
        brand_name: "DeepSeek",
        default_base_url: DEEPSEEK_OPENAI_BASE,
        protocols: ALL_PROTOCOLS,
        protocol_base_urls: DEEPSEEK_BASES,
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
    ProviderTemplate {
        kind: ProviderKind::OpenRouter,
        brand_name: "OpenRouter",
        default_base_url: OPENROUTER_BASE,
        protocols: ALL_PROTOCOLS,
        protocol_base_urls: OPENROUTER_BASES,
        default_protocol: ProviderProtocol::ChatCompletions,
        extension_fields: &[],
    },
];

/// Return every built-in template. This reads constants only and cannot fail.
pub fn provider_templates() -> &'static [ProviderTemplate] {
    &TEMPLATES
}

/// Documented HTTPS base URL to suggest for `kind` + `protocol`.
///
/// Falls back to the kind's default base URL when the protocol is not listed
/// (should not happen for validated settings).
pub fn base_url_for_protocol(kind: ProviderKind, protocol: ProviderProtocol) -> &'static str {
    let template = kind.template();
    template
        .protocols
        .iter()
        .position(|entry| *entry == protocol)
        .and_then(|index| template.protocol_base_urls.get(index).copied())
        .unwrap_or(template.default_base_url)
}
