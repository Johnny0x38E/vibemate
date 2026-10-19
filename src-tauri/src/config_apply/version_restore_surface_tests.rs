//! Strict version-restore IPC boundary tests with real SQLite and temp files.

use super::*;
use crate::agents::{
    ClaudeCodeRestoreError, DiscoveryEnvironment,
    recovery::ConfigurationAgent,
    version_restore::{
        ApplyVersionRestoreRequest, VersionRestoreRequest, VersionTextRequest,
        apply_version_restore, plan_version_restore, read_version_text,
    },
};
use crate::config_apply::{
    ConfigFormat, OperationKind, ReviewedVersionFile, VersionRestoreAction, VersionTextSide,
};
use crate::config_plan::SECRET_PLACEHOLDER;
use serde_json::json;

const BEFORE: &[u8] = br#"{"api_key":"synthetic-old-secret","value":"before"}"#;
const FIRST: &[u8] = br#"{"api_key":"synthetic-first-secret","value":"first"}"#;
const SECOND: &[u8] = br#"{"api_key":"synthetic-second-secret","value":"second"}"#;

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
    if !root.join("settings.json").exists() {
        fs::write(root.join("settings.json"), BEFORE).unwrap();
    }
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
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, FIRST);
    apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, SECOND);
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
    assert!(!serialized.contains("synthetic-old-secret"));
    assert!(!serialized.contains("synthetic-first-secret"));
    assert!(!serialized.contains("synthetic-second-secret"));
    assert!(serialized.contains("••••••••"));
    assert!(!serialized.contains("backupPath"));
    assert!(!serialized.contains(fixture.backups().root().to_str().unwrap()));
    assert!(!plan.files[0].summary.changes.is_empty());

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
    assert_eq!(fs::read(root.join("settings.json")).unwrap(), FIRST);
}

#[test]
fn already_restored_entries_keep_present_target_tokens_and_complete_text() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let operation = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, FIRST);
    let request = VersionRestoreRequest {
        agent: ConfigurationAgent::ClaudeCode,
        entry_key: format!("operation:{operation}"),
    };
    let plan =
        plan_version_restore(&fixture.storage, &environment, &request, &fixture.backups()).unwrap();
    assert!(!plan.restorable);
    let file = &plan.files[0];
    assert_eq!(file.action, VersionRestoreAction::Skip);
    assert!(file.target_digest.is_some());
    assert_eq!(file.target_digest, file.current_digest);
    let text = read_version_text(
        &fixture.storage,
        &environment,
        &VersionTextRequest {
            agent: ConfigurationAgent::ClaudeCode,
            entry_key: request.entry_key,
            revision_id: file.revision_id.clone().unwrap(),
            current_digest: file.current_digest.clone(),
        },
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(text.current, text.target);
    let VersionTextSide::Present { text, .. } = text.target else {
        panic!("present retained file");
    };
    assert!(text.contains("first"));
    assert!(!text.contains("synthetic-first-secret"));
}

#[test]
fn an_independent_written_version_can_recreate_a_missing_live_file() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    fs::create_dir_all(&root).unwrap();
    let target = read_target_bytes(&root.join("settings.json")).unwrap().0;
    let operation = fixture
        .apply(
            OperationKind::ClaudeCodeSettings,
            &[PlannedFile {
                target: WriteTarget::from_plan(&target),
                content: FIRST.to_vec(),
            }],
        )
        .unwrap();
    fs::remove_file(root.join("settings.json")).unwrap();
    let plan = plan_version_restore(
        &fixture.storage,
        &environment,
        &VersionRestoreRequest {
            agent: ConfigurationAgent::ClaudeCode,
            entry_key: format!("operation:{}", operation.operation_id),
        },
        &fixture.backups(),
    )
    .unwrap();
    assert!(plan.restorable);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Restore);
    assert_eq!(plan.files[0].current_digest, None);
    assert!(plan.files[0].target_digest.is_some());
    assert_eq!(plan.files[0].reason, None);
}

#[test]
fn the_text_surface_returns_sanitized_current_and_retained_text() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, FIRST);
    apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, SECOND);
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
    let revision_id = plan.files[0].revision_id.clone().expect("revision");
    let mut request = VersionTextRequest {
        agent: ConfigurationAgent::ClaudeCode,
        entry_key,
        revision_id,
        current_digest: plan.files[0].current_digest.clone(),
    };
    let text =
        read_version_text(&fixture.storage, &environment, &request, &fixture.backups()).unwrap();
    assert_eq!(text.format, ConfigFormat::Json);
    let VersionTextSide::Present { text: current, .. } = text.current else {
        panic!("expected current text");
    };
    assert!(current.contains("\"second\""));
    assert!(current.contains(SECRET_PLACEHOLDER));
    assert!(!current.contains("synthetic-second-secret"));
    let VersionTextSide::Present { text: target, .. } = text.target else {
        panic!("expected target text");
    };
    assert!(target.contains("\"first\""));
    assert!(!target.contains("synthetic-first-secret"));

    request.current_digest = Some("f".repeat(64));
    assert_eq!(
        read_version_text(&fixture.storage, &environment, &request, &fixture.backups()),
        Err(ClaudeCodeRestoreError::PlanStale)
    );
    request.current_digest = plan.files[0].current_digest.clone();
    request.revision_id = "other-revision".to_owned();
    assert_eq!(
        read_version_text(&fixture.storage, &environment, &request, &fixture.backups()),
        Err(ClaudeCodeRestoreError::InvalidRequest)
    );
    for value in [
        json!({"agent":"pi","entryKey":"operation:abc","revisionId":"rev","currentDigest":null,"path":"/x"}),
        json!({"agent":"pi","entryKey":"operation:abc","revisionId":5,"currentDigest":null}),
    ] {
        assert!(serde_json::from_value::<VersionTextRequest>(value).is_err());
    }
}

#[test]
fn a_stale_review_and_out_of_scope_entry_are_refused() {
    let fixture = Fixture::new();
    let mut environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, FIRST);
    apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, SECOND);
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
    let first = apply_at(&fixture, OperationKind::ClaudeCodeSettings, &root, FIRST);
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
