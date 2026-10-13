//! Alias preservation, stale-snapshot protection and persistence tests.

use super::*;
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
            "vibemate-alias-{}-{}",
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

fn request(snapshot: &ModelSnapshot, alias: Option<&str>) -> UpdateModelAliasRequest {
    UpdateModelAliasRequest {
        provider_id: PROVIDER.to_owned(),
        model_id: MODEL.to_owned(),
        expected_digest: snapshot.content_digest.clone(),
        alias: alias.map(str::to_owned),
    }
}

#[test]
fn editing_and_clearing_alias_preserves_manual_and_fetched_metadata_across_restart() {
    for source in ["manual", "fetched"] {
        let directory = TestDirectory::new();
        let storage = directory.open();
        seed(&storage, source);
        let original = get(&storage);
        let updated =
            update_model_alias(&storage, &request(&original, Some("  My model  ")), 20).unwrap();
        let mut expected_model = original.model.clone();
        expected_model.alias = Some("My model".to_owned());
        expected_model.updated_at_ms = 20;
        assert_eq!(updated.model, expected_model);
        assert_ne!(updated.content_digest, original.content_digest);
        drop(storage);
        let restarted = directory.open();
        assert_eq!(get(&restarted), updated);
        let cleared = update_model_alias(&restarted, &request(&updated, None), 30).unwrap();
        expected_model.alias = None;
        expected_model.updated_at_ms = 30;
        assert_eq!(cleared.model, expected_model);
        assert_ne!(cleared.content_digest, updated.content_digest);
    }
}

#[test]
fn a_second_connection_cannot_overwrite_a_new_alias_even_at_the_same_timestamp() {
    let directory = TestDirectory::new();
    let first = directory.open();
    seed(&first, "manual");
    let second = directory.open();
    let old = get(&first);
    let updated = update_model_alias(&second, &request(&old, Some("New")), 10).unwrap();
    assert_eq!(updated.model.updated_at_ms, old.model.updated_at_ms);
    assert_eq!(
        update_model_alias(&first, &request(&old, Some("Lost update")), 10),
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
            update_model_alias(&storage, &request(&before, Some("Stale")), 20),
            Err(ModelError::ModelChanged)
        );
        assert_eq!(get(&storage), changed);
    }
}

#[test]
fn unchanged_alias_is_a_noop_and_invalid_labels_do_not_write() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    seed(&storage, "manual");
    let original = get(&storage);
    assert_eq!(
        update_model_alias(&storage, &request(&original, Some(" Original ")), 100).unwrap(),
        original
    );
    for alias in [
        " ".to_owned(),
        "a\nlabel".to_owned(),
        "hidden\u{202e}".to_owned(),
        "x".repeat(65),
    ] {
        assert_eq!(
            update_model_alias(&storage, &request(&original, Some(&alias)), 20),
            Err(ModelError::ModelAliasInvalid)
        );
        assert_eq!(get(&storage), original);
    }
}

#[test]
fn invalid_digests_and_missing_models_are_rejected() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    seed(&storage, "manual");
    let original = get(&storage);
    for digest in ["".to_owned(), "G".repeat(64), "0".repeat(63)] {
        let mut input = request(&original, Some("New"));
        input.expected_digest = digest;
        assert_eq!(
            update_model_alias(&storage, &input, 20),
            Err(ModelError::InvalidRequest)
        );
    }
    let mut malformed = request(&original, Some("New"));
    malformed.model_id = "bad id".to_owned();
    assert_eq!(
        update_model_alias(&storage, &malformed, 20),
        Err(ModelError::InvalidRequest)
    );
    assert_eq!(
        update_model_alias(&storage, &request(&original, None), -1),
        Err(ModelError::InvalidRequest)
    );
    storage
        .lock()
        .unwrap()
        .execute("DELETE FROM provider_model", [])
        .unwrap();
    assert_eq!(
        update_model_alias(&storage, &request(&original, Some("New")), 20),
        Err(ModelError::ModelNotFound)
    );
    assert_eq!(
        get_model_snapshot(
            &storage,
            &GetModelSnapshotRequest {
                provider_id: PROVIDER.to_owned(),
                model_id: MODEL.to_owned()
            }
        ),
        Err(ModelError::ModelNotFound)
    );
}
