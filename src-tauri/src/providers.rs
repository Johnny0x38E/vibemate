//! Provider domain facade. Types, validation, SQL, and credential orchestration
//! remain independent of desktop IPC; the public entry points stay stable.

mod types;
pub use types::{
    CreateProviderRequest, ListProvidersRequest, MAX_BASE_URL_LEN, MAX_DISPLAY_NAME_CHARS,
    MAX_PAGE_SIZE, ProviderError, ProviderId, ProviderKind, ProviderPage, ProviderProtocol,
    ProviderRecord, ProviderSettings, ProviderTemplate, UpdateProviderRequest,
};
mod templates;
mod validation;
pub use templates::provider_templates;
pub(crate) use validation::is_hidden_format_character;
pub use validation::{normalize_base_url, parse_kind, validate_display_name, validate_settings};
mod repository;
mod service;
#[cfg(test)]
use crate::storage::Storage;
pub use repository::{get_provider, list_providers};
pub use service::{create_provider, update_provider, update_provider_with_secret};
use std::time::{SystemTime, UNIX_EPOCH};

/// Read the system clock as Unix epoch milliseconds for `created_at`/`updated_at`.
///
/// Callers pass the result into `create_provider`/`update_provider`, which keeps
/// those functions deterministic in tests.
///
/// # Errors
/// Returns `OperationFailed` if the clock is set before 1970 or impossibly far ahead.
pub fn current_unix_millis() -> Result<i64, ProviderError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProviderError::OperationFailed)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ProviderError::OperationFailed)
}

/// Test support shared with `provider_secrets`.
///
/// Make `COMMIT` fail after a reference row is inserted or updated, while every
/// statement before it succeeds.
///
/// A deferred foreign key is checked only at `COMMIT`. The test-only TEMP
/// trigger adds a row that breaks such a key, so `COMMIT` reports a constraint
/// error and SQLite rolls the transaction back. This is how a test can reach
/// the `COMMIT` failure path, which in real use comes from I/O errors.
#[cfg(test)]
pub(crate) fn fail_commits_after_reference_writes(storage: &Storage) {
    storage
        .lock()
        .expect("lock")
        .execute_batch(
            "CREATE TEMP TABLE commit_guard_parent (id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE commit_guard_child (
                 parent INTEGER REFERENCES commit_guard_parent (id)
                     DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TEMP TRIGGER fail_commit_on_insert AFTER INSERT ON provider_credential
             BEGIN INSERT INTO commit_guard_child VALUES (1); END;
             CREATE TEMP TRIGGER fail_commit_on_update AFTER UPDATE ON provider_credential
             BEGIN INSERT INTO commit_guard_child VALUES (1); END;",
        )
        .expect("commit guard");
}

#[cfg(test)]
mod tests;
