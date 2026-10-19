//! Strict history-delete IPC boundary tests with real SQLite and temp files.

use super::*;
use crate::agents::{
    ClaudeCodeRestoreError, DiscoveryEnvironment,
    history_delete::{DeleteHistoryRequest, delete_history_entry},
    recovery::ConfigurationAgent,
};
use crate::config_apply::{HistoryDeleteState, OperationKind};
use std::fs;

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
    let path = root.join("settings.json");
    if !path.exists() {
        fs::write(&path, b"{\"value\":\"seed\"}\n").unwrap();
    }
    let target = read_target_bytes(&path).unwrap().0;
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
fn the_surface_deletes_one_entry_for_the_named_agent() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"{\"value\":\"first\"}\n",
    );
    apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"{\"value\":\"second\"}\n",
    );
    let before = fs::read(root.join("settings.json")).unwrap();
    let outcome = delete_history_entry(
        &fixture.storage,
        &environment,
        &DeleteHistoryRequest {
            agent: ConfigurationAgent::ClaudeCode,
            entry_key: format!("operation:{first}"),
        },
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(outcome.state, HistoryDeleteState::Completed);
    assert_eq!(fs::read(root.join("settings.json")).unwrap(), before);
    let serialized = serde_json::to_string(&outcome).unwrap();
    assert!(
        !serialized.contains(fixture.backups().root().to_str().unwrap())
            || outcome.leftovers.is_empty()
    );
}

#[test]
fn the_surface_refuses_cross_agent_and_malformed_keys() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"{\"value\":\"first\"}\n",
    );
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &environment,
            &DeleteHistoryRequest {
                agent: ConfigurationAgent::Pi,
                entry_key: format!("operation:{first}"),
            },
            &fixture.backups(),
        )
        .unwrap_err(),
        ClaudeCodeRestoreError::StorageUnavailable
    );
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &environment,
            &DeleteHistoryRequest {
                agent: ConfigurationAgent::ClaudeCode,
                entry_key: "operation:not-a-real-id".to_owned(),
            },
            &fixture.backups(),
        )
        .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
}

#[test]
fn the_surface_maps_protection_codes() {
    let fixture = Fixture::new();
    let environment = environment(&fixture);
    let root = environment.home.join(".claude");
    let first = apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        &root,
        b"{\"value\":\"first\"}\n",
    );
    fixture.sql(&format!(
        "UPDATE config_operation SET state = 'unknown' WHERE id = '{first}';"
    ));
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &environment,
            &DeleteHistoryRequest {
                agent: ConfigurationAgent::ClaudeCode,
                entry_key: format!("operation:{first}"),
            },
            &fixture.backups(),
        )
        .unwrap_err(),
        ClaudeCodeRestoreError::HistoryActive
    );
}
