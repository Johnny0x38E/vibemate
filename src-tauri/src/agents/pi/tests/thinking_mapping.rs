//! Declared switchable levels are injected independently of the saved default.

use super::*;

#[test]
fn injection_enables_all_declared_extended_levels_without_selecting_a_default() {
    let mut f = Fixture::new();
    declare_thinking(
        &mut f,
        &[EffortLevel::High, EffortLevel::XHigh, EffortLevel::Max],
    );
    let plan = f.plan();
    assert_eq!(plan.selected_thinking_level, None);
    assert!(
        !plan.files[1]
            .changes
            .iter()
            .any(|change| change.key_path.contains("modelThinkingLevels"))
    );
    for (level, vendor) in [("xhigh", "high"), ("max", "max")] {
        assert!(plan.files[0].changes.iter().any(|change| {
            change
                .key_path
                .ends_with(&format!(".thinkingLevelMap.{level}"))
                && change.after == vendor
        }));
    }
    f.apply(&reviewed_from(plan)).unwrap();
    let models: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    let definition = models["providers"]["vibemate-Pi-test"]["models"][0]
        .as_object()
        .unwrap();
    assert_eq!(
        definition["thinkingLevelMap"],
        json!({"xhigh":"high","max":"max"})
    );
    let accepted = thinking::accepted_levels(definition).unwrap();
    assert!(accepted.contains(&PiThinkingLevel::XHigh));
    assert!(accepted.contains(&PiThinkingLevel::Max));
    // Choosing a default later does not remove any other switchable level.
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::High)))
        .unwrap();
    let after: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        after["providers"]["vibemate-Pi-test"]["models"][0]["thinkingLevelMap"],
        definition["thinkingLevelMap"]
    );
}

#[test]
fn injection_preserves_user_maps_exclusions_and_unrelated_request_parameters() {
    let mut f = Fixture::new();
    declare_thinking(
        &mut f,
        &[EffortLevel::High, EffortLevel::XHigh, EffortLevel::Max],
    );
    f.apply(&f.reviewed()).unwrap();
    edit_owned_model(&f, |model| {
        model["thinkingLevelMap"] = json!({"xhigh":"custom-user-value","max":null});
        model["samplingParams"] =
            json!({"temperature":0.7,"top_p":0.9,"max_tokens":1234,"custom":true});
    });
    let before: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        f.plan_with(Some(PiThinkingLevel::Max)).unwrap_err(),
        PlanError::ThinkingLevelUnavailable
    );
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::High)))
        .unwrap();
    let after: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    let key = |value: &Value, field: &str| {
        value["providers"]["vibemate-Pi-test"]["models"][0][field].clone()
    };
    assert_eq!(
        key(&after, "thinkingLevelMap"),
        key(&before, "thinkingLevelMap")
    );
    assert_eq!(
        key(&after, "samplingParams"),
        key(&before, "samplingParams")
    );
}

#[test]
fn unverified_bindings_and_unlisted_levels_do_not_gain_vendor_mappings() {
    for variant in 0..6 {
        let mut f = Fixture::new();
        declare_thinking(&mut f, &[EffortLevel::XHigh, EffortLevel::Max]);
        match variant {
            0 => f.model.model.model_id = "another-model".into(),
            1 => f.provider.base_url = "https://custom.example".into(),
            2 => {
                f.provider.kind = ProviderKind::OpenRouter;
                f.provider.base_url = "https://openrouter.ai/api/v1".into();
            }
            3 => {
                f.model.model.thinking_effort = Some(ThinkingEffort::Any {
                    default_level: None,
                })
            }
            4 => {
                f.model.model.capability_overrides.thinking = Some(CapabilityOverride::Unsupported)
            }
            _ => {
                f.provider.protocol = ProviderProtocol::Responses;
                f.model.model.resolved_protocol = Some(ProviderProtocol::Responses);
            }
        }
        let plan = f.plan();
        assert!(
            !plan.files[0]
                .changes
                .iter()
                .any(|change| change.key_path.contains("thinkingLevelMap"))
        );
        assert_eq!(
            f.plan_with(Some(PiThinkingLevel::Max)).unwrap_err(),
            PlanError::ThinkingLevelUnavailable
        );
    }
}
