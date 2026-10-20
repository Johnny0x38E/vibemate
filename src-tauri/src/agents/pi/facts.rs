//! Model facts that a Pi plan shows beside its file changes.
//!
//! Each fact records where a central value came from and what this plan does
//! with it. The rows are built in the same branches that write the values, so
//! they cannot describe a rule the writer does not follow. They are part of the
//! reviewed plan, so the detailed preview rebuilds and compares them too.
//! No credential or document content is stored here.

use crate::model_catalog::CapabilityState;
use serde::Serialize;
use serde_json::Value;

/// The Pi setting a fact describes. The interface owns the wording for each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PiFactItem {
    /// The model context window, written to `contextWindow`.
    ContextWindow,
    /// The model output limit, written to `maxTokens`. Pi reads it as the model's limit.
    MaxOutputTokens,
    /// Reasoning support, written to `reasoning`.
    Reasoning,
    /// Image input, written to `input`.
    ImageInput,
    /// The saved per-model thinking level in the user's `settings.json`.
    ThinkingLevel,
}

/// Where a central value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PiFactSource {
    /// Saved with the model. The library does not record whether a catalog or a person entered it.
    Library,
    /// The provider catalog states this for the model.
    Catalog,
    /// The user declared this in the model settings. A declaration overrides the catalog.
    UserDeclaration,
    /// Chosen in the configuration form for this request.
    Choice,
    /// Nothing is stated, so there is no central value.
    NotStated,
}

/// What this plan does with a fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PiFactState {
    /// The plan writes `value` to `target`.
    Written,
    /// The plan writes nothing; the native value already in the file stays as it is.
    Unchanged,
    /// The plan writes nothing and the file holds no value; Pi's own default applies.
    NotStated,
}

/// One fact row in a Pi plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiModelFact {
    /// The setting this row describes.
    pub item: PiFactItem,
    /// Where the central value came from.
    pub source: PiFactSource,
    /// What the plan does with the value.
    pub state: PiFactState,
    /// The native field or setting the item maps to. Fixed for each item.
    pub target: &'static str,
    /// The value written or kept, as display text. `None` when nothing is stated.
    pub value: Option<String>,
}

impl PiModelFact {
    /// A value this plan writes to `target`. The display text comes from the same
    /// JSON value that is written, so the row cannot disagree with the file.
    pub(super) fn written(
        item: PiFactItem,
        source: PiFactSource,
        target: &'static str,
        value: &Value,
    ) -> Self {
        Self {
            item,
            source,
            state: PiFactState::Written,
            target,
            value: Some(display_text(value)),
        }
    }

    /// A fact the plan does not write. An existing native value is kept and shown;
    /// when the file holds none, Pi's own default applies.
    pub(super) fn kept_or_unstated(
        item: PiFactItem,
        source: PiFactSource,
        target: &'static str,
        existing: Option<&Value>,
    ) -> Self {
        match existing {
            Some(value) => Self {
                item,
                source,
                state: PiFactState::Unchanged,
                target,
                value: Some(display_text(value)),
            },
            None => Self {
                item,
                source,
                state: PiFactState::NotStated,
                target,
                value: None,
            },
        }
    }
}

/// Source of a reasoning or image-input state. A user declaration wins over the
/// catalog; an unknown catalog state with no declaration has no source.
pub(super) fn capability_source(declared: bool, catalog: CapabilityState) -> PiFactSource {
    if declared {
        PiFactSource::UserDeclaration
    } else if catalog != CapabilityState::Unknown {
        PiFactSource::Catalog
    } else {
        PiFactSource::NotStated
    }
}

/// Display text for a native JSON value: strings as they are, arrays joined with
/// commas, and every other value as its JSON spelling (numbers and booleans).
fn display_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(display_text)
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn display_text_matches_the_json_that_is_written() {
        assert_eq!(display_text(&json!(128000)), "128000");
        assert_eq!(display_text(&json!(true)), "true");
        assert_eq!(display_text(&json!(["text", "image"])), "text, image");
        assert_eq!(display_text(&json!("high")), "high");
    }

    #[test]
    fn a_declaration_wins_and_an_unknown_catalog_state_has_no_source() {
        assert_eq!(
            capability_source(true, CapabilityState::Unsupported),
            PiFactSource::UserDeclaration
        );
        assert_eq!(
            capability_source(false, CapabilityState::Supported),
            PiFactSource::Catalog
        );
        assert_eq!(
            capability_source(false, CapabilityState::Unknown),
            PiFactSource::NotStated
        );
    }

    #[test]
    fn a_missing_value_is_not_stated_and_an_existing_one_is_kept_with_its_text() {
        let kept = PiModelFact::kept_or_unstated(
            PiFactItem::ImageInput,
            PiFactSource::NotStated,
            "input",
            Some(&json!(["text", "image"])),
        );
        assert_eq!(kept.state, PiFactState::Unchanged);
        assert_eq!(kept.value.as_deref(), Some("text, image"));
        let absent = PiModelFact::kept_or_unstated(
            PiFactItem::ContextWindow,
            PiFactSource::NotStated,
            "contextWindow",
            None,
        );
        assert_eq!(absent.state, PiFactState::NotStated);
        assert_eq!(absent.value, None);
    }
}
