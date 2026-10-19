//! Read-only desktop history boundary tests with real SQLite and synthetic files.

use super::*;
use crate::agents::{
    ClaudeCodeRestoreError, DiscoveryEnvironment,
    history::{ConfigurationHistoryRequest, configuration_history_page},
    recovery::ConfigurationAgent,
};
use crate::config_apply::{HistoryCurrentStatus, HistoryScope, configuration_history};
use crate::credentials::{Secret, fake::FakeStore};
use crate::providers::CreateProviderRequest;
use serde_json::json;
use std::collections::BTreeMap;

fn environment(fixture: &Fixture) -> DiscoveryEnvironment {
    DiscoveryEnvironment {
        home: fixture.root.join("home-with-no-installed-agents"),
        ..DiscoveryEnvironment::default()
    }
}

fn request(agent: ConfigurationAgent, limit: u32) -> ConfigurationHistoryRequest {
    ConfigurationHistoryRequest {
        agent,
        after: None,
        limit,
    }
}

fn apply_in(
    fixture: &Fixture,
    kind: OperationKind,
    root: &std::path::Path,
    names: &[&str],
) -> String {
    fs::create_dir_all(root).unwrap();
    let files: Vec<_> = names
        .iter()
        .map(|name| {
            let path = root.join(name);
            fs::write(&path, b"synthetic-private-before").unwrap();
            let target = read_target_bytes(&path).unwrap().0;
            PlannedFile {
                target: WriteTarget::from_plan(&target),
                content: b"synthetic-private-after".to_vec(),
            }
        })
        .collect();
    fixture.apply(kind, &files).unwrap().operation_id
}

#[test]
fn history_surface_defaults_work_without_installations_targets_or_backups() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    for (agent, kind, directory, names) in [
        (
            ConfigurationAgent::ClaudeCode,
            OperationKind::ClaudeCodeSettings,
            ".claude",
            &["settings.json"][..],
        ),
        (
            ConfigurationAgent::Pi,
            OperationKind::PiSettings,
            ".pi/agent",
            &["models.json", "settings.json"][..],
        ),
        (
            ConfigurationAgent::GrokBuild,
            OperationKind::GrokSettings,
            ".grok",
            &["config.toml"][..],
        ),
    ] {
        let root = environment.home.join(directory);
        let id = apply_in(&fixture, kind, &root, names);
        fs::remove_dir_all(&root).unwrap();
        let page = configuration_history_page(
            &fixture.storage,
            &environment,
            &request(agent, 50),
            &fixture.backups(),
        )
        .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].entry.operation_id, id);
        assert_eq!(page.entries[0].entry.source_steps.len(), names.len());
        // Current pin always ships with the IPC page, even when live files are gone.
        assert_eq!(page.current.files.len(), names.len());
        assert_eq!(page.current.matched_entry_key, None);
        let serialized = serde_json::to_string(&page).unwrap();
        assert!(serialized.contains("\"current\""));
        assert!(!serialized.contains("synthetic-private"));
        assert!(!serialized.contains("backupPath"));
        assert!(!serialized.contains(fixture.backups().root().to_str().unwrap()));
    }
    fs::remove_dir_all(fixture.backups().root()).unwrap();
    // A retained journal reference is not enough: missing copies are unavailable.
    let page = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::Pi, 50),
        &fixture.backups(),
    )
    .unwrap();
    assert!(page.entries[0].entry.revisions.iter().all(|revision| {
        revision.backup_recorded && revision.backup_state == RevisionBackupState::Unavailable
    }));
    assert!(page.entries[0].entry.source_steps.iter().all(|step| {
        step.backup_recorded && step.backup_state == RevisionBackupState::Unavailable
    }));
}

#[test]
fn history_surface_overrides_isolate_roots_agents_and_unowned_files() {
    let fixture = Fixture::new();
    let mut environment = environment(&fixture);
    let root = fixture.root.join("selected");
    let other = fixture.root.join("selected-other");
    environment.claude_directory = Some(root.clone().into_os_string());
    environment.pi_directory = Some(root.clone().into_os_string());
    environment.grok_directory = Some(root.clone().into_os_string());
    for (agent, kind, names) in [
        (
            ConfigurationAgent::ClaudeCode,
            OperationKind::ClaudeCodeSettings,
            &["settings.json"][..],
        ),
        (
            ConfigurationAgent::Pi,
            OperationKind::PiSettings,
            &["settings.json", "models.json"][..],
        ),
        (
            ConfigurationAgent::GrokBuild,
            OperationKind::GrokSettings,
            &["config.toml"][..],
        ),
    ] {
        let expected = apply_in(&fixture, kind, &root, names);
        apply_in(&fixture, kind, &other, names);
        // Even matching one owned file cannot admit a mixed/unowned operation.
        apply_in(&fixture, kind, &root, &[names[0], "auth.json"]);
        let page = configuration_history_page(
            &fixture.storage,
            &environment,
            &request(agent, 50),
            &fixture.backups(),
        )
        .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].entry.operation_id, expected);
    }
}

#[test]
fn history_surface_pages_restore_batches_and_survives_restart_without_mutation() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".pi/agent");
    let mut operations = std::collections::BTreeSet::new();
    for _ in 0..12 {
        operations.insert(apply_in(
            &fixture,
            OperationKind::PiSettings,
            &root,
            &["models.json", "settings.json"],
        ));
    }
    let latest = recent_operations(&fixture.storage, OperationKind::PiSettings, 1).unwrap()[0]
        .id
        .clone();
    fixture.restore(&latest).unwrap();
    let before = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::Pi, 50),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(before.entries.len(), 13);
    assert_eq!(
        before
            .entries
            .iter()
            .filter(|entry| entry.entry.intent == HistoryIntent::Restore)
            .count(),
        1
    );
    assert!(
        before
            .entries
            .iter()
            .all(|entry| entry.entry.source_steps.len() == 2)
    );
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let mut page_request = request(ConfigurationAgent::Pi, 3);
    let mut keys = std::collections::BTreeSet::new();
    loop {
        let page =
            configuration_history_page(&reopened, &environment, &page_request, &fixture.backups())
                .unwrap();
        for entry in page.entries {
            assert!(operations.contains(&entry.entry.operation_id));
            assert!(keys.insert(entry.entry.entry_key));
        }
        page_request.after = page.next_cursor;
        if page_request.after.is_none() {
            break;
        }
    }
    assert_eq!(keys.len(), 13);
    let after = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::Pi, 50),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        fs::read(root.join("models.json")).unwrap(),
        b"synthetic-private-before"
    );
}

#[test]
fn history_surface_rejects_wrong_root_or_agent_cursor_and_invalid_environment() {
    let fixture = Fixture::new();
    let mut environment = environment(&fixture);
    let root = environment.home.join(".pi/agent");
    for _ in 0..2 {
        apply_in(&fixture, OperationKind::PiSettings, &root, &["models.json"]);
    }
    let first = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::Pi, 1),
        &fixture.backups(),
    )
    .unwrap();
    let mut next = request(ConfigurationAgent::GrokBuild, 1);
    next.after = first.next_cursor;
    assert!(next.after.is_some());
    assert_eq!(
        configuration_history_page(&fixture.storage, &environment, &next, &fixture.backups())
            .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
    next.agent = ConfigurationAgent::Pi;
    environment.pi_directory = Some(fixture.root.join("different").into_os_string());
    assert_eq!(
        configuration_history_page(&fixture.storage, &environment, &next, &fixture.backups())
            .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
    for invalid in ["", "relative", "invalid\nroot"] {
        environment.pi_directory = Some(invalid.into());
        assert_eq!(
            configuration_history_page(
                &fixture.storage,
                &environment,
                &request(ConfigurationAgent::Pi, 1),
                &fixture.backups()
            )
            .unwrap_err(),
            ClaudeCodeRestoreError::InvalidRequest
        );
    }
    for limit in [0, 51, u32::MAX] {
        assert_eq!(
            configuration_history_page(
                &fixture.storage,
                &DiscoveryEnvironment {
                    pi_directory: Some(root.clone().into_os_string()),
                    ..DiscoveryEnvironment::default()
                },
                &request(ConfigurationAgent::Pi, limit),
                &fixture.backups()
            )
            .unwrap_err(),
            ClaudeCodeRestoreError::InvalidRequest
        );
    }
}

#[test]
fn history_surface_adds_provider_labels_and_resolution_state_without_file_bytes() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    fs::create_dir_all(&root).unwrap();
    let provider = crate::providers::create_provider(
        &fixture.storage,
        &FakeStore::default(),
        CreateProviderRequest {
            kind: "deepseek".to_owned(),
            display_name: "History Provider".to_owned(),
            base_url: "https://api.deepseek.com".to_owned(),
            protocol: "chat_completions".to_owned(),
            extensions: BTreeMap::new(),
            secret: Secret::new("synthetic-history-key".to_owned()),
        },
        42,
    )
    .unwrap();
    let path = root.join("settings.json");
    fs::write(&path, b"synthetic-private-before").unwrap();
    let target = read_target_bytes(&path).unwrap().0;
    let applied = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        Some(provider.id.as_str()),
        &[PlannedFile {
            target: WriteTarget::from_plan(&target),
            content: b"synthetic-private-after".to_vec(),
        }],
        &fixture.backups(),
    )
    .unwrap();
    let request = request(ConfigurationAgent::ClaudeCode, 50);
    let page =
        configuration_history_page(&fixture.storage, &environment, &request, &fixture.backups())
            .unwrap();
    assert_eq!(
        page.entries[0].owner_label.as_deref(),
        Some("History Provider")
    );
    assert!(!page.entries[0].requires_resolution);

    // An interrupted file result must withhold the legacy restore action until
    // the user records an observation; the page itself still only reports it.
    fixture.sql(&format!(
        "UPDATE config_file_revision SET state = 'prepared' WHERE operation_id = '{}'",
        applied.operation_id
    ));
    let page =
        configuration_history_page(&fixture.storage, &environment, &request, &fixture.backups())
            .unwrap();
    assert!(page.entries[0].requires_resolution);
    let serialized = serde_json::to_string(&page).unwrap();
    assert!(serialized.contains("\"requiresResolution\":true"));
    assert!(serialized.contains("\"ownerLabel\":\"History Provider\""));
    assert!(!serialized.contains("synthetic-private"));
    // The shared entry is flattened, so the frontend receives one object rather
    // than a nested `entry` field it would have to unwrap.
    let value = serde_json::to_value(&page).unwrap();
    let entry = &value["entries"][0];
    assert_eq!(
        entry["operationId"].as_str(),
        Some(applied.operation_id.as_str())
    );
    assert_eq!(entry["requiresResolution"].as_bool(), Some(true));
    assert_eq!(entry["ownerLabel"].as_str(), Some("History Provider"));
    assert!(entry.get("entry").is_none());

    // A reference to an absent Provider is preserved without inventing a label.
    let unowned = apply_in(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        &["settings.json"],
    );
    assert!(!unowned.is_empty());
    let page =
        configuration_history_page(&fixture.storage, &environment, &request, &fixture.backups())
            .unwrap();
    let entry = page
        .entries
        .iter()
        .find(|entry| entry.entry.operation_id == unowned)
        .expect("unowned operation");
    assert!(entry.entry.owner_ref.is_none());
    assert!(entry.owner_label.is_none());
    assert!(!serialized.contains("synthetic-history-key"));
}

#[test]
fn history_surface_payload_rejects_unsupported_agents_paths_and_cursor_extensions() {
    for value in [
        json!({"agent":"codex","after":null,"limit":10}),
        json!({"agent":"pi","after":null,"limit":10,"configRoot":"/arbitrary"}),
        json!({"agent":"pi","after":null,"limit":10,"entryPath":"/arbitrary"}),
        json!({"agent":"pi","after":null,"limit":-1}),
        json!({"agent":"pi","after":null,"limit":1.5}),
        json!({"agent":"pi","after":null}),
        json!({"agent":"pi","after":{"scopeDigest":"a".repeat(64),"createdAt":1,"entryKey":format!("operation:{}", "b".repeat(32)),"path":"/arbitrary"},"limit":10}),
    ] {
        assert!(serde_json::from_value::<ConfigurationHistoryRequest>(value).is_err());
    }
    let fixture = Fixture::new();
    let malformed: ConfigurationHistoryRequest = serde_json::from_value(json!({"agent":"pi","after":{"scopeDigest":"not-a-digest","createdAt":-1,"entryKey":"arbitrary"},"limit":10})).unwrap();
    assert_eq!(
        configuration_history_page(
            &fixture.storage,
            &environment(&fixture),
            &malformed,
            &fixture.backups()
        )
        .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
}

#[test]
fn history_surface_includes_current_pin_for_empty_and_matched_pages() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    // Empty journal: IPC page still carries the current pin (unrecorded, no match).
    let empty = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::ClaudeCode, 50),
        &fixture.backups(),
    )
    .unwrap();
    assert!(empty.entries.is_empty());
    assert!(empty.next_cursor.is_none());
    assert_eq!(empty.current.status, HistoryCurrentStatus::Unrecorded);
    assert_eq!(empty.current.matched_entry_key, None);
    assert_eq!(empty.current.files.len(), 1);
    assert!(
        empty.current.files[0]
            .target_path
            .ends_with(".claude/settings.json"),
        "{}",
        empty.current.files[0].target_path
    );
    assert!(empty.current.files[0].readable);
    assert_eq!(empty.current.files[0].current_digest, None);
    let empty_json = serde_json::to_value(&empty).unwrap();
    assert!(empty_json.get("current").is_some());
    assert_eq!(
        empty_json["current"]["matchedEntryKey"],
        serde_json::Value::Null
    );
    assert_eq!(empty_json["current"]["status"], "unrecorded");

    // Matched live files: pin names the matching entry and matches the core page.
    let root = environment.home.join(".claude");
    let id = apply_in(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        &["settings.json"],
    );
    let matched = configuration_history_page(
        &fixture.storage,
        &environment,
        &request(ConfigurationAgent::ClaudeCode, 50),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(matched.entries.len(), 1);
    assert_eq!(matched.current.status, HistoryCurrentStatus::Matched);
    assert_eq!(
        matched.current.matched_entry_key.as_deref(),
        Some(matched.entries[0].entry.entry_key.as_str())
    );
    assert_eq!(
        matched.current.matched_entry_key.as_deref(),
        Some(format!("operation:{id}").as_str())
    );
    let scope =
        HistoryScope::new(OperationKind::ClaudeCodeSettings, &root, &["settings.json"]).unwrap();
    let core =
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
    assert_eq!(matched.current, core.current);
    let matched_json = serde_json::to_value(&matched).unwrap();
    assert_eq!(
        matched_json["current"]["matchedEntryKey"].as_str(),
        Some(format!("operation:{id}").as_str())
    );
    assert_eq!(matched_json["current"]["status"], "matched");
}
