//! Operating-system credential storage for secrets such as provider API keys.
//!
//! The database keeps only a credential reference, a non-secret name such as
//! `provider-<id>`. The secret itself lives in the platform store: Keychain on
//! macOS, Credential Manager on Windows, and Secret Service on Linux desktops.
//! Callers depend on the `CredentialStore` trait, so tests use an in-memory fake.
//! No code path falls back to a plaintext file or database column.
//!
//! Secrets must never reach logs, panics, database rows, IPC responses, or error
//! text. `Secret` hides its value in `Debug` output, and `CredentialError` carries no
//! payload because some platform errors contain the raw secret bytes.

/// Service name for every vibemate entry in the OS credential store.
///
/// Changing this value makes previously saved secrets unreachable, so treat it as
/// part of the stored data format.
const SERVICE_NAME: &str = "dev.vibemate.desktop";

/// A secret value held in memory only while an operation needs it.
///
/// `Debug` prints a placeholder, so formatting a `Secret` in a log or panic cannot
/// reveal the value. Reading the value needs the explicit `expose` method, which
/// makes each use easy to find during review. `Secret` does not implement `Clone`,
/// so copies stay visible in the code.
pub struct Secret(String);

impl Secret {
    /// Wrap a secret value. The caller should drop its own copies as soon as possible.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the raw value for a platform call. Do not log it or return it to the UI.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

/// Why a credential operation failed.
///
/// The variants carry no payload on purpose. The original platform error is
/// converted into one of these categories and then dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// The platform store could not be initialized, so no secret can be saved.
    Unavailable,
    /// The store refused access, for example because it is locked or read-only.
    AccessDenied,
    /// The store reported a failure while writing, reading, or deleting.
    OperationFailed,
    /// The store rejected the reference, for example because it is too long.
    InvalidReference,
    /// The stored bytes are not the UTF-8 text that vibemate wrote.
    CorruptValue,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter
                .write_str("The operating system credential store is not available on this device."),
            Self::AccessDenied => formatter.write_str(
                "vibemate could not access the operating system credential store. Unlock it or allow access, then try again.",
            ),
            Self::OperationFailed => formatter.write_str(
                "The operating system credential store reported an error. No plaintext copy was saved.",
            ),
            Self::InvalidReference => formatter
                .write_str("The operating system credential store rejected the credential name."),
            Self::CorruptValue => {
                formatter.write_str("The saved credential is unreadable. Enter it again.")
            }
        }
    }
}

impl std::error::Error for CredentialError {}

/// Read, write, and delete secrets by credential reference.
///
/// A reference is a stable, non-secret name stored in the database. Implementations
/// must not keep the secret anywhere else.
pub trait CredentialStore {
    /// Store or replace the secret for `reference`.
    ///
    /// A failed write should leave the previous secret in place. The platform stores
    /// behave this way, and the test fake models it.
    fn save(&self, reference: &str, secret: &Secret) -> Result<(), CredentialError>;

    /// Return the secret for `reference`, or `None` when nothing is stored.
    fn load(&self, reference: &str) -> Result<Option<Secret>, CredentialError>;

    /// Remove the secret for `reference`. Deleting a missing secret succeeds.
    fn delete(&self, reference: &str) -> Result<(), CredentialError>;
}

/// The platform credential store used by the desktop app.
///
/// The first `keyring::Entry::new` call selects the platform backend: Keychain on
/// macOS, Credential Manager on Windows, and Secret Service on other Unix systems.
/// Secret Service needs a running keyring service such as GNOME Keyring or KWallet.
/// Without one, calls fail with a credential error, and nothing is saved in plaintext.
pub struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn save(&self, reference: &str, secret: &Secret) -> Result<(), CredentialError> {
        let entry = keyring::Entry::new(SERVICE_NAME, reference).map_err(map_keyring_error)?;
        entry
            .set_password(secret.expose())
            .map_err(map_keyring_error)
    }

    fn load(&self, reference: &str) -> Result<Option<Secret>, CredentialError> {
        let entry = keyring::Entry::new(SERVICE_NAME, reference).map_err(map_keyring_error)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            // A missing item is a normal state, not a failure.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        let entry = keyring::Entry::new(SERVICE_NAME, reference).map_err(map_keyring_error)?;
        match entry.delete_credential() {
            // Deleting an already missing item leaves the desired state in place.
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

/// Convert a keyring error into a category that is safe to show and easy to test.
///
/// The original error is dropped on purpose. Some variants contain the secret bytes,
/// and a `Debug` print of them would leak the secret.
fn map_keyring_error(error: keyring::Error) -> CredentialError {
    match error {
        keyring::Error::NoDefaultStore => CredentialError::Unavailable,
        keyring::Error::NoStorageAccess(_) => CredentialError::AccessDenied,
        keyring::Error::TooLong(_, _) | keyring::Error::Invalid(_, _) => {
            CredentialError::InvalidReference
        }
        keyring::Error::BadEncoding(_)
        | keyring::Error::BadDataFormat(_, _)
        | keyring::Error::BadStoreFormat(_) => CredentialError::CorruptValue,
        // Covers platform failures, ambiguous entries, unsupported operations, and
        // future variants. The original error is not kept, so it cannot leak data.
        _ => CredentialError::OperationFailed,
    }
}

/// Why replacing a credential and committing its database record failed.
#[derive(Debug)]
pub enum ReplaceError<E> {
    /// Reading the previous secret or writing the new one failed. The commit step never ran.
    Credential(CredentialError),
    /// The commit step failed after the new secret was written.
    CommitFailed {
        /// The error returned by the commit step.
        source: E,
        /// `true` when the previous credential state was restored.
        ///
        /// `false` means the store may still hold the new secret while the database
        /// does not reference it. The caller must keep a recovery record.
        restored: bool,
    },
}

/// Replace the secret for `reference`, then run `commit` to record the change.
///
/// Steps:
/// 1. Read the previous secret as a private backup. Stop if the read fails.
/// 2. Write the new secret. Stop if the write fails; the previous secret remains.
/// 3. Run `commit`, usually the database update that points at this reference.
/// 4. If `commit` fails, restore the previous state: write the old secret back, or
///    delete the new reference when no old secret existed.
///
/// The backup lives only in a local variable and is dropped when this function returns.
/// It is never written to disk, logged, or returned.
pub fn replace_then_commit<E>(
    store: &dyn CredentialStore,
    reference: &str,
    secret: &Secret,
    commit: impl FnOnce() -> Result<(), E>,
) -> Result<(), ReplaceError<E>> {
    let previous = store.load(reference).map_err(ReplaceError::Credential)?;
    store
        .save(reference, secret)
        .map_err(ReplaceError::Credential)?;
    match commit() {
        Ok(()) => Ok(()),
        Err(source) => {
            let restore = match &previous {
                Some(old) => store.save(reference, old),
                None => store.delete(reference),
            };
            Err(ReplaceError::CommitFailed {
                source,
                restored: restore.is_ok(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// An in-memory store where tests can make saves or deletes fail on purpose.
    #[derive(Default)]
    struct FakeStore {
        values: Mutex<HashMap<String, String>>,
        save_error: Mutex<Option<CredentialError>>,
        delete_error: Mutex<Option<CredentialError>>,
    }

    impl FakeStore {
        /// Create a store that already holds one secret.
        fn holding(reference: &str, value: &str) -> Self {
            let store = Self::default();
            store
                .values
                .lock()
                .expect("lock fake values")
                .insert(reference.to_string(), value.to_string());
            store
        }

        /// Return the stored value, if any, for assertions.
        fn stored(&self, reference: &str) -> Option<String> {
            self.values
                .lock()
                .expect("lock fake values")
                .get(reference)
                .cloned()
        }

        fn fail_saves(&self, error: Option<CredentialError>) {
            *self.save_error.lock().expect("lock save failure") = error;
        }

        fn fail_deletes(&self, error: Option<CredentialError>) {
            *self.delete_error.lock().expect("lock delete failure") = error;
        }
    }

    impl CredentialStore for FakeStore {
        fn save(&self, reference: &str, secret: &Secret) -> Result<(), CredentialError> {
            if let Some(error) = *self.save_error.lock().expect("lock save failure") {
                return Err(error);
            }
            self.values
                .lock()
                .expect("lock fake values")
                .insert(reference.to_string(), secret.expose().to_string());
            Ok(())
        }

        fn load(&self, reference: &str) -> Result<Option<Secret>, CredentialError> {
            Ok(self
                .values
                .lock()
                .expect("lock fake values")
                .get(reference)
                .map(|value| Secret::new(value.clone())))
        }

        fn delete(&self, reference: &str) -> Result<(), CredentialError> {
            if let Some(error) = *self.delete_error.lock().expect("lock delete failure") {
                return Err(error);
            }
            self.values
                .lock()
                .expect("lock fake values")
                .remove(reference);
            Ok(())
        }
    }

    #[test]
    fn secret_debug_output_hides_the_value() {
        let secret = Secret::new("synthetic-test-value".to_string());

        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
    }

    #[test]
    fn unavailable_store_stops_before_the_database_commit() {
        let store = FakeStore::holding("provider-1", "old");
        store.fail_saves(Some(CredentialError::Unavailable));
        let mut committed = false;

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || {
                committed = true;
                Ok::<(), &'static str>(())
            },
        );

        assert!(matches!(
            result,
            Err(ReplaceError::Credential(CredentialError::Unavailable))
        ));
        assert!(!committed);
        assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
    }

    #[test]
    fn access_denied_during_replace_keeps_the_previous_secret() {
        let store = FakeStore::holding("provider-1", "old");
        store.fail_saves(Some(CredentialError::AccessDenied));

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || Ok::<(), &'static str>(()),
        );

        assert!(matches!(
            result,
            Err(ReplaceError::Credential(CredentialError::AccessDenied))
        ));
        assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
    }

    #[test]
    fn failed_commit_restores_the_previous_secret() {
        let store = FakeStore::holding("provider-1", "old");

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || Err::<(), &'static str>("database write failed"),
        );

        assert!(matches!(
            result,
            Err(ReplaceError::CommitFailed { restored: true, .. })
        ));
        assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
    }

    #[test]
    fn failed_commit_removes_a_new_secret_when_none_existed_before() {
        let store = FakeStore::default();

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || Err::<(), &'static str>("database write failed"),
        );

        assert!(matches!(
            result,
            Err(ReplaceError::CommitFailed { restored: true, .. })
        ));
        assert_eq!(store.stored("provider-1"), None);
    }

    #[test]
    fn failed_restore_is_reported_so_a_recovery_record_can_be_kept() {
        let store = FakeStore::holding("provider-1", "old");

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || {
                // Simulate the store failing again during the compensation step.
                store.fail_saves(Some(CredentialError::OperationFailed));
                Err::<(), &'static str>("database write failed")
            },
        );

        assert!(matches!(
            result,
            Err(ReplaceError::CommitFailed {
                restored: false,
                ..
            })
        ));
        assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
    }

    #[test]
    fn failed_delete_during_restore_is_reported() {
        let store = FakeStore::default();

        let result = replace_then_commit(
            &store,
            "provider-1",
            &Secret::new("new".to_string()),
            || {
                store.fail_deletes(Some(CredentialError::OperationFailed));
                Err::<(), &'static str>("database write failed")
            },
        );

        assert!(matches!(
            result,
            Err(ReplaceError::CommitFailed {
                restored: false,
                ..
            })
        ));
        assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
    }

    #[test]
    fn platform_errors_map_to_safe_categories() {
        assert_eq!(
            map_keyring_error(keyring::Error::NoDefaultStore),
            CredentialError::Unavailable
        );
        assert_eq!(
            map_keyring_error(keyring::Error::NoStorageAccess(Box::new(
                std::io::Error::other("locked")
            ))),
            CredentialError::AccessDenied
        );
        assert_eq!(
            map_keyring_error(keyring::Error::PlatformFailure(Box::new(
                std::io::Error::other("platform failure")
            ))),
            CredentialError::OperationFailed
        );
        assert_eq!(
            map_keyring_error(keyring::Error::BadEncoding(b"not-text".to_vec())),
            CredentialError::CorruptValue
        );
        assert_eq!(
            map_keyring_error(keyring::Error::TooLong("account".to_string(), 10)),
            CredentialError::InvalidReference
        );
    }

    /// Manual check against the real platform store. It creates and deletes a
    /// temporary synthetic entry. Run it on a developer machine with:
    /// `cargo test --manifest-path src-tauri/Cargo.toml --locked credentials -- --ignored`
    #[test]
    #[ignore = "touches the real OS credential store; run manually on a developer machine"]
    fn os_store_round_trip_uses_a_temporary_entry() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after 1970")
            .as_nanos();
        let reference = format!("smoke-test-{}-{nanos}", std::process::id());
        let store = OsCredentialStore;

        store
            .save(
                &reference,
                &Secret::new("synthetic-smoke-value".to_string()),
            )
            .expect("save temporary entry");
        let loaded = store
            .load(&reference)
            .expect("load temporary entry")
            .expect("temporary entry exists");
        assert_eq!(loaded.expose(), "synthetic-smoke-value");
        store.delete(&reference).expect("delete temporary entry");
        assert!(store.load(&reference).expect("load after delete").is_none());
    }
}
