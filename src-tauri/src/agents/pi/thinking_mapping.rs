//! Translate declared model levels into Pi's native switchable thinking map.
//! This is model injection, not request tuning: all verified declared extended
//! levels are mapped independently of the saved default. User entries stay owned
//! by the user, including null exclusions and custom mappings.

use super::{object_entry, set_value};
use crate::config_plan::{ConfigChange, PlanError};
use crate::model_catalog::{CapabilityState, EffortLevel, ThinkingEffort};
use crate::models::ProviderModel;
use crate::providers::{ProviderKind, ProviderProtocol, ProviderRecord};
use serde_json::{Map, Value, json};

/// Add missing verified mappings for all declared extended levels, not just the
/// chosen default. Verified Chat bindings are DeepSeek flash/pro and OpenRouter's
/// gateway, where the central model list bounds the accepted gateway values.
/// Returns `TargetConflict` for a malformed owned map; no I/O or credential reads.
pub(super) fn inject_declared_levels(
    definition: &mut Map<String, Value>,
    provider: &ProviderRecord,
    model: &ProviderModel,
    protocol: ProviderProtocol,
    base: &str,
    prefix: &str,
    changes: &mut Vec<ConfigChange>,
) -> Result<(), PlanError> {
    let thinking = model
        .capability_overrides
        .thinking
        .map(|value| value.as_state())
        .unwrap_or(model.capabilities.thinking.state);
    if protocol != ProviderProtocol::ChatCompletions || thinking != CapabilityState::Supported {
        return Ok(());
    }
    // OpenRouter normalizes declared efforts at its own gateway. Unlike the
    // DeepSeek alias, xhigh is sent unchanged in Pi's nested reasoning.effort.
    // Neither contract applies to lookalike hosts, custom bases or other APIs.
    let xhigh_vendor = match provider.kind {
        ProviderKind::DeepSeek
            if matches!(
                model.model_id.as_str(),
                "deepseek-flash" | "deepseek-v4-pro"
            ) && base == "https://api.deepseek.com" =>
        {
            "high"
        }
        ProviderKind::OpenRouter if base == "https://openrouter.ai/api/v1" => "xhigh",
        _ => return Ok(()),
    };
    let Some(ThinkingEffort::Listed { levels, .. }) = &model.thinking_effort else {
        return Ok(());
    };
    // Inject only explicitly listed values, never an expansion of `any`.
    let mappings: Vec<_> = [
        (EffortLevel::XHigh, "xhigh", xhigh_vendor),
        (EffortLevel::Max, "max", "max"),
    ]
    .into_iter()
    .filter(|(level, _, _)| levels.contains(level))
    .collect();
    if mappings.is_empty() {
        return Ok(());
    }
    let mapping = object_entry(definition, "thinkingLevelMap")?;
    for (_, level, vendor) in mappings {
        if !mapping.contains_key(level) {
            set_value(
                mapping,
                level,
                json!(vendor),
                &format!("{prefix}.thinkingLevelMap"),
                false,
                changes,
            )?;
        }
    }
    Ok(())
}
