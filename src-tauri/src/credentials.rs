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

use serde::{Deserialize, Deserializer};

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

/// Read a secret from an IPC request field, which must be a JSON string.
///
/// This only wraps the text; call `validate_secret` before storing it. Validation
/// is separate so a bad value gets a stable error code instead of a raw
/// deserialization message. `Secret` deliberately has no `Serialize`, so it can
/// never be sent back to the frontend.
impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Secret::new)
    }
}

/// Largest secret, in UTF-16 code units, after trimming.
///
/// This is the tightest hard limit of the supported stores: Windows Credential
/// Manager stores the secret as UTF-16 with at most 2560 bytes
/// (`CRED_MAX_CREDENTIAL_BLOB_SIZE`), and each code unit takes 2 bytes. Most
/// characters are one unit; characters outside the Basic Multilingual Plane, such
/// as many emoji, are two. The same limit applies on every platform, so a key that
/// works on one computer also works on another.
pub const MAX_SECRET_UTF16_UNITS: usize = 1280;

/// A secret was rejected by `validate_secret`. It carries no copy of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSecret;

impl std::fmt::Display for InvalidSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("The key is empty, contains a control character, or is too long.")
    }
}

impl std::error::Error for InvalidSecret {}

/// Check a user-entered secret and return the value to store.
///
/// The vendor decides which characters a key may contain, so this does not guess a
/// format. It only:
/// - removes leading and trailing whitespace, which copying and pasting often adds;
/// - rejects an empty value;
/// - rejects control characters such as line breaks (CR, LF) and tabs, which cannot
///   be sent in an HTTP header;
/// - rejects a value longer than `MAX_SECRET_UTF16_UNITS`.
///
/// Everything else is allowed, including spaces inside the key and non-ASCII text.
///
/// # Errors
/// Returns `InvalidSecret` for an empty, control-character, or too-long value.
pub fn validate_secret(secret: Secret) -> Result<Secret, InvalidSecret> {
    let trimmed = secret.expose().trim();
    if trimmed.is_empty()
        || trimmed.chars().any(char::is_control)
        || trimmed.encode_utf16().count() > MAX_SECRET_UTF16_UNITS
    {
        return Err(InvalidSecret);
    }
    if trimmed.len() == secret.expose().len() {
        // Nothing was trimmed: keep the original value instead of copying it.
        return Ok(secret);
    }
    Ok(Secret::new(trimmed.to_string()))
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

/// How a commit step failed. This decides whether the credential change is undone.
#[derive(Debug, PartialEq, Eq)]
pub enum CommitFailure<E> {
    /// The database is unchanged: the failure happened before `COMMIT` (for
    /// example an `INSERT` was rejected). Undo the credential change.
    NotCommitted(E),
    /// `COMMIT` itself failed. The change may or may not have reached the database,
    /// so undoing the credential change could leave a reference with no secret
    /// behind it. Leave the credential store as it is and report an unknown outcome.
    Uncertain(E),
}

/// Why saving a secret under a new reference and committing its record failed.
#[derive(Debug)]
pub enum SaveNewError<E> {
    /// Writing the secret failed, so the commit step never ran and nothing was saved.
    Credential(CredentialError),
    /// The commit step failed after the secret was written.
    CommitFailed {
        /// The error returned by the commit step.
        source: E,
        /// `true` when the new entry was deleted again, so nothing is left behind.
        ///
        /// `false` means the store may still hold an entry that no database row
        /// references. The app never reads such an entry, but the caller must
        /// report that the outcome is not clean.
        cleaned_up: bool,
    },
    /// `COMMIT` failed and may have taken effect, so the new entry was kept: if the
    /// record was saved, it still has its secret. The caller must report that the
    /// outcome is unknown.
    CommitUncertain {
        /// The error returned by the commit step.
        source: E,
    },
}

/// Save `secret` under a newly generated `reference`, then run `commit`.
///
/// Use this when a record and its secret are created together, for example a new
/// provider and its API key. Steps:
/// 1. Write the secret. Stop if the write fails; there is nothing to undo.
/// 2. Run `commit`, usually the database transaction that inserts the record and
///    its credential reference, and return its value on success.
/// 3. If `commit` fails before `COMMIT` (`CommitFailure::NotCommitted`), delete the
///    entry again so no unreferenced secret stays. If `COMMIT` itself failed
///    (`CommitFailure::Uncertain`), keep the entry: deleting it could leave a saved
///    record without its secret, while a kept entry is at worst unreferenced.
///
/// The reference must be new (built from a freshly generated ID), so no earlier
/// secret is overwritten and none needs a backup.
pub fn save_new_then_commit<T, E>(
    store: &dyn CredentialStore,
    reference: &str,
    secret: &Secret,
    commit: impl FnOnce() -> Result<T, CommitFailure<E>>,
) -> Result<T, SaveNewError<E>> {
    store
        .save(reference, secret)
        .map_err(SaveNewError::Credential)?;
    match commit() {
        Ok(value) => Ok(value),
        Err(CommitFailure::NotCommitted(source)) => Err(SaveNewError::CommitFailed {
            source,
            cleaned_up: store.delete(reference).is_ok(),
        }),
        Err(CommitFailure::Uncertain(source)) => Err(SaveNewError::CommitUncertain { source }),
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
        /// `false` means the store may now hold the new secret even though the
        /// commit failed: either restoring failed, or the previous secret was
        /// unreadable and could not be put back. The caller must report that the
        /// outcome is unknown.
        restored: bool,
    },
    /// `COMMIT` failed and may have taken effect, so the new secret was kept
    /// instead of restoring the old one. The caller must report that the outcome
    /// is unknown.
    CommitUncertain {
        /// The error returned by the commit step.
        source: E,
    },
}

/// What the store held before a replacement, kept only in memory.
enum Backup {
    /// No secret was stored; undo by deleting the new one.
    Missing,
    /// The previous secret; undo by writing it back.
    Saved(Secret),
    /// A value existed but could not be read, so it cannot be restored.
    Unreadable,
}

/// Replace the secret for `reference`, then run `commit` to record the change.
///
/// Steps:
/// 1. Read the previous secret as a private backup. Stop if the read fails, except
///    when the stored value is unreadable (see below).
/// 2. Write the new secret. Stop if the write fails; the previous secret remains.
/// 3. Run `commit`, usually the database update that points at this reference, and
///    return its value on success.
/// 4. If `commit` fails before `COMMIT` (`CommitFailure::NotCommitted`), restore
///    the previous state: write the old secret back, or delete the new reference
///    when no old secret existed.
/// 5. If `COMMIT` itself failed (`CommitFailure::Uncertain`), keep the new secret.
///    The database may already record the change; for a first key, deleting the
///    new secret would then leave a reference with no secret at all. Keeping it
///    means every possible database state still has a secret behind it.
///
/// An unreadable previous value (`CorruptValue`) does not stop the replacement:
/// replacing is the only way to repair it. If `commit` then fails, the new secret
/// stays, because deleting it would leave a referenced entry with no secret at
/// all, and `restored` is `false`.
///
/// The backup lives only in a local variable and is dropped when this function returns.
/// It is never written to disk, logged, or returned.
pub fn replace_then_commit<T, E>(
    store: &dyn CredentialStore,
    reference: &str,
    secret: &Secret,
    commit: impl FnOnce() -> Result<T, CommitFailure<E>>,
) -> Result<T, ReplaceError<E>> {
    let backup = match store.load(reference) {
        Ok(Some(old)) => Backup::Saved(old),
        Ok(None) => Backup::Missing,
        Err(CredentialError::CorruptValue) => Backup::Unreadable,
        Err(error) => return Err(ReplaceError::Credential(error)),
    };
    store
        .save(reference, secret)
        .map_err(ReplaceError::Credential)?;
    match commit() {
        Ok(value) => Ok(value),
        Err(CommitFailure::Uncertain(source)) => Err(ReplaceError::CommitUncertain { source }),
        Err(CommitFailure::NotCommitted(source)) => {
            let restored = match &backup {
                Backup::Saved(old) => store.save(reference, old).is_ok(),
                Backup::Missing => store.delete(reference).is_ok(),
                Backup::Unreadable => false,
            };
            Err(ReplaceError::CommitFailed { source, restored })
        }
    }
}

/// An in-memory credential store for tests in any module of this crate.
///
/// It never touches the real OS store. Tests can make each operation fail on
/// purpose and count calls, for example to prove that reading a status does not
/// access the credential store at all.
#[cfg(test)]
pub(crate) mod fake {
    use super::{CredentialError, CredentialStore, Secret};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// The fake store. Every field is behind a `Mutex` because the trait methods
    /// take `&self`.
    #[derive(Default)]
    pub(crate) struct FakeStore {
        values: Mutex<HashMap<String, String>>,
        save_error: Mutex<Option<CredentialError>>,
        load_error: Mutex<Option<CredentialError>>,
        delete_error: Mutex<Option<CredentialError>>,
        calls: Mutex<usize>,
    }

    impl FakeStore {
        /// Create a store that already holds one secret.
        pub(crate) fn holding(reference: &str, value: &str) -> Self {
            let store = Self::default();
            store
                .values
                .lock()
                .expect("lock fake values")
                .insert(reference.to_string(), value.to_string());
            store
        }

        /// Return the stored value, if any, for assertions.
        pub(crate) fn stored(&self, reference: &str) -> Option<String> {
            self.values
                .lock()
                .expect("lock fake values")
                .get(reference)
                .cloned()
        }

        /// Whether any entry holds `value`, for tests that cannot know the entry name.
        pub(crate) fn holds_value(&self, value: &str) -> bool {
            self.values
                .lock()
                .expect("lock fake values")
                .values()
                .any(|stored| stored == value)
        }

        /// Make every later `save` fail with `error`, or succeed again with `None`.
        pub(crate) fn fail_saves(&self, error: Option<CredentialError>) {
            *self.save_error.lock().expect("lock save failure") = error;
        }

        /// Make every later `load` fail with `error`, or succeed again with `None`.
        pub(crate) fn fail_loads(&self, error: Option<CredentialError>) {
            *self.load_error.lock().expect("lock load failure") = error;
        }

        /// Make every later `delete` fail with `error`, or succeed again with `None`.
        pub(crate) fn fail_deletes(&self, error: Option<CredentialError>) {
            *self.delete_error.lock().expect("lock delete failure") = error;
        }

        /// How many `save`, `load`, and `delete` calls were made, failed ones included.
        pub(crate) fn call_count(&self) -> usize {
            *self.calls.lock().expect("lock call count")
        }

        fn count_call(&self) {
            *self.calls.lock().expect("lock call count") += 1;
        }
    }

    impl CredentialStore for FakeStore {
        fn save(&self, reference: &str, secret: &Secret) -> Result<(), CredentialError> {
            self.count_call();
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
            self.count_call();
            if let Some(error) = *self.load_error.lock().expect("lock load failure") {
                return Err(error);
            }
            Ok(self
                .values
                .lock()
                .expect("lock fake values")
                .get(reference)
                .map(|value| Secret::new(value.clone())))
        }

        fn delete(&self, reference: &str) -> Result<(), CredentialError> {
            self.count_call();
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
}

#[cfg(test)]
mod tests;
