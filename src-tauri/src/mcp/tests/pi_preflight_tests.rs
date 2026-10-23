//! Preflight regression tests use a store that panics on every credential read.

use super::*;
use serde_json::json;

#[test]
fn pi_preflight_maps_stdio_without_reading_secrets_or_changing_metadata() {
    let fixture = Fixture::new();
    let store = FakeStore::default();
    let mut input = with_env(vec![secret("TOKEN", Some("synthetic-preflight-value"))]);
    if let ConnectionInput::Stdio { args, cwd, .. } = &mut input.connection {
        *args = vec!["~/data".into(), "literal space".into()];
        *cwd = Some("relative/work".into());
    }
    let row = save_definition(&fixture.storage, &store, input, 1).unwrap();
    let original = br#"{"autoEnableCodemode":false,"mcpServers":{"other":{"env":{"TOKEN":"native-private-value"}}}}"#;
    let before = store.values.borrow().clone();
    let plan =
        preflight_pi_registration(&fixture.storage, &row.id, row.revision, Some(original)).unwrap();
    assert_eq!(
        plan.redacted_entry,
        json!({
            "command":"nonexistent-vibemate-test-server", "args":["~/data", "literal space"],
            "cwd":"relative/work", "env":{"TOKEN":crate::config_plan::SECRET_PLACEHOLDER}, "enabled":true
        })
    );
    assert!(plan.session_relative_cwd);
    assert!(plan.expands_home);
    let output = serde_json::to_string(&plan).unwrap();
    assert!(!output.contains("synthetic-preflight-value"));
    assert!(!output.contains("native-private-value"));
    assert!(!output.contains("other"));
    assert_eq!(*store.values.borrow(), before);
    assert_eq!(
        get_definition(&fixture.storage, &row.id).unwrap().revision,
        row.revision
    );
}

#[test]
fn pi_preflight_maps_disabled_http_and_never_includes_stdio_fields() {
    let fixture = Fixture::new();
    let store = FakeStore::default();
    let mut input = request();
    input.enabled = false;
    input.connection = ConnectionInput::Http {
        url: "https://example.com/mcp".into(),
        headers: vec![secret("Authorization", Some("Bearer synthetic-preflight"))],
    };
    let row = save_definition(&fixture.storage, &store, input, 1).unwrap();
    let plan = preflight_pi_registration(&fixture.storage, &row.id, row.revision, None).unwrap();
    assert_eq!(
        plan.redacted_entry,
        json!({"url":"https://example.com/mcp", "headers":{"authorization":crate::config_plan::SECRET_PLACEHOLDER},"enabled":false})
    );
    assert!(!plan.session_relative_cwd);
    assert!(!plan.expands_home);
}

#[test]
fn pi_preflight_refuses_existing_names_even_when_disabled_or_malformed() {
    let fixture = Fixture::new();
    let store = FakeStore::default();
    let mut input = request();
    input.server_name = "my-tools".into();
    let row = save_definition(&fixture.storage, &store, input, 1).unwrap();
    for existing in [
        json!({"my-tools":{"enabled":false}}),
        json!({"my_tools":null}),
    ] {
        let bytes = serde_json::to_vec(&json!({"mcpServers":existing})).unwrap();
        assert_eq!(
            preflight_pi_registration(&fixture.storage, &row.id, row.revision, Some(&bytes))
                .unwrap_err(),
            PiMcpPreflightError::NameConflict
        );
    }
}

#[test]
fn pi_preflight_rejects_malformed_or_oversized_native_containers() {
    let fixture = Fixture::new();
    let store = FakeStore::default();
    let row = save_definition(&fixture.storage, &store, request(), 1).unwrap();
    for bytes in [
        b"".as_slice(),
        b"[]",
        b"null",
        b"{",
        br#"{"mcpServers":null}"#,
        br#"{"mcpServers":[]}"#,
    ] {
        assert_eq!(
            preflight_pi_registration(&fixture.storage, &row.id, row.revision, Some(bytes))
                .unwrap_err(),
            PiMcpPreflightError::InvalidConfig
        );
    }
    let oversized = vec![b' '; crate::config_plan::MAX_TARGET_BYTES as usize + 1];
    assert_eq!(
        preflight_pi_registration(&fixture.storage, &row.id, row.revision, Some(&oversized))
            .unwrap_err(),
        PiMcpPreflightError::InvalidConfig
    );
    for bytes in [b"{}".as_slice(), b"\xef\xbb\xbf{\"mcpServers\":{}}"] {
        preflight_pi_registration(&fixture.storage, &row.id, row.revision, Some(bytes)).unwrap();
    }
}

#[test]
fn pi_preflight_refuses_stale_or_missing_definitions() {
    let fixture = Fixture::new();
    let store = FakeStore::default();
    let row = save_definition(&fixture.storage, &store, request(), 1).unwrap();
    assert_eq!(
        preflight_pi_registration(&fixture.storage, &row.id, row.revision + 1, None).unwrap_err(),
        PiMcpPreflightError::Definition(McpError::RevisionConflict)
    );
    delete_definition(
        &fixture.storage,
        &store,
        DeleteMcpRequest {
            id: row.id.clone(),
            expected_revision: row.revision,
        },
    )
    .unwrap();
    assert_eq!(
        preflight_pi_registration(&fixture.storage, &row.id, row.revision, None).unwrap_err(),
        PiMcpPreflightError::Definition(McpError::NotFound)
    );
}
