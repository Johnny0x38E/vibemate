//! Model-edit tests: alias handling, capability declarations, stale-snapshot
//! protection and persistence.

use super::*;
use crate::model_catalog::{CapabilityOverride, CapabilityOverrides, EffortLevel, ThinkingEffort};
use crate::storage::Storage;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const PROVIDER: &str = "0123456789abcdef0123456789abcdef";
const MODEL: &str = "vendor/model";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "vibemate-edit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn open(&self) -> Storage {
        Storage::open_in_directory(&self.0).unwrap()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn seed(storage: &Storage, source: &str) {
    storage
        .lock()
        .unwrap()
        .execute_batch(
            "INSERT INTO provider_instance VALUES ('0123456789abcdef0123456789abcdef',
         'command-code', 'Test', 'https://example.com', 'chat_completions', 1, 1, 1);",
        )
        .unwrap();
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model
         (provider_id, model_id, source, selected, alias, upstream_name, context_window,
          max_output_tokens, input_modalities, output_modalities, supported_endpoints,
          created_at, updated_at, last_seen_at, missing_since)
         VALUES (?1, ?2, ?3, 1, 'Original', 'Upstream', 10000, 1000,
         '[\"text\",\"image\"]', '[\"text\"]', '[\"/chat/completions\"]', 5, 10, 8, NULL)",
            params![PROVIDER, MODEL, source],
        )
        .unwrap();
}

fn get(storage: &Storage) -> ModelSnapshot {
    get_model_snapshot(
        storage,
        &GetModelSnapshotRequest {
            provider_id: PROVIDER.to_owned(),
            model_id: MODEL.to_owned(),
        },
    )
    .unwrap()
}

fn edit_request(
    snapshot: &ModelSnapshot,
    alias: Option<&str>,
    capability_overrides: CapabilityOverrides,
) -> UpdateModelEditRequest {
    UpdateModelEditRequest {
        provider_id: PROVIDER.to_owned(),
        model_id: MODEL.to_owned(),
        expected_digest: snapshot.content_digest.clone(),
        alias: alias.map(str::to_owned),
        capability_overrides,
        context_window: snapshot.model.context_window,
        max_output_tokens: snapshot.model.max_output_tokens,
        thinking_effort: snapshot.model.thinking_effort.clone(),
    }
}

#[test]
fn editing_and_clearing_the_alias_preserves_all_other_metadata_across_restart() {
    for source in ["manual", "fetched"] {
        let directory = TestDirectory::new();
        let storage = directory.open();
        seed(&storage, source);
        let original = get(&storage);
        let updated = update_model_edit(
            &storage,
            &edit_request(&original, Some("  My model  "), CapabilityOverrides::none()),
            20,
        )
        .unwrap();
        let mut expected_model = original.model.clone();
        expected_model.alias = Some("My model".to_owned());
        expected_model.updated_at_ms = 20;
        assert_eq!(updated.model, expected_model);
        assert_ne!(updated.content_digest, original.content_digest);
        drop(storage);
        let restarted = directory.open();
        assert_eq!(get(&restarted), updated);
        let cleared = update_model_edit(
            &restarted,
            &edit_request(&updated, None, CapabilityOverrides::none()),
            30,
        )
        .unwrap();
        expected_model.alias = None;
        expected_model.updated_at_ms = 30;
        assert_eq!(cleared.model, expected_model);
    }
}

#[test]
fn capability_declarations_persist_and_restore_default_clears_them() {
    for source in ["manual", "fetched"] {
        let directory = TestDirectory::new();
        let storage = directory.open();
        seed(&storage, source);
        let original = get(&storage);
        let declarations = CapabilityOverrides {
            thinking: Some(CapabilityOverride::Supported),
            image_input: Some(CapabilityOverride::Unsupported),
            tool_calls: None,
        };
        let updated = update_model_edit(
            &storage,
            &edit_request(&original, Some("  Edited  "), declarations),
            25,
        )
        .unwrap();
        let mut expected = original.model.clone();
        expected.alias = Some("Edited".to_owned());
        expected.capability_overrides = declarations;
        expected.updated_at_ms = 25;
        assert_eq!(updated.model, expected);
        assert_ne!(updated.content_digest, original.content_digest);
        drop(storage);
        // Declarations and label survive a restart like any other model metadata.
        let restarted = directory.open();
        assert_eq!(get(&restarted), updated);
        // "Restore default" clears every declaration and leaves the alias alone.
        let cleared = update_model_edit(
            &restarted,
            &edit_request(&updated, Some("Edited"), CapabilityOverrides::none()),
            30,
        )
        .unwrap();
        expected.capability_overrides = CapabilityOverrides::none();
        expected.updated_at_ms = 30;
        assert_eq!(cleared.model, expected);
        // The stored column is NULL again, the same "no declarations" form that
        // fresh and migrated rows use.
        let stored: Option<String> = restarted
            .lock()
            .unwrap()
            .query_row(
                "SELECT capability_overrides FROM provider_model",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, None);
    }
}

#[test]
fn alias_and_capability_writes_conflict_on_the_same_snapshot() {
    let directory = TestDirectory::new();
    let first = directory.open();
    seed(&first, "manual");
    let second = directory.open();
    let old = get(&first);
    let declarations = CapabilityOverrides {
        thinking: Some(CapabilityOverride::Supported),
        ..CapabilityOverrides::none()
    };
    let updated = update_model_edit(&second, &edit_request(&old, None, declarations), 10).unwrap();
    // A stale save that still carries the empty declarations it loaded cannot
    // silently drop the declarations another window just added.
    assert_eq!(
        update_model_edit(
            &first,
            &edit_request(&old, Some("Lost update"), CapabilityOverrides::none()),
            10
        ),
        Err(ModelError::ModelChanged)
    );
    // A stale declaration save cannot overwrite newer declarations either, even
    // when both writes would land on the same timestamp.
    let changed = CapabilityOverrides {
        thinking: Some(CapabilityOverride::Unsupported),
        ..CapabilityOverrides::none()
    };
    assert_eq!(
        update_model_edit(&first, &edit_request(&old, None, changed), 10),
        Err(ModelError::ModelChanged)
    );
    assert_eq!(get(&first), updated);
}

#[test]
fn metadata_and_routing_changes_invalidate_the_snapshot_without_a_timestamp_change() {
    for sql in [
        "UPDATE provider_model SET context_window = 20000",
        "UPDATE provider_model SET supported_endpoints = '[\"/messages\"]'",
        "UPDATE provider_model SET input_modalities = '[\"text\"]'",
        "UPDATE provider_model SET missing_since = 10",
    ] {
        let directory = TestDirectory::new();
        let storage = directory.open();
        seed(&storage, "fetched");
        let before = get(&storage);
        storage.lock().unwrap().execute_batch(sql).unwrap();
        let changed = get(&storage);
        assert_eq!(before.model.updated_at_ms, changed.model.updated_at_ms);
        assert_ne!(before.content_digest, changed.content_digest);
        assert_eq!(
            update_model_edit(
                &storage,
                &edit_request(&before, Some("Stale"), CapabilityOverrides::none()),
                20
            ),
            Err(ModelError::ModelChanged)
        );
        assert_eq!(get(&storage), changed);
    }
}

#[test]
fn unchanged_edits_are_noops_and_only_the_edited_field_changes() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    seed(&storage, "fetched");
    let original = get(&storage);
    // Same values, differently phrased: no write, same digest.
    assert_eq!(
        update_model_edit(
            &storage,
            &edit_request(&original, Some(" Original "), CapabilityOverrides::none()),
            100
        )
        .unwrap(),
        original
    );
    // Only the declarations change; the label stays as it was.
    let declarations = CapabilityOverrides {
        thinking: Some(CapabilityOverride::Supported),
        ..CapabilityOverrides::none()
    };
    let declared = update_model_edit(
        &storage,
        &edit_request(&original, Some("Original"), declarations),
        20,
    )
    .unwrap();
    assert_eq!(declared.model.alias.as_deref(), Some("Original"));
    assert_eq!(declared.model.capability_overrides, declarations);
    assert_ne!(declared.content_digest, original.content_digest);
}

#[test]
fn model_edits_reject_invalid_input_without_writing() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    seed(&storage, "manual");
    let original = get(&storage);
    for digest in ["".to_owned(), "G".repeat(64), "0".repeat(63)] {
        let mut input = edit_request(&original, Some("New"), CapabilityOverrides::none());
        input.expected_digest = digest;
        assert_eq!(
            update_model_edit(&storage, &input, 20),
            Err(ModelError::InvalidRequest)
        );
    }
    let mut malformed = edit_request(&original, Some("New"), CapabilityOverrides::none());
    malformed.model_id = "bad id".to_owned();
    assert_eq!(
        update_model_edit(&storage, &malformed, 20),
        Err(ModelError::InvalidRequest)
    );
    assert_eq!(
        update_model_edit(
            &storage,
            &edit_request(&original, Some("New"), CapabilityOverrides::none()),
            -1
        ),
        Err(ModelError::InvalidRequest)
    );
    // An invalid label must not write the declarations either: the request is
    // one edit, so it commits all fields or none.
    let declarations = CapabilityOverrides {
        image_input: Some(CapabilityOverride::Unsupported),
        ..CapabilityOverrides::none()
    };
    for alias in [
        " ".to_owned(),
        "a\nlabel".to_owned(),
        "hidden\u{202e}".to_owned(),
        "x".repeat(65),
    ] {
        assert_eq!(
            update_model_edit(
                &storage,
                &edit_request(&original, Some(&alias), declarations),
                20
            ),
            Err(ModelError::ModelAliasInvalid)
        );
    }
    assert_eq!(get(&storage), original);
    storage
        .lock()
        .unwrap()
        .execute("DELETE FROM provider_model", [])
        .unwrap();
    assert_eq!(
        update_model_edit(
            &storage,
            &edit_request(&original, Some("New"), declarations),
            20
        ),
        Err(ModelError::ModelNotFound)
    );
}

#[test]
fn a_manual_save_replaces_limits_and_a_clear_removes_them() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    seed(&storage, "fetched");
    let original = get(&storage);
    assert_eq!(original.model.context_window, Some(10_000));
    assert_eq!(original.model.max_output_tokens, Some(1_000));

    let mut request = edit_request(&original, Some("Original"), CapabilityOverrides::none());
    request.context_window = Some(128_000);
    request.max_output_tokens = Some(8_192);
    request.thinking_effort = Some(ThinkingEffort::Listed {
        levels: vec![EffortLevel::Low, EffortLevel::Max],
        default_level: Some(EffortLevel::Max),
    });
    let saved = update_model_edit(&storage, &request, 30).unwrap();
    assert_eq!(saved.model.context_window, Some(128_000));
    assert_eq!(saved.model.max_output_tokens, Some(8_192));
    assert_eq!(
        saved.model.thinking_effort,
        Some(ThinkingEffort::Listed {
            levels: vec![EffortLevel::Max, EffortLevel::Low],
            default_level: Some(EffortLevel::Max),
        })
    );
    drop(storage);
    let storage = directory.open();
    assert_eq!(get(&storage), saved);

    let mut clear = edit_request(&saved, Some("Original"), CapabilityOverrides::none());
    clear.context_window = None;
    clear.max_output_tokens = None;
    clear.thinking_effort = None;
    let cleared = update_model_edit(&storage, &clear, 40).unwrap();
    assert_eq!(cleared.model.context_window, None);
    assert_eq!(cleared.model.max_output_tokens, None);
    assert_eq!(cleared.model.thinking_effort, None);

    let mut invalid = edit_request(&cleared, Some("Original"), CapabilityOverrides::none());
    invalid.context_window = Some(0);
    assert_eq!(
        update_model_edit(&storage, &invalid, 50),
        Err(ModelError::ModelParametersInvalid)
    );
    assert_eq!(get(&storage).model.context_window, None);
}
