//! Claude Code's explicit effort choice for the documented DeepSeek binding.
//!
//! Model capability metadata only limits the menu; it never selects a value.
//! The caller chooses a typed effort or leaves native settings untouched. No
//! effort is converted to a token count and no Provider request is made here.

use super::ModelFacts;
use crate::config_plan::PlanError;
use crate::model_catalog::{EffortLevel, ThinkingEffort};
use crate::providers::ProviderKind;
use serde_json::Value;

/// Native environment channel, including Claude Code's persistent `max` choice.
pub(super) const KEY: &str = "env.CLAUDE_CODE_EFFORT_LEVEL";

/// Validate an explicit choice against both the adapter and current model facts.
/// An unset choice preserves existing settings even when metadata is unknown.
/// Returns `InvalidRequest` before any file or credential access on refusal.
pub(super) fn validate_selection(
    provider_kind: ProviderKind,
    model: &ModelFacts,
    selected_effort: Option<EffortLevel>,
) -> Result<(), PlanError> {
    let Some(level) = selected_effort else {
        return Ok(());
    };
    let explicitly_listed = matches!(
        &model.thinking_effort,
        Some(ThinkingEffort::Listed { levels, .. }) if levels.contains(&level)
    );
    if provider_kind != ProviderKind::DeepSeek
        || model.id != "deepseek-flash"
        || !model.thinking_supported
        || !is_native_effort(level)
        || !explicitly_listed
    {
        return Err(PlanError::InvalidRequest);
    }
    Ok(())
}

/// Whether the native environment channel accepts this persistent effort token.
/// This does not claim that the current model or running session supports it.
pub(super) fn is_native_effort(level: EffortLevel) -> bool {
    matches!(
        level,
        EffortLevel::Low
            | EffortLevel::Medium
            | EffortLevel::High
            | EffortLevel::XHigh
            | EffortLevel::Max
    )
}

/// Recover the selected option from a bounded, untrusted reviewed plan.
///
/// Only this known field may supply a request option, never a path or arbitrary
/// configuration value. The caller revalidates it against stored model metadata,
/// rebuilds the whole plan and compares it before returning any detailed text.
/// Returns `InvalidRequest` for malformed/duplicate efforts or a missing array.
pub(crate) fn reviewed_effort(reviewed: &Value) -> Result<Option<EffortLevel>, PlanError> {
    let changes = reviewed
        .get("changes")
        .and_then(Value::as_array)
        .ok_or(PlanError::InvalidRequest)?;
    let mut selected = None;
    for change in changes {
        if change.get("keyPath").and_then(Value::as_str) != Some(KEY) {
            continue;
        }
        if selected.is_some() {
            return Err(PlanError::InvalidRequest);
        }
        selected = Some(
            change
                .get("after")
                .and_then(Value::as_str)
                .and_then(EffortLevel::parse)
                .ok_or(PlanError::InvalidRequest)?,
        );
    }
    Ok(selected)
}
