//! Built-in provider capabilities and defaults; no network access.

use super::types::*;

/// Built-in templates in the order of `docs/integrations/providers.md`.
///
/// Every kind allows only Chat Completions for now. DeepSeek and OpenRouter use it
/// as the documented first path. Command Code routes by model, so a per-model
/// override is left to the model tasks (P12/P15) rather than guessed here.
pub(super) const TEMPLATES: [ProviderTemplate; 3] = [
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
