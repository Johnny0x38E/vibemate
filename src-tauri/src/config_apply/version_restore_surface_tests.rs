//! Strict version-restore IPC boundary tests with real SQLite and temp files.

use super::*;
use crate::agents::{
    ClaudeCodeRestoreError, DiscoveryEnvironment,
    recovery::ConfigurationAgent,
    version_restore::{
        ApplyVersionRestoreRequest, VersionRestoreRequest, apply_version_restore,
        plan_version_restore,
    },
};
use crate::config_apply::{OperationKind, ReviewedVersionFile, VersionRestoreAction};
use serde_json::json;

fn environment(fixture: &Fixture) -> DiscoveryEnvironment {
    DiscoveryEnvironment {
        home: fixture.root.join("home-with-no-installed-agents"),
        ..DiscoveryEnvironment::default()
    }
}

fn apply_at(
    fixture: &Fixture,
    kind: OperationKind,
    root: &std::path::Path,
    content: &[u8],
) -> String {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("settings.json"), b"synthetic-before").unwrap();
    let target = read_target_bytes(&root.join("settings.json")).unwrap().0;
    fixture
        .apply(
            kind,
            &[PlannedFile {
                target: WriteTarget::from_plan(&target),
                content: content.to_vec(),
            }],
        )
        .unwrap()
        .operation_id
}

#[test]
fn the_surface_previews_and_applies_one_reviewed_entry() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, b"first");
    apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"second",
    );
    let request = VersionRestoreRequest {
        agent: ConfigurationAgent::ClaudeCode,
        entry_key: format!("operation:{first}"),
    };
    let plan =
        plan_version_restore(&fixture.storage, &environment, &request, &fixture.backups()).unwrap();
    assert!(plan.restorable);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Restore);
    let serialized = serde_json::to_string(&plan).unwrap();
    assert!(!serialized.contains("synthetic-"));
    assert!(!serialized.contains("backupPath"));
    assert!(!serialized.contains(fixture.backups().root().to_str().unwrap()));

    let reviewed: Vec<_> = plan
        .files
        .iter()
        .map(|file| ReviewedVersionFile {
            revision_id: file.revision_id.clone().expect("revision"),
            current_digest: file.current_digest.clone(),
        })
        .collect();
    let outcome = apply_version_restore(
        &fixture.storage,
        &environment,
        &ApplyVersionRestoreRequest {
            agent: ConfigurationAgent::ClaudeCode,
            entry_key: request.entry_key.clone(),
            reviewed,
        },
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(outcome.files.len(), 1);
    assert_eq!(outcome.entry_key, request.entry_key);
    assert_eq!(
        fs::read(root.join("settings.json")).unwrap(),
        b"synthetic-before"
    );
}

#[test]
fn a_stale_review_and_out_of_scope_entry_are_refused() {
    let fixture = Fixture::new();
    let mut environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, b"first");
    apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"second",
    );
    let entry_key = format!("operation:{first}");
    let plan = plan_version_restore(
        &fixture.storage,
        &environment,
        &VersionRestoreRequest {
            agent: ConfigurationAgent::ClaudeCode,
            entry_key: entry_key.clone(),
        },
        &fixture.backups(),
    )
    .unwrap();
    let reviewed: Vec<_> = plan
        .files
        .iter()
        .map(|file| ReviewedVersionFile {
            revision_id: file.revision_id.clone().expect("revision"),
            current_digest: file.current_digest.clone(),
        })
        .collect();
    fs::write(root.join("settings.json"), b"external").unwrap();
    assert_eq!(
        apply_version_restore(
            &fixture.storage,
            &environment,
            &ApplyVersionRestoreRequest {
                agent: ConfigurationAgent::ClaudeCode,
                entry_key: entry_key.clone(),
                reviewed,
            },
            &fixture.backups(),
        ),
        Err(ClaudeCodeRestoreError::PlanStale)
    );

    environment.claude_directory = Some(root.join("elsewhere").into_os_string());
    assert_eq!(
        plan_version_restore(
            &fixture.storage,
            &environment,
            &VersionRestoreRequest {
                agent: ConfigurationAgent::ClaudeCode,
                entry_key,
            },
            &fixture.backups(),
        ),
        Err(ClaudeCodeRestoreError::StorageUnavailable)
    );
}

#[test]
fn unsupported_agents_paths_and_unknown_fields_are_rejected() {
    for value in [
        json!({"agent":"codex","entryKey":"operation:abc"}),
        json!({"entryKey":"operation:abc"}),
        json!({"agent":"pi","entryKey":"operation:abc","path":"/arbitrary"}),
        json!({"agent":"pi","entryKey":5}),
        json!({"agent":"pi"}),
    ] {
        assert!(serde_json::from_value::<VersionRestoreRequest>(value).is_err());
    }
    let revision = "a".repeat(32);
    for value in [
        json!({"agent":"pi","entryKey":"operation:abc","reviewed":[],"extra":true}),
        json!({"agent":"pi","entryKey":"operation:abc"}),
        json!({"agent":"pi","entryKey":"operation:abc","reviewed":[{"revisionId":revision,"currentDigest":null,"path":"/x"}]}),
    ] {
        assert!(serde_json::from_value::<ApplyVersionRestoreRequest>(value).is_err());
    }
}

#[test]
fn malformed_review_tokens_are_refused_before_scope_lookup() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    for reviewed in [
        vec![ReviewedVersionFile {
            revision_id: "bad id".to_owned(),
            current_digest: None,
        }],
        vec![ReviewedVersionFile {
            revision_id: "valid-revision".to_owned(),
            current_digest: Some("short".to_owned()),
        }],
    ] {
        assert_eq!(
            apply_version_restore(
                &fixture.storage,
                &environment,
                &ApplyVersionRestoreRequest {
                    agent: ConfigurationAgent::Pi,
                    entry_key: "operation:abc".to_owned(),
                    reviewed,
                },
                &fixture.backups(),
            ),
            Err(ClaudeCodeRestoreError::InvalidRequest)
        );
    }
}

#[test]
fn empty_and_oversized_reviews_are_refused_before_the_domain() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, b"first");
    let entry_key = format!("operation:{first}");
    for reviewed in [
        Vec::new(),
        (0..17)
            .map(|index| ReviewedVersionFile {
                revision_id: format!("revision-{index}"),
                current_digest: None,
            })
            .collect(),
    ] {
        assert_eq!(
            apply_version_restore(
                &fixture.storage,
                &environment,
                &ApplyVersionRestoreRequest {
                    agent: ConfigurationAgent::ClaudeCode,
                    entry_key: entry_key.clone(),
                    reviewed,
                },
                &fixture.backups(),
            ),
            Err(ClaudeCodeRestoreError::InvalidRequest)
        );
    }
}

#[test]
fn a_malformed_or_missing_entry_is_a_safe_request_error() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    for entry_key in ["not-an-entry", "operation:short", "batch:../../outside"] {
        assert_eq!(
            plan_version_restore(
                &fixture.storage,
                &environment,
                &VersionRestoreRequest {
                    agent: ConfigurationAgent::Pi,
                    entry_key: entry_key.to_owned(),
                },
                &fixture.backups(),
            ),
            Err(ClaudeCodeRestoreError::InvalidRequest)
        );
    }
}
