//! Declared switchable levels are injected independently of the saved default.

/// An official gateway binding with an upstream-prefixed model ID.
fn openrouter_fixture() -> Fixture {
    let mut f = Fixture::new();
    f.provider.kind = ProviderKind::OpenRouter;
    f.provider.base_url = "https://openrouter.ai/api/v1".into();
    f.model.model.model_id = "vendor/reasoning-model".into();
    declare_thinking(
        &mut f,
        &[EffortLevel::High, EffortLevel::XHigh, EffortLevel::Max],
    );
    f
}

#[test]
fn openrouter_injects_declared_gateway_values_independently_of_default() {
    let f = openrouter_fixture();
    let plan = f.plan();
    assert_eq!(plan.selected_thinking_level, None);
    f.apply(&reviewed_from(plan)).unwrap();
    let document: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    let model = &document["providers"]["vibemate-Pi-test"]["models"][0];
    assert_eq!(
        model["thinkingLevelMap"],
        json!({"xhigh":"xhigh","max":"max"})
    );
    assert_eq!(model["compat"]["thinkingFormat"], "openrouter");
    // Either explicit startup choice consumes the same full switchable map.
    for level in [PiThinkingLevel::XHigh, PiThinkingLevel::Max] {
        f.apply(&f.reviewed_with(Some(level))).unwrap();
        let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
        assert_eq!(
            settings["modelThinkingLevels"]["vibemate-Pi-test/vendor/reasoning-model"],
            level.as_str()
        );
        let after: Value = serde_json::from_str(&f.text("models.json")).unwrap();
        assert_eq!(
            after["providers"]["vibemate-Pi-test"]["models"][0]["thinkingLevelMap"],
            model["thinkingLevelMap"]
        );
    }
}

#[test]
fn openrouter_mapping_and_default_restore_original_file_bytes() {
    let f = openrouter_fixture();
    let models = "{\"providers\":{\"mine\":{\"models\":[{\"id\":\"keep\"}]}}}";
    let settings =
        "{\"defaultThinkingLevel\":\"low\",\"modelThinkingLevels\":{\"mine/keep\":\"high\"}}";
    f.write("models.json", models);
    f.write("settings.json", settings);
    let applied = f
        .apply(&f.reviewed_with(Some(PiThinkingLevel::Max)))
        .unwrap();
    let plan = crate::agents::plan_config_restore(
        &f.storage,
        &applied.operation_id,
        OperationKind::PiSettings,
    )
    .unwrap();
    let request = ApplyClaudeCodeRestoreRequest {
        operation_id: applied.operation_id,
        reviewed: plan
            .steps
            .iter()
            .map(|step| ReviewedRestoreStepRequest {
                ordinal: step.ordinal,
                current_digest: step.current_digest.clone(),
            })
            .collect(),
    };
    let restored = crate::agents::apply_config_restore(
        &f.storage,
        &request,
        &f.root.join("backups"),
        OperationKind::PiSettings,
    )
    .unwrap();
    assert_eq!(restored.state, OperationState::Compensated);
    assert_eq!(f.text("models.json"), models);
    assert_eq!(f.text("settings.json"), settings);
}

#[test]
fn openrouter_preserves_null_user_values_and_rejects_stale_approval() {
    let f = openrouter_fixture();
    f.apply(&f.reviewed()).unwrap();
    let reviewed = f.reviewed_with(Some(PiThinkingLevel::Max));
    edit_owned_model(&f, |model| {
        model["thinkingLevelMap"] = json!({"xhigh":"custom","max":null});
        model["samplingParams"] = json!({"temperature":0.7});
    });
    assert!(
        f.apply(&reviewed).is_err(),
        "an old approval cannot replace a new user exclusion"
    );
    assert_eq!(
        f.plan_with(Some(PiThinkingLevel::Max)).unwrap_err(),
        PlanError::ThinkingLevelUnavailable
    );
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::XHigh)))
        .unwrap();
    let document: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    let model = &document["providers"]["vibemate-Pi-test"]["models"][0];
    assert_eq!(
        model["thinkingLevelMap"],
        json!({"xhigh":"custom","max":null})
    );
    assert_eq!(model["samplingParams"], json!({"temperature":0.7}));
}

#[test]
fn openrouter_unverified_routes_and_unlisted_levels_are_not_expanded() {
    for variant in 0..6 {
        let mut f = openrouter_fixture();
        match variant {
            0 => f.provider.base_url = "https://openrouter.ai.example/api/v1".into(),
            1 => f.provider.base_url = "https://openrouter.ai/api".into(),
            2 => f.model.model.resolved_protocol = Some(ProviderProtocol::Responses),
            3 => f.model.model.resolved_protocol = Some(ProviderProtocol::AnthropicMessages),
            4 => {
                f.model.model.thinking_effort = Some(ThinkingEffort::Any {
                    default_level: Some(EffortLevel::Max),
                })
            }
            _ => f.model.model.thinking_effort = Some(listed(&[])),
        }
        assert!(
            !f.plan().files[0]
                .changes
                .iter()
                .any(|change| change.key_path.contains("thinkingLevelMap")),
            "variant {variant}"
        );
        assert_eq!(
            f.plan_with(Some(PiThinkingLevel::Max)).unwrap_err(),
            PlanError::ThinkingLevelUnavailable,
            "variant {variant}"
        );
    }
}

use super::*;

#[test]
fn deepseek_pro_uses_the_documented_chat_alias_without_mapping_other_ids() {
    let mut f = Fixture::new();
    f.model.model.model_id = "deepseek-v4-pro".into();
    declare_thinking(&mut f, &[EffortLevel::XHigh, EffortLevel::Max]);
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::XHigh)))
        .unwrap();
    let models: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        models["providers"]["vibemate-Pi-test"]["models"][0]["thinkingLevelMap"],
        json!({"xhigh":"high","max":"max"})
    );
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/deepseek-v4-pro"],
        "xhigh"
    );
    f.model.model.model_id = "deepseek-v4-pro[1m]".into();
    assert_eq!(
        f.plan_with(Some(PiThinkingLevel::XHigh)).unwrap_err(),
        PlanError::ThinkingLevelUnavailable
    );
}

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
                f.provider.kind = ProviderKind::CommandCode;
                f.provider.base_url = "https://api.commandcode.ai/provider/v1".into();
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
