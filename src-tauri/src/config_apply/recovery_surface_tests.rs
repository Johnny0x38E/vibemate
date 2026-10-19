//! Agent-scoped recovery entry tests using synthetic configurations and real SQLite.

use super::*;
use crate::agents::{self, recovery::*};

fn interrupt(fixture: &Fixture, kind: OperationKind, legacy_result: bool) -> String {
    fs::write(fixture.root.join("a"), b"synthetic-original-private").unwrap();
    fixture.sql(if legacy_result {
        "CREATE TRIGGER refuse_surface_result BEFORE UPDATE ON config_operation_step WHEN NEW.state = 'applied' BEGIN SELECT RAISE(ABORT, 'injected'); END;"
    } else {
        "CREATE TRIGGER refuse_surface_result BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;"
    });
    assert_eq!(
        fixture
            .apply(kind, &[fixture.file("a", b"synthetic-intended-private")])
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    fixture.sql("DROP TRIGGER refuse_surface_result;");
    fixture.revisions()[0].operation.clone()
}

fn review(fixture: &Fixture, agent: ConfigurationAgent, id: &str) -> ConfigurationResolutionReview {
    review_configuration_resolution(
        &fixture.storage,
        &ReviewResolutionRequest {
            agent,
            operation_id: id.to_owned(),
        },
        fixture.backups().root(),
    )
    .unwrap()
}

fn resolve(
    fixture: &Fixture,
    agent: ConfigurationAgent,
    review: &ConfigurationResolutionReview,
) -> Result<ConfigurationResolutionOutcome, agents::ClaudeCodeRestoreError> {
    resolve_configuration_operation(
        &fixture.storage,
        &ResolveConfigurationRequest {
            agent,
            operation_id: review.operation_id.clone(),
            reviewed_digest: review.reviewed_digest.clone(),
        },
        fixture.backups().root(),
    )
}

#[test]
fn all_three_agent_surfaces_settle_both_crash_windows_without_restoring_and_require_a_separate_restore()
 {
    for (agent, kind) in [
        (
            ConfigurationAgent::ClaudeCode,
            OperationKind::ClaudeCodeSettings,
        ),
        (ConfigurationAgent::Pi, OperationKind::PiSettings),
        (ConfigurationAgent::GrokBuild, OperationKind::GrokSettings),
    ] {
        for legacy_result in [false, true] {
            let fixture = Fixture::new();
            let id = interrupt(&fixture, kind, legacy_result);
            let listed = agents::list_config_operations(&fixture.storage, kind).unwrap();
            assert!(listed[0].requires_resolution);
            assert!(!listed[0].restorable);
            let observed = review(&fixture, agent, &id);
            assert!(observed.resolvable);
            assert_eq!(observed.revisions.len(), 1);
            let serialized = serde_json::to_string(&observed).unwrap();
            assert!(!serialized.contains("synthetic-original-private"));
            assert!(!serialized.contains("synthetic-intended-private"));
            assert!(!serialized.contains("backupPath"));
            let result = resolve(&fixture, agent, &observed).unwrap();
            assert_eq!(result.operation_id, id);
            assert_eq!(result.state, OperationState::Applied);
            assert_eq!(
                fs::read(fixture.root.join("a")).unwrap(),
                b"synthetic-intended-private"
            );
            assert_eq!(fixture.revisions().len(), 1);
            assert_eq!(
                resolve(&fixture, agent, &observed).unwrap_err(),
                agents::ClaudeCodeRestoreError::PlanStale
            );
            let listed = agents::list_config_operations(&fixture.storage, kind).unwrap();
            assert!(!listed[0].requires_resolution);
            assert!(listed[0].restorable);
            fixture.restore(&id).unwrap();
            assert_eq!(
                fs::read(fixture.root.join("a")).unwrap(),
                b"synthetic-original-private"
            );
        }
    }
}

#[test]
fn unsupported_agents_unknown_fields_paths_and_malformed_requests_are_refused() {
    let id = "a".repeat(32);
    for value in [
        serde_json::json!({"agent":"codex", "operationId":id}),
        serde_json::json!({"agent":"pi", "operationId":id, "backupRoot":"/elsewhere"}),
        serde_json::json!({"agent":"pi"}),
    ] {
        assert!(serde_json::from_value::<ReviewResolutionRequest>(value).is_err());
    }
    assert!(
        serde_json::from_value::<ResolveConfigurationRequest>(serde_json::json!({
            "agent":"pi", "operationId":id, "reviewedDigest":"a".repeat(64), "revisions":[],
        }))
        .is_err()
    );
    let fixture = Fixture::new();
    assert_eq!(
        review_configuration_resolution(
            &fixture.storage,
            &ReviewResolutionRequest {
                agent: ConfigurationAgent::Pi,
                operation_id: "../config".to_owned(),
            },
            fixture.backups().root()
        )
        .unwrap_err(),
        agents::ClaudeCodeRestoreError::InvalidRequest
    );
    assert_eq!(
        resolve_configuration_operation(
            &fixture.storage,
            &ResolveConfigurationRequest {
                agent: ConfigurationAgent::Pi,
                operation_id: id,
                reviewed_digest: "not-a-digest".to_owned(),
            },
            fixture.backups().root()
        )
        .unwrap_err(),
        agents::ClaudeCodeRestoreError::InvalidRequest
    );
}

#[test]
fn another_agents_operation_cannot_be_inspected_or_resolved() {
    let fixture = Fixture::new();
    let id = interrupt(&fixture, OperationKind::PiSettings, false);
    let observed = review(&fixture, ConfigurationAgent::Pi, &id);
    for agent in [
        ConfigurationAgent::ClaudeCode,
        ConfigurationAgent::GrokBuild,
    ] {
        assert_eq!(
            review_configuration_resolution(
                &fixture.storage,
                &ReviewResolutionRequest {
                    agent,
                    operation_id: id.clone(),
                },
                fixture.backups().root()
            )
            .unwrap_err(),
            agents::ClaudeCodeRestoreError::OperationNotFound
        );
        assert_eq!(
            resolve(&fixture, agent, &observed).unwrap_err(),
            agents::ClaudeCodeRestoreError::OperationNotFound
        );
    }
    assert_eq!(fixture.revisions()[0].state, "prepared");
}

#[test]
fn changed_files_source_metadata_and_backups_invalidate_the_review_digest() {
    for change in ["target", "step", "backup"] {
        let fixture = Fixture::new();
        let id = interrupt(&fixture, OperationKind::PiSettings, false);
        let observed = review(&fixture, ConfigurationAgent::Pi, &id);
        match change {
            "target" => fs::write(fixture.root.join("a"), b"external").unwrap(),
            "step" => fixture.sql("UPDATE config_operation_step SET restore_state = 'failed', state = 'applied', applied_digest = expected_digest, previous_existed = 1;"),
            "backup" => fs::remove_file(fixture.revisions()[0].backup.as_ref().unwrap()).unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            resolve(&fixture, ConfigurationAgent::Pi, &observed).unwrap_err(),
            agents::ClaudeCodeRestoreError::PlanStale
        );
        assert_eq!(fixture.revisions()[0].state, "prepared");
    }
}

#[test]
fn blocked_observations_and_empty_reviews_offer_no_confirmation_and_do_not_change_files() {
    for unavailable in [false, true] {
        let fixture = Fixture::new();
        let id = interrupt(&fixture, OperationKind::PiSettings, false);
        if unavailable {
            fs::remove_file(fixture.revisions()[0].backup.as_ref().unwrap()).unwrap();
        } else {
            fs::write(fixture.root.join("a"), b"external").unwrap();
        }
        let observed = review(&fixture, ConfigurationAgent::Pi, &id);
        assert!(!observed.resolvable);
        assert_eq!(
            resolve(&fixture, ConfigurationAgent::Pi, &observed).unwrap_err(),
            agents::ClaudeCodeRestoreError::OperationFailed
        );
        assert_eq!(fixture.revisions()[0].state, "prepared");
    }
    let fixture = Fixture::new();
    let applied = fixture
        .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
        .unwrap();
    let observed = review(&fixture, ConfigurationAgent::Pi, &applied.operation_id);
    assert!(observed.revisions.is_empty());
    assert!(!observed.resolvable);
    assert_eq!(
        resolve(&fixture, ConfigurationAgent::Pi, &observed).unwrap_err(),
        agents::ClaudeCodeRestoreError::PlanStale
    );
}
