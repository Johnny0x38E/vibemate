//! Coordinated settings/key saves tested without the real OS credential store.

use super::*;

const NEW_KEY: &str = "vibemate-synthetic-unified-new-key";

#[test]
fn unified_save_replaces_the_key_and_commits_settings_and_reference_together() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    let updated = update_provider_with_secret(
        &storage,
        &store,
        &update_request(&original, "New", "https://api.deepseek.com/v1"),
        Some(Secret::new(NEW_KEY.into())),
        20,
    )
    .unwrap();
    assert_eq!(updated.revision, original.revision + 1);
    assert_eq!(updated.display_name, "New");
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(NEW_KEY)
    );
    let status =
        crate::provider_secrets::get_secret_status(&storage, original.id.as_str()).unwrap();
    assert_eq!(status.updated_at_ms, Some(20));
    assert!(!serde_json::to_string(&updated).unwrap().contains(NEW_KEY));
    drop(storage);
    let database = fs::read(directory.0.join("vibemate.sqlite3")).unwrap();
    assert!(
        !database
            .windows(NEW_KEY.len())
            .any(|bytes| bytes == NEW_KEY.as_bytes())
    );
    assert_eq!(
        get_provider(&directory.open(), original.id.as_str()),
        Ok(updated)
    );
}

#[test]
fn unified_save_without_a_new_key_never_accesses_or_changes_credentials() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    let before = store.call_count();
    store.fail_loads(Some(CredentialError::AccessDenied));
    store.fail_saves(Some(CredentialError::Unavailable));
    let updated = update_provider_with_secret(
        &storage,
        &store,
        &update_request(&original, "New", &original.base_url),
        None,
        20,
    )
    .unwrap();
    assert_eq!(updated.display_name, "New");
    assert_eq!(store.call_count(), before);
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(SYNTHETIC_KEY)
    );
    assert_eq!(
        crate::provider_secrets::get_secret_status(&storage, original.id.as_str())
            .unwrap()
            .updated_at_ms,
        Some(10)
    );
}

#[test]
fn unified_save_invalid_or_stale_requests_do_not_touch_the_store() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    let before = store.call_count();
    let mut stale = update_request(&original, "New", &original.base_url);
    stale.expected_revision += 1;
    for (request, key, error) in [
        (
            update_request(&original, "", &original.base_url),
            NEW_KEY,
            ProviderError::DisplayNameInvalid,
        ),
        (
            update_request(&original, "New", &original.base_url),
            "bad\nkey",
            ProviderError::SecretInvalid,
        ),
        (stale, NEW_KEY, ProviderError::RevisionConflict),
    ] {
        assert_eq!(
            update_provider_with_secret(
                &storage,
                &store,
                &request,
                Some(Secret::new(key.into())),
                20
            ),
            Err(error)
        );
    }
    assert_eq!(store.call_count(), before);
    assert_eq!(get_provider(&storage, original.id.as_str()), Ok(original));
}

#[test]
fn unified_save_credential_failure_keeps_settings_and_the_previous_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    store.fail_saves(Some(CredentialError::AccessDenied));
    assert_eq!(
        update_provider_with_secret(
            &storage,
            &store,
            &update_request(&original, "New", &original.base_url),
            Some(Secret::new(NEW_KEY.into())),
            20
        ),
        Err(ProviderError::CredentialStoreAccessDenied)
    );
    assert_eq!(
        get_provider(&storage, original.id.as_str()),
        Ok(original.clone())
    );
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(SYNTHETIC_KEY)
    );
}

#[test]
fn unified_save_reference_failure_rolls_back_settings_and_restores_the_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    storage
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_key_reference BEFORE UPDATE ON provider_credential
         BEGIN SELECT RAISE(ABORT, 'test reference failure'); END;",
        )
        .unwrap();
    assert_eq!(
        update_provider_with_secret(
            &storage,
            &store,
            &update_request(&original, "New", &original.base_url),
            Some(Secret::new(NEW_KEY.into())),
            20
        ),
        Err(ProviderError::WriteFailed)
    );
    assert_eq!(
        get_provider(&storage, original.id.as_str()),
        Ok(original.clone())
    );
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(SYNTHETIC_KEY)
    );
    assert_eq!(
        crate::provider_secrets::get_secret_status(&storage, original.id.as_str())
            .unwrap()
            .updated_at_ms,
        Some(10)
    );
}

#[test]
fn unified_save_can_add_a_key_for_a_legacy_instance() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    storage
        .lock()
        .unwrap()
        .execute("DELETE FROM provider_credential", [])
        .unwrap();
    store.delete(&original.id.credential_reference()).unwrap();
    update_provider_with_secret(
        &storage,
        &store,
        &update_request(&original, "New", &original.base_url),
        Some(Secret::new(NEW_KEY.into())),
        20,
    )
    .unwrap();
    assert_eq!(
        crate::provider_secrets::get_secret_status(&storage, original.id.as_str())
            .unwrap()
            .state,
        crate::provider_secrets::ProviderSecretState::Set
    );
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(NEW_KEY)
    );
}

#[test]
fn unified_save_uncertain_commit_keeps_the_new_key_and_reports_unknown_outcome() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let original =
        create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
    storage
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TABLE deferred_failure (
            id TEXT REFERENCES provider_instance(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER defer_update_failure AFTER UPDATE ON provider_instance
         BEGIN INSERT INTO deferred_failure VALUES ('missing'); END;",
        )
        .unwrap();
    assert_eq!(
        update_provider_with_secret(
            &storage,
            &store,
            &update_request(&original, "New", &original.base_url),
            Some(Secret::new(NEW_KEY.into())),
            20
        ),
        Err(ProviderError::OperationFailed)
    );
    assert_eq!(
        store.stored(&original.id.credential_reference()).as_deref(),
        Some(NEW_KEY)
    );
}

struct ConcurrentStore<'a> {
    store: &'a FakeStore,
    storage: &'a Storage,
    original: &'a ProviderRecord,
    fail_restore: bool,
}

impl CredentialStore for ConcurrentStore<'_> {
    fn load(&self, reference: &str) -> Result<Option<Secret>, CredentialError> {
        self.store.load(reference)
    }
    fn save(&self, reference: &str, secret: &Secret) -> Result<(), CredentialError> {
        self.store.save(reference, secret)?;
        if secret.expose() == NEW_KEY {
            // Simulate another process changing the settings during an OS prompt.
            // This would deadlock if the coordinated save held its SQLite lock.
            update_provider(
                self.storage,
                &update_request(self.original, "Concurrent", &self.original.base_url),
                15,
            )
            .unwrap();
            if self.fail_restore {
                self.store
                    .fail_saves(Some(CredentialError::OperationFailed));
            }
        }
        Ok(())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.store.delete(reference)
    }
}

#[test]
fn unified_save_rechecks_concurrent_changes_and_reports_a_failed_restore() {
    for fail_restore in [false, true] {
        let directory = TestDirectory::new();
        let storage = directory.open();
        let store = FakeStore::default();
        let original =
            create_provider(&storage, &store, create_request("deepseek", "Old"), 10).unwrap();
        let concurrent = ConcurrentStore {
            store: &store,
            storage: &storage,
            original: &original,
            fail_restore,
        };
        let expected = if fail_restore {
            ProviderError::OperationFailed
        } else {
            ProviderError::RevisionConflict
        };
        assert_eq!(
            update_provider_with_secret(
                &storage,
                &concurrent,
                &update_request(&original, "New", &original.base_url),
                Some(Secret::new(NEW_KEY.into())),
                20
            ),
            Err(expected)
        );
        assert_eq!(
            get_provider(&storage, original.id.as_str())
                .unwrap()
                .display_name,
            "Concurrent"
        );
        let expected_key = if fail_restore { NEW_KEY } else { SYNTHETIC_KEY };
        assert_eq!(
            store.stored(&original.id.credential_reference()).as_deref(),
            Some(expected_key)
        );
    }
}
