//! Pi's saved thinking level for one model: which levels Pi accepts and which
//! level vibemate may write to `settings.json`.
//!
//! Pi keeps the saved default in `settings.modelThinkingLevels["<provider>/<modelId>"]`
//! and silently clamps a level its model does not accept. vibemate therefore saves a
//! level only when the central model declares it and the merged `models.json`
//! definition accepts it. This module never writes a `thinkingLevelMap`, never
//! converts a level into a token budget and never guesses a provider value.

use crate::config_plan::PlanError;
use crate::model_catalog::{CapabilityState, EffortLevel, ThinkingEffort};
use crate::models::ProviderModel;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Pi's thinking levels, using the same tokens as its settings and model schemas.
///
/// Variant order matches Pi's own ranking, so [`Self::ALL`] can be filtered in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PiThinkingLevel {
    /// Reasoning disabled.
    Off,
    /// The smallest reasoning allocation above off.
    Minimal,
    /// Small reasoning allocation.
    Low,
    /// Moderate reasoning allocation.
    Medium,
    /// Large reasoning allocation.
    High,
    /// Needs an explicit Pi `thinkingLevelMap` entry before it is accepted.
    XHigh,
    /// Needs an explicit Pi `thinkingLevelMap` entry before it is accepted.
    Max,
}

impl PiThinkingLevel {
    /// Every Pi level in Pi's ranking order.
    pub const ALL: [Self; 7] = [
        Self::Off,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::XHigh,
        Self::Max,
    ];

    /// The exact token Pi stores in `settings.json` and `thinkingLevelMap`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    /// Whether Pi accepts this level for a reasoning model with no map entry.
    ///
    /// `xhigh` and `max` are excluded: Pi accepts them only after a model maps
    /// them to a provider value, and vibemate does not write such maps.
    pub const fn accepted_without_mapping(self) -> bool {
        matches!(
            self,
            Self::Off | Self::Minimal | Self::Low | Self::Medium | Self::High
        )
    }

    /// The central catalog token that declares this level.
    ///
    /// The catalog calls "no reasoning" `none`; Pi calls the same state `off`.
    /// Every other token has the same spelling in both places.
    const fn to_effort(self) -> EffortLevel {
        match self {
            Self::Off => EffortLevel::Off,
            Self::Minimal => EffortLevel::Minimal,
            Self::Low => EffortLevel::Low,
            Self::Medium => EffortLevel::Medium,
            Self::High => EffortLevel::High,
            Self::XHigh => EffortLevel::XHigh,
            Self::Max => EffortLevel::Max,
        }
    }
}

/// Levels Pi accepts for one merged model definition, in Pi's ranking order.
///
/// This follows Pi 1.3.0's `getSupportedThinkingLevels`, checked against the
/// installed bundle: without `reasoning: true` only `off` is accepted. A reasoning
/// model accepts every level except one whose `thinkingLevelMap` entry is `null`,
/// and `xhigh`/`max` also need a string entry.
///
/// A non-boolean `reasoning`, a non-object map or a non-string entry is refused
/// instead of guessed.
///
/// # Errors
/// Returns `TargetConflict` for a value of an unexpected shape.
pub fn accepted_levels(definition: &Map<String, Value>) -> Result<Vec<PiThinkingLevel>, PlanError> {
    let reasoning = match definition.get("reasoning") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(PlanError::TargetConflict),
    };
    if !reasoning {
        return Ok(vec![PiThinkingLevel::Off]);
    }
    let map = match definition.get("thinkingLevelMap") {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) => Some(map),
        Some(_) => return Err(PlanError::TargetConflict),
    };
    let mut levels = Vec::new();
    for level in PiThinkingLevel::ALL {
        let accepted = match map.and_then(|map| map.get(level.as_str())) {
            None => level.accepted_without_mapping(),
            Some(Value::Null) => false,
            Some(Value::String(_)) => true,
            Some(_) => return Err(PlanError::TargetConflict),
        };
        if accepted {
            levels.push(level);
        }
    }
    Ok(levels)
}

/// Check a requested level against central declarations and Pi's accepted set.
///
/// The level must be the effective `supported` thinking state, listed by the
/// model's own effort statement, and accepted by the merged definition. Any
/// failure returns `ThinkingLevelUnavailable` before a file or credential is read.
///
/// # Errors
/// Returns `ThinkingLevelUnavailable` when any of the three checks fails.
pub fn validate_selection(
    model: &ProviderModel,
    level: PiThinkingLevel,
    accepted: &[PiThinkingLevel],
) -> Result<(), PlanError> {
    let thinking = model
        .capability_overrides
        .thinking
        .map(|declared| declared.as_state())
        .unwrap_or(model.capabilities.thinking.state);
    let declared = matches!(
        &model.thinking_effort,
        Some(ThinkingEffort::Listed { levels, .. }) if levels.contains(&level.to_effort())
    );
    if thinking != CapabilityState::Supported || !declared || !accepted.contains(&level) {
        return Err(PlanError::ThinkingLevelUnavailable);
    }
    Ok(())
}

/// Read the saved-level token from a complete reviewed Pi plan.
///
/// A missing or `null` value means "do not overwrite". Any other value must be
/// one of Pi's level tokens; the caller rebuilds the whole plan before trusting it.
///
/// # Errors
/// Returns `InvalidRequest` for a value that is not a Pi level token.
pub(crate) fn reviewed_level(reviewed: &Value) -> Result<Option<PiThinkingLevel>, PlanError> {
    match reviewed.get("selectedThinkingLevel") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|_| PlanError::InvalidRequest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn accepted(definition: Value) -> Vec<&'static str> {
        let map = definition.as_object().unwrap();
        accepted_levels(map)
            .unwrap()
            .into_iter()
            .map(PiThinkingLevel::as_str)
            .collect()
    }

    #[test]
    fn without_reasoning_only_off_is_accepted() {
        assert_eq!(accepted(json!({})), ["off"]);
        assert_eq!(accepted(json!({"reasoning": false})), ["off"]);
    }

    #[test]
    fn reasoning_accepts_the_mapping_free_levels_and_keeps_xhigh_and_max_out() {
        assert_eq!(
            accepted(json!({"reasoning": true})),
            ["off", "minimal", "low", "medium", "high"]
        );
        assert_eq!(
            accepted(json!({"reasoning": true, "thinkingLevelMap": null})),
            ["off", "minimal", "low", "medium", "high"]
        );
    }

    #[test]
    fn a_string_map_entry_enables_xhigh_or_max_and_null_removes_a_level() {
        assert!(
            accepted(json!({"reasoning": true, "thinkingLevelMap": {"xhigh": "max"}}))
                .contains(&"xhigh")
        );
        assert!(
            accepted(json!({"reasoning": true, "thinkingLevelMap": {"max": "max"}}))
                .contains(&"max")
        );
        assert_eq!(
            accepted(json!({"reasoning": true, "thinkingLevelMap": {"high": null, "off": null}})),
            ["minimal", "low", "medium"]
        );
    }

    #[test]
    fn unexpected_shapes_are_refused_instead_of_guessed() {
        for definition in [
            json!({"reasoning": "yes"}),
            json!({"reasoning": true, "thinkingLevelMap": "high"}),
            json!({"reasoning": true, "thinkingLevelMap": {"high": 1}}),
        ] {
            assert_eq!(
                accepted_levels(definition.as_object().unwrap()),
                Err(PlanError::TargetConflict),
                "{definition}"
            );
        }
    }

    #[test]
    fn reviewed_level_reads_only_known_tokens_and_absence_means_inherit() {
        assert_eq!(reviewed_level(&json!({})), Ok(None));
        assert_eq!(
            reviewed_level(&json!({"selectedThinkingLevel": null})),
            Ok(None)
        );
        assert_eq!(
            reviewed_level(&json!({"selectedThinkingLevel": "high"})),
            Ok(Some(PiThinkingLevel::High))
        );
        assert_eq!(
            reviewed_level(&json!({"selectedThinkingLevel": "none"})),
            Err(PlanError::InvalidRequest)
        );
    }

    #[test]
    fn serde_tokens_match_the_settings_schema_tokens() {
        for level in PiThinkingLevel::ALL {
            assert_eq!(
                serde_json::to_value(level).unwrap(),
                Value::String(level.as_str().to_owned())
            );
        }
    }
}
