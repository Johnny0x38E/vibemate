//! Temporary-database tests; no Agent configuration or credential store is used.

use super::*;
use crate::model_catalog::{CapabilityOverride, CapabilityOverrides, ThinkingEffort};
use crate::models::{AddManualModelRequest, DeleteManualModelRequest, UpdateModelEditRequest};
use std::path::PathBuf;
use std::sync::{
    Barrier,
    atomic::{AtomicU64, Ordering},
};

const A: &str = "0123456789abcdef0123456789abcdef";
const B: &str = "fedcba9876543210fedcba9876543210";
const FLASH: &str = "deepseek-flash";

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Fixture {
    storage: Storage,
    directory: Directory,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = Directory(std::env::temp_dir().join(format!(
            "vibemate-effort-preference-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let storage = Storage::open_in_directory(&directory.0).unwrap();
        storage.lock().unwrap().execute_batch(&format!(
            "INSERT INTO provider_instance VALUES('{A}','deepseek','First','https://api.deepseek.com','chat_completions',1,1,1);
             INSERT INTO provider_instance VALUES('{B}','deepseek','Second','https://api.deepseek.com','chat_completions',1,1,1);"
        )).unwrap();
        let fixture = Self { storage, directory };
        fixture.add(A, FLASH);
        fixture.add(B, FLASH);
        fixture.add(A, "other-model");
        fixture
    }
    fn add(&self, provider: &str, model: &str) {
        crate::models::add_manual_model(
            &self.storage,
            &AddManualModelRequest {
                provider_id: provider.into(),
                model_id: model.into(),
                alias: None,
                capability_overrides: CapabilityOverrides {
                    thinking: Some(CapabilityOverride::Supported),
                    image_input: None,
                    tool_calls: None,
                },
                context_window: None,
                max_output_tokens: None,
                thinking_effort: Some(ThinkingEffort::Listed {
                    levels: vec![EffortLevel::Low, EffortLevel::High, EffortLevel::Max],
                    default_level: Some(EffortLevel::High),
                }),
            },
            1,
        )
        .unwrap();
    }
    fn load(&self, provider: &str, model: &str) -> EffortPreference {
        load_preference(
            &self.storage,
            &GetModelSnapshotRequest {
                provider_id: provider.into(),
                model_id: model.into(),
            },
        )
        .unwrap()
    }
}
fn request(current: &EffortPreference, level: Option<EffortLevel>) -> SaveEffortPreference {
    SaveEffortPreference {
        provider_id: current.snapshot.model.provider_id.clone(),
        model_id: current.snapshot.model.model_id.clone(),
        expected_model_digest: current.snapshot.content_digest.clone(),
        expected_revision: current.revision.clone(),
        selected_effort: level,
    }
}

#[test]
fn first_read_does_not_choose_the_model_default_or_write_anything() {
    let fixture = Fixture::new();
    let first = fixture.load(A, FLASH);
    assert_eq!(first.selected_effort, None);
    assert_eq!(first.revision, None);
    let same = save_preference(&fixture.storage, &request(&first, None)).unwrap();
    assert_eq!(same.revision, None);
    let connection = fixture.storage.lock().unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM claude_effort_preference", [], |row| {
                row.get::<_, u32>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM config_operation", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert!(!fixture.directory.0.join("settings.json").exists());
}

#[test]
fn saving_survives_reopening_without_changing_central_metadata() {
    let fixture = Fixture::new();
    let first = fixture.load(A, FLASH);
    let saved =
        save_preference(&fixture.storage, &request(&first, Some(EffortLevel::Max))).unwrap();
    assert!(saved.revision.as_deref().is_some_and(is_random_id));
    assert_eq!(saved.snapshot, first.snapshot);
    let repeated =
        save_preference(&fixture.storage, &request(&saved, Some(EffortLevel::Max))).unwrap();
    assert_eq!(repeated.revision, saved.revision);
    let Fixture { storage, directory } = fixture;
    drop(storage);
    let reopened = Storage::open_in_directory(&directory.0).unwrap();
    let loaded = load_preference(
        &reopened,
        &GetModelSnapshotRequest {
            provider_id: A.into(),
            model_id: FLASH.into(),
        },
    )
    .unwrap();
    assert_eq!(loaded.selected_effort, Some(EffortLevel::Max));
    assert_eq!(loaded.revision, saved.revision);
    assert_eq!(loaded.snapshot, saved.snapshot);
}

#[test]
fn equal_model_ids_in_other_accounts_and_other_models_do_not_share_preferences() {
    let fixture = Fixture::new();
    let a = fixture.load(A, FLASH);
    save_preference(&fixture.storage, &request(&a, Some(EffortLevel::Max))).unwrap();
    assert_eq!(fixture.load(B, FLASH).selected_effort, None);
    assert_eq!(fixture.load(A, "other-model").selected_effort, None);
    let b = fixture.load(B, FLASH);
    save_preference(&fixture.storage, &request(&b, Some(EffortLevel::Low))).unwrap();
    assert_eq!(
        fixture.load(A, FLASH).selected_effort,
        Some(EffortLevel::Max)
    );
    assert_eq!(
        fixture.load(B, FLASH).selected_effort,
        Some(EffortLevel::Low)
    );
}

#[test]
fn an_intervening_save_and_clear_cannot_look_like_the_original_absence() {
    let fixture = Fixture::new();
    let first = fixture.load(A, FLASH);
    let saved =
        save_preference(&fixture.storage, &request(&first, Some(EffortLevel::Max))).unwrap();
    let cleared = save_preference(&fixture.storage, &request(&saved, None)).unwrap();
    assert_eq!(cleared.selected_effort, None);
    assert!(cleared.revision.is_some());
    assert_ne!(cleared.revision, saved.revision);
    assert_eq!(
        save_preference(&fixture.storage, &request(&first, Some(EffortLevel::High))).unwrap_err(),
        PreferenceError::PreferenceChanged
    );
    assert_eq!(
        save_preference(&fixture.storage, &request(&saved, None)).unwrap_err(),
        PreferenceError::PreferenceChanged
    );
}

#[test]
fn independent_connections_cannot_overwrite_the_same_read_revision() {
    let fixture = Fixture::new();
    let other = Storage::open_in_directory(&fixture.directory.0).unwrap();
    let barrier = Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let before = fixture.load(A, FLASH);
            barrier.wait();
            save_preference(&fixture.storage, &request(&before, Some(EffortLevel::Max)))
        });
        let second = scope.spawn(|| {
            let before = load_preference(
                &other,
                &GetModelSnapshotRequest {
                    provider_id: A.into(),
                    model_id: FLASH.into(),
                },
            )
            .unwrap();
            barrier.wait();
            save_preference(&other, &request(&before, Some(EffortLevel::Low)))
        });
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(PreferenceError::PreferenceChanged)))
            .count(),
        1
    );
}

#[test]
fn changed_model_metadata_refuses_stale_saves_and_preserves_an_obsolete_saved_choice() {
    let fixture = Fixture::new();
    let before = fixture.load(A, FLASH);
    let saved =
        save_preference(&fixture.storage, &request(&before, Some(EffortLevel::Max))).unwrap();
    crate::models::update_model_edit(
        &fixture.storage,
        &UpdateModelEditRequest {
            provider_id: A.into(),
            model_id: FLASH.into(),
            expected_digest: saved.snapshot.content_digest.clone(),
            alias: None,
            capability_overrides: saved.snapshot.model.capability_overrides,
            context_window: None,
            max_output_tokens: None,
            thinking_effort: None,
        },
        2,
    )
    .unwrap();
    assert_eq!(
        save_preference(&fixture.storage, &request(&saved, Some(EffortLevel::Low))).unwrap_err(),
        PreferenceError::ModelChanged
    );
    let obsolete = fixture.load(A, FLASH);
    assert_eq!(obsolete.selected_effort, Some(EffortLevel::Max));
    assert_eq!(
        save_preference(
            &fixture.storage,
            &request(&obsolete, Some(EffortLevel::Max))
        )
        .unwrap_err(),
        PreferenceError::EffortNotMapped
    );
    assert_eq!(
        save_preference(&fixture.storage, &request(&obsolete, None))
            .unwrap()
            .selected_effort,
        None
    );
}

#[test]
fn unsupported_values_and_other_bindings_cannot_be_remembered_as_mapped() {
    let fixture = Fixture::new();
    let before = fixture.load(A, FLASH);
    for level in [EffortLevel::Minimal, EffortLevel::Off, EffortLevel::Medium] {
        assert_eq!(
            save_preference(&fixture.storage, &request(&before, Some(level))).unwrap_err(),
            PreferenceError::EffortNotMapped
        );
    }
    let other = fixture.load(A, "other-model");
    assert_eq!(
        save_preference(&fixture.storage, &request(&other, Some(EffortLevel::High))).unwrap_err(),
        PreferenceError::EffortNotMapped
    );
    assert_eq!(fixture.load(A, FLASH).revision, None);
}

#[test]
fn model_and_provider_deletion_cascade_only_their_own_preferences() {
    let fixture = Fixture::new();
    for provider in [A, B] {
        save_preference(
            &fixture.storage,
            &request(&fixture.load(provider, FLASH), Some(EffortLevel::Max)),
        )
        .unwrap();
    }
    crate::models::delete_manual_model(
        &fixture.storage,
        &DeleteManualModelRequest {
            provider_id: A.into(),
            model_id: FLASH.into(),
        },
    )
    .unwrap();
    assert_eq!(
        fixture.load(B, FLASH).selected_effort,
        Some(EffortLevel::Max)
    );
    fixture.add(A, FLASH);
    assert_eq!(fixture.load(A, FLASH).revision, None);
    let connection = fixture.storage.lock().unwrap();
    connection
        .execute("DELETE FROM provider_instance WHERE id=?1", [B])
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM claude_effort_preference", [], |row| {
                row.get::<_, u32>(0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn a_rejected_write_rolls_back_and_corrupt_data_never_means_inherit() {
    let fixture = Fixture::new();
    let saved = save_preference(
        &fixture.storage,
        &request(&fixture.load(A, FLASH), Some(EffortLevel::Low)),
    )
    .unwrap();
    fixture.storage.lock().unwrap().execute_batch("CREATE TRIGGER fail_preference BEFORE UPDATE ON claude_effort_preference BEGIN SELECT RAISE(ABORT,'blocked'); END;").unwrap();
    assert_eq!(
        save_preference(&fixture.storage, &request(&saved, Some(EffortLevel::Max))).unwrap_err(),
        PreferenceError::WriteFailed
    );
    assert_eq!(
        fixture.load(A, FLASH).selected_effort,
        Some(EffortLevel::Low)
    );
    fixture.storage.lock().unwrap().execute_batch("DROP TRIGGER fail_preference; PRAGMA ignore_check_constraints=ON; UPDATE claude_effort_preference SET effort='ultracode';").unwrap();
    assert_eq!(
        load_preference(
            &fixture.storage,
            &GetModelSnapshotRequest {
                provider_id: A.into(),
                model_id: FLASH.into()
            }
        )
        .unwrap_err(),
        PreferenceError::InvalidStoredData
    );
}
