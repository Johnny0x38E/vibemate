//! Behavior tests for the parent module, kept separate from runtime code.

use super::*;
use crate::credentials::CredentialError;
use crate::credentials::fake::FakeStore;
use crate::providers::{CreateProviderRequest, ProviderRecord, create_provider, get_provider};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const OLD_KEY: &str = "vibemate-synthetic-old-key-1b2c";
const NEW_KEY: &str = "vibemate-synthetic-new-key-9d8e";

/// A unique temporary app-data folder, removed when the test ends.
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vibemate-secrets-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create unique test directory");
        Self(path)
    }

    fn open(&self) -> Storage {
        Storage::open_in_directory(&self.0).expect("open storage")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Create a provider whose key `OLD_KEY` is saved in `store` at time 100.
fn create_with_key(storage: &Storage, store: &FakeStore) -> ProviderRecord {
    let request = CreateProviderRequest {
        kind: "deepseek".to_string(),
        display_name: "Keyed".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        protocol: "chat_completions".to_string(),
        extensions: BTreeMap::new(),
        secret: Secret::new(OLD_KEY.to_string()),
    };
    create_provider(storage, store, request, 100).expect("create")
}

/// Insert a provider row without a key, as a P10 build saved it.
fn insert_legacy_provider(storage: &Storage) -> String {
    let id = "a".repeat(32);
    storage
        .lock()
        .expect("lock")
        .execute(
            "INSERT INTO provider_instance
                 (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
             VALUES (?1, 'deepseek', 'Legacy', 'https://api.deepseek.com', 'chat_completions', 1, 10, 10)",
            [&id],
        )
        .expect("insert legacy provider");
    id
}

fn replace_request(provider_id: &str, key: &str) -> ReplaceProviderSecretRequest {
    ReplaceProviderSecretRequest {
        provider_id: provider_id.to_string(),
        secret: Secret::new(key.to_string()),
    }
}

/// Make SQLite reject writes, so the commit step of a replacement fails.
fn reject_writes(storage: &Storage) {
    storage
        .lock()
        .expect("lock")
        .pragma_update(None, "query_only", true)
        .expect("read-only");
}

#[test]
fn provider_secrets_status_comes_from_sqlite_only() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create_with_key(&storage, &FakeStore::default());
    let legacy = insert_legacy_provider(&storage);

    // No credential store is passed, so reading a status cannot touch one.
    assert_eq!(
        get_secret_status(&storage, created.id.as_str()),
        Ok(ProviderSecretStatus {
            provider_id: created.id.clone(),
            state: ProviderSecretState::Set,
            updated_at_ms: Some(100),
        })
    );
    let legacy_status = get_secret_status(&storage, &legacy).expect("legacy status");
    assert_eq!(legacy_status.state, ProviderSecretState::Missing);
    assert_eq!(legacy_status.updated_at_ms, None);
    assert_eq!(
        get_secret_status(&storage, &"f".repeat(32)),
        Err(ProviderError::NotFound)
    );
    assert_eq!(
        get_secret_status(&storage, "Keyed"),
        Err(ProviderError::InvalidRequest)
    );
}

#[test]
fn provider_secrets_replace_keeps_the_provider_revision() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);

    let status = replace_provider_secret(
        &storage,
        &store,
        replace_request(created.id.as_str(), &format!(" {NEW_KEY}\n")),
        200,
    )
    .expect("replace");

    assert_eq!(status.state, ProviderSecretState::Set);
    assert_eq!(status.updated_at_ms, Some(200));
    assert_eq!(get_secret_status(&storage, created.id.as_str()), Ok(status));
    assert_eq!(
        store.stored(&created.id.credential_reference()).as_deref(),
        Some(NEW_KEY)
    );
    // Revision, updated_at, and every other provider field are unchanged.
    assert_eq!(get_provider(&storage, created.id.as_str()), Ok(created));
}

#[test]
fn provider_secrets_replace_sets_a_missing_legacy_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let legacy = insert_legacy_provider(&storage);

    let status = replace_provider_secret(&storage, &store, replace_request(&legacy, NEW_KEY), 50)
        .expect("set legacy key");

    assert_eq!(status.state, ProviderSecretState::Set);
    assert_eq!(get_secret_status(&storage, &legacy), Ok(status));
    let reference: String = storage
        .lock()
        .expect("lock")
        .query_row(
            "SELECT credential_ref FROM provider_credential WHERE provider_id = ?1",
            [&legacy],
            |row| row.get(0),
        )
        .expect("reference row");
    assert_eq!(reference, format!("provider-{legacy}"));
    assert_eq!(store.stored(&reference).as_deref(), Some(NEW_KEY));
}

#[test]
fn provider_secrets_bad_requests_never_reach_the_credential_store() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let calls_after_create = store.call_count();

    for (request, code) in [
        (
            replace_request(&"f".repeat(32), NEW_KEY),
            ProviderError::NotFound,
        ),
        (
            replace_request("Keyed", NEW_KEY),
            ProviderError::InvalidRequest,
        ),
        (
            replace_request(created.id.as_str(), "two\nlines"),
            ProviderError::SecretInvalid,
        ),
        (
            replace_request(created.id.as_str(), "   "),
            ProviderError::SecretInvalid,
        ),
    ] {
        assert_eq!(
            replace_provider_secret(&storage, &store, request, 200),
            Err(code)
        );
    }
    assert_eq!(store.call_count(), calls_after_create);
    assert_eq!(
        store.stored(&created.id.credential_reference()).as_deref(),
        Some(OLD_KEY)
    );
}

#[test]
fn provider_secrets_credential_store_failures_change_nothing() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let before = get_secret_status(&storage, created.id.as_str()).expect("status");

    store.fail_saves(Some(CredentialError::AccessDenied));
    assert_eq!(
        replace_provider_secret(
            &storage,
            &store,
            replace_request(created.id.as_str(), NEW_KEY),
            200
        ),
        Err(ProviderError::CredentialStoreAccessDenied)
    );
    store.fail_saves(None);
    store.fail_loads(Some(CredentialError::Unavailable));
    assert_eq!(
        replace_provider_secret(
            &storage,
            &store,
            replace_request(created.id.as_str(), NEW_KEY),
            200
        ),
        Err(ProviderError::CredentialStoreUnavailable)
    );
    store.fail_loads(None);

    assert_eq!(
        store.stored(&created.id.credential_reference()).as_deref(),
        Some(OLD_KEY)
    );
    assert_eq!(get_secret_status(&storage, created.id.as_str()), Ok(before));
}

#[test]
fn provider_secrets_database_failure_restores_the_previous_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let before = get_secret_status(&storage, created.id.as_str()).expect("status");
    reject_writes(&storage);

    assert_eq!(
        replace_provider_secret(
            &storage,
            &store,
            replace_request(created.id.as_str(), NEW_KEY),
            200
        ),
        Err(ProviderError::WriteFailed)
    );
    assert_eq!(
        store.stored(&created.id.credential_reference()).as_deref(),
        Some(OLD_KEY)
    );
    assert_eq!(get_secret_status(&storage, created.id.as_str()), Ok(before));
}

#[test]
fn provider_secrets_failed_restore_is_an_unknown_outcome() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    // With no previous key, undoing means deleting the new one; make that fail.
    let legacy = insert_legacy_provider(&storage);
    reject_writes(&storage);
    store.fail_deletes(Some(CredentialError::OperationFailed));

    assert_eq!(
        replace_provider_secret(&storage, &store, replace_request(&legacy, NEW_KEY), 200),
        Err(ProviderError::SecretOutcomeUnknown)
    );
    let reference = format!("provider-{legacy}");
    assert_eq!(store.stored(&reference).as_deref(), Some(NEW_KEY));
    assert_eq!(
        get_secret_status(&storage, &legacy).map(|status| status.state),
        Ok(ProviderSecretState::Missing)
    );
}

#[test]
fn provider_secrets_failed_commit_keeps_the_new_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let legacy = insert_legacy_provider(&storage);
    crate::providers::fail_commits_after_reference_writes(&storage);

    // Replacing an existing key hits the UPDATE trigger, setting a missing
    // key hits the INSERT trigger; COMMIT fails in both cases.
    for provider_id in [created.id.as_str(), legacy.as_str()] {
        let calls_before = store.call_count();
        assert_eq!(
            replace_provider_secret(&storage, &store, replace_request(provider_id, NEW_KEY), 200),
            Err(ProviderError::SecretOutcomeUnknown)
        );
        // load + save only: no restore after an uncertain COMMIT.
        assert_eq!(store.call_count() - calls_before, 2);
        let reference = format!("provider-{provider_id}");
        assert_eq!(store.stored(&reference).as_deref(), Some(NEW_KEY));
    }
}

#[test]
fn provider_secrets_unreadable_old_key_is_replaced_or_reported() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let reference = created.id.credential_reference();
    store.fail_loads(Some(CredentialError::CorruptValue));

    // Replacing is the only way to repair an unreadable key.
    replace_provider_secret(
        &storage,
        &store,
        replace_request(created.id.as_str(), NEW_KEY),
        200,
    )
    .expect("replace corrupt key");
    assert_eq!(store.stored(&reference).as_deref(), Some(NEW_KEY));

    // If the commit then fails, the old key cannot be put back.
    reject_writes(&storage);
    assert_eq!(
        replace_provider_secret(
            &storage,
            &store,
            replace_request(created.id.as_str(), OLD_KEY),
            300
        ),
        Err(ProviderError::SecretOutcomeUnknown)
    );
}

#[test]
fn provider_secrets_reference_is_not_saved_for_a_removed_provider() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let id = ProviderId::parse(&"b".repeat(32)).expect("id");
    assert_eq!(
        save_reference(&storage, &id, &id.credential_reference(), 1),
        Err(CommitFailure::NotCommitted(ProviderError::NotFound))
    );
}

#[test]
fn provider_secrets_key_never_reaches_the_database_file_or_debug_output() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let created = create_with_key(&storage, &store);
    let request = replace_request(created.id.as_str(), NEW_KEY);
    assert!(!format!("{request:?}").contains(NEW_KEY));
    let status = replace_provider_secret(&storage, &store, request, 200).expect("replace");
    assert!(!format!("{status:?}").contains(NEW_KEY));
    drop(storage);

    let mut saw_reference = false;
    let reference = created.id.credential_reference();
    for entry in fs::read_dir(&directory.0).expect("list app-data folder") {
        let bytes = fs::read(entry.expect("entry").path()).expect("read file");
        for key in [OLD_KEY, NEW_KEY] {
            assert!(
                !bytes
                    .windows(key.len())
                    .any(|window| window == key.as_bytes()),
                "a key was written to a database file"
            );
        }
        saw_reference |= bytes
            .windows(reference.len())
            .any(|window| window == reference.as_bytes());
    }
    // Proves the scan read the real database contents.
    assert!(saw_reference);
}
