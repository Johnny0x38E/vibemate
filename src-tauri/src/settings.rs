//! Non-sensitive application preferences persisted in private SQLite storage.
//!
//! This module has no Tauri dependency. Validated language and Agent
//! auto-version-probe preferences are stored here; credentials and agent
//! configuration belong to separate modules.

use crate::storage::Storage;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

/// A saved choice, distinct from the UI locale resolved from the system language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalePreference {
    /// Resolve the system language on each application startup.
    #[serde(rename = "system")]
    System,
    /// Use Simplified Chinese regardless of the system language.
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    /// Use English regardless of the system language.
    #[serde(rename = "en")]
    English,
}

impl LocalePreference {
    fn stored_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::SimplifiedChinese => "zh-CN",
            Self::English => "en",
        }
    }

    fn from_stored_value(value: &str) -> Result<Self, SettingsError> {
        match value {
            "system" => Ok(Self::System),
            "zh-CN" => Ok(Self::SimplifiedChinese),
            "en" => Ok(Self::English),
            _ => Err(SettingsError::InvalidPreference),
        }
    }
}

/// Stable IPC error codes. No SQL, path, preference contents, or platform error
/// is carried across the boundary; the frontend supplies localized explanations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsError {
    /// Startup failed or the connection lock was poisoned; no operation was made.
    StorageUnavailable,
    /// SQLite could not read the preference; this is not a first-start default.
    ReadFailed,
    /// SQLite rejected the write, leaving the earlier committed value unchanged.
    WriteFailed,
    /// A stored value is outside the supported choices; do not silently replace it.
    InvalidPreference,
    /// A blocking command task could not finish; its result is unknown.
    OperationFailed,
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => "Preference storage is unavailable.",
            Self::ReadFailed => "The language preference could not be read.",
            Self::WriteFailed => "The language preference could not be saved.",
            Self::InvalidPreference => "The stored language preference is invalid.",
            Self::OperationFailed => "The language preference operation could not finish.",
        })
    }
}

impl std::error::Error for SettingsError {}

/// Read a saved choice, or follow the system when no preference row exists.
/// This read never writes defaults or replaces invalid data.
///
/// # Errors
/// Returns `StorageUnavailable` for a poisoned lock, `ReadFailed` for a database
/// failure, or `InvalidPreference` for an unsupported stored value.
pub fn load_locale_preference(storage: &Storage) -> Result<LocalePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    let stored: Option<String> = connection
        .query_row(
            "SELECT preference FROM locale_preference WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| SettingsError::ReadFailed)?;
    match stored {
        Some(value) => LocalePreference::from_stored_value(&value),
        None => Ok(LocalePreference::System),
    }
}

/// Save a validated choice and return it only after SQLite commits.
/// The single UPSERT is atomic; unrelated data is untouched. No credential data
/// or browser storage is involved.
///
/// # Errors
/// Returns `StorageUnavailable` for a poisoned lock or `WriteFailed` when SQLite
/// cannot commit the statement. It never reports an unsuccessful write as saved.
pub fn save_locale_preference(
    storage: &Storage,
    preference: LocalePreference,
) -> Result<LocalePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    connection
        .execute(
            "INSERT INTO locale_preference (id, preference) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET preference = excluded.preference",
            [preference.stored_value()],
        )
        .map_err(|_| SettingsError::WriteFailed)?;
    Ok(preference)
}

/// Whether the user allowed Agent CLI `--version` probes to run without a
/// per-run click. This is informed consent only: it never stores a version
/// number, and probe results stay in window memory.
///
/// `rename_all = "camelCase"` matches the frontend wire format
/// (`{ "autoProbe": boolean }`) to the Rust field name `auto_probe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoVersionProbePreference {
    /// `true` lets discovery and detail views trigger fixed `--version` probes.
    pub auto_probe: bool,
}

impl Default for AutoVersionProbePreference {
    /// Auto-probe is off until the user turns it on, matching the manual-only
    /// default documented in `docs/plans/agent-version-probe.md`.
    fn default() -> Self {
        Self { auto_probe: false }
    }
}

/// Read the saved auto-probe choice, or report `false` when no row exists.
/// This read never writes defaults or replaces invalid data.
///
/// # Errors
/// Returns `StorageUnavailable` for a poisoned lock or `ReadFailed` for a
/// database failure. An unsupported stored value is `InvalidPreference`.
pub fn load_auto_version_probe(
    storage: &Storage,
) -> Result<AutoVersionProbePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    let stored: Option<i64> = connection
        .query_row(
            "SELECT auto_probe FROM agent_version_preference WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| SettingsError::ReadFailed)?;
    match stored {
        Some(0) => Ok(AutoVersionProbePreference { auto_probe: false }),
        Some(1) => Ok(AutoVersionProbePreference { auto_probe: true }),
        Some(_) => Err(SettingsError::InvalidPreference),
        None => Ok(AutoVersionProbePreference::default()),
    }
}

/// Persist the auto-probe choice and return it only after SQLite commits.
/// The single UPSERT is atomic; unrelated data is untouched.
///
/// # Errors
/// Returns `StorageUnavailable` for a poisoned lock or `WriteFailed` when SQLite
/// cannot commit the statement. It never reports an unsuccessful write as saved.
pub fn save_auto_version_probe(
    storage: &Storage,
    preference: AutoVersionProbePreference,
) -> Result<AutoVersionProbePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    connection
        .execute(
            "INSERT INTO agent_version_preference (id, auto_probe) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET auto_probe = excluded.auto_probe",
            [i64::from(preference.auto_probe)],
        )
        .map_err(|_| SettingsError::WriteFailed)?;
    Ok(preference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use rusqlite::Connection;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("vibemate-locale-{}-{id}", std::process::id()));
            fs::create_dir(&path).expect("create unique test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn locale_preference_defaults_to_system_without_writing_a_row() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        assert_eq!(
            load_locale_preference(&storage),
            Ok(LocalePreference::System)
        );
        let count: u32 = storage
            .lock()
            .expect("lock")
            .query_row("SELECT COUNT(*) FROM locale_preference", [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(count, 0);
    }

    #[test]
    fn locale_preference_saves_replaces_and_reopens_all_supported_choices() {
        let directory = TestDirectory::new();
        for preference in [
            LocalePreference::SimplifiedChinese,
            LocalePreference::English,
            LocalePreference::System,
        ] {
            let storage = Storage::open_in_directory(&directory.0).expect("open storage");
            assert_eq!(save_locale_preference(&storage, preference), Ok(preference));
            drop(storage);
            let reopened = Storage::open_in_directory(&directory.0).expect("reopen storage");
            assert_eq!(load_locale_preference(&reopened), Ok(preference));
            let count: u32 = reopened
                .lock()
                .expect("lock")
                .query_row("SELECT COUNT(*) FROM locale_preference", [], |row| {
                    row.get(0)
                })
                .expect("count rows");
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn locale_preference_upgrade_from_v1_preserves_unrelated_data() {
        let directory = TestDirectory::new();
        let connection =
            Connection::open(directory.0.join("vibemate.sqlite3")).expect("old database");
        connection.execute_batch("PRAGMA user_version = 1; CREATE TABLE sample (value TEXT); INSERT INTO sample VALUES ('kept');").expect("v1 data");
        drop(connection);
        let storage = Storage::open_in_directory(&directory.0).expect("upgrade");
        assert_eq!(
            storage.schema_version().expect("version"),
            crate::storage::latest_schema_version()
        );
        assert_eq!(
            load_locale_preference(&storage),
            Ok(LocalePreference::System)
        );
        let value: String = storage
            .lock()
            .expect("lock")
            .query_row("SELECT value FROM sample", [], |row| row.get(0))
            .expect("original row");
        assert_eq!(value, "kept");
    }

    #[test]
    fn locale_preference_failed_save_keeps_the_previous_value() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        save_locale_preference(&storage, LocalePreference::English).expect("save old value");
        // query_only causes a real SQLite write error without changing file permissions,
        // which would behave differently under Windows or elevated test accounts.
        storage
            .lock()
            .expect("lock")
            .pragma_update(None, "query_only", true)
            .expect("make connection read-only");
        assert_eq!(
            save_locale_preference(&storage, LocalePreference::SimplifiedChinese),
            Err(SettingsError::WriteFailed)
        );
        assert_eq!(
            load_locale_preference(&storage),
            Ok(LocalePreference::English)
        );
    }

    #[test]
    fn locale_preference_read_failure_does_not_return_a_default() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        storage
            .lock()
            .expect("lock")
            .execute_batch("DROP TABLE locale_preference")
            .expect("simulate broken schema");
        assert_eq!(
            load_locale_preference(&storage),
            Err(SettingsError::ReadFailed)
        );
    }

    #[test]
    fn locale_preference_invalid_stored_value_is_reported_not_overwritten() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        let connection = storage.lock().expect("lock");
        connection.execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO locale_preference VALUES (1, 'invalid');").expect("simulate invalid data");
        drop(connection);
        assert_eq!(
            load_locale_preference(&storage),
            Err(SettingsError::InvalidPreference)
        );
        let value: String = storage
            .lock()
            .expect("lock")
            .query_row("SELECT preference FROM locale_preference", [], |row| {
                row.get(0)
            })
            .expect("stored value");
        assert_eq!(value, "invalid");
    }

    #[test]
    fn locale_preference_schema_rejects_unknown_values_and_extra_rows() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        let connection = storage.lock().expect("lock");
        assert!(
            connection
                .execute("INSERT INTO locale_preference VALUES (1, 'fr')", [])
                .is_err()
        );
        assert!(
            connection
                .execute("INSERT INTO locale_preference VALUES (2, 'en')", [])
                .is_err()
        );
    }

    #[test]
    fn locale_preference_poisoned_storage_rejects_reads_and_writes() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        // A panicking operation poisons the mutex. Later operations must fail
        // safely, not guess whether the interrupted database work completed.
        std::thread::scope(|scope| {
            let result = scope
                .spawn(|| {
                    let _connection = storage.lock().expect("lock");
                    panic!("synthetic interrupted operation");
                })
                .join();
            assert!(result.is_err());
        });
        assert_eq!(
            load_locale_preference(&storage),
            Err(SettingsError::StorageUnavailable)
        );
        assert_eq!(
            save_locale_preference(&storage, LocalePreference::English),
            Err(SettingsError::StorageUnavailable)
        );
    }

    #[test]
    fn locale_preference_deserialization_accepts_only_the_wire_values() {
        for (input, expected) in [
            ("system", LocalePreference::System),
            ("zh-CN", LocalePreference::SimplifiedChinese),
            ("en", LocalePreference::English),
        ] {
            let value = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(input);
            assert_eq!(
                LocalePreference::deserialize(value).expect("valid preference"),
                expected
            );
        }
        let unknown = serde::de::value::StrDeserializer::<serde::de::value::Error>::new("fr");
        assert!(LocalePreference::deserialize(unknown).is_err());
    }

    #[test]
    fn auto_version_probe_defaults_to_off_without_writing_a_row() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        assert_eq!(
            load_auto_version_probe(&storage),
            Ok(AutoVersionProbePreference::default())
        );
        let count: u32 = storage
            .lock()
            .expect("lock")
            .query_row("SELECT COUNT(*) FROM agent_version_preference", [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(count, 0);
    }

    #[test]
    fn auto_version_probe_saves_both_choices_and_reopens() {
        let directory = TestDirectory::new();
        for auto_probe in [true, false, true] {
            let storage = Storage::open_in_directory(&directory.0).expect("open storage");
            let preference = AutoVersionProbePreference { auto_probe };
            assert_eq!(
                save_auto_version_probe(&storage, preference),
                Ok(preference)
            );
            drop(storage);
            let reopened = Storage::open_in_directory(&directory.0).expect("reopen storage");
            assert_eq!(load_auto_version_probe(&reopened), Ok(preference));
            let count: u32 = reopened
                .lock()
                .expect("lock")
                .query_row("SELECT COUNT(*) FROM agent_version_preference", [], |row| {
                    row.get(0)
                })
                .expect("count rows");
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn auto_version_probe_failed_save_keeps_the_previous_value() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        save_auto_version_probe(&storage, AutoVersionProbePreference { auto_probe: true })
            .expect("save old value");
        storage
            .lock()
            .expect("lock")
            .pragma_update(None, "query_only", true)
            .expect("make connection read-only");
        assert_eq!(
            save_auto_version_probe(&storage, AutoVersionProbePreference { auto_probe: false }),
            Err(SettingsError::WriteFailed)
        );
        assert_eq!(
            load_auto_version_probe(&storage),
            Ok(AutoVersionProbePreference { auto_probe: true })
        );
    }

    #[test]
    fn auto_version_probe_invalid_stored_value_is_reported_not_overwritten() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        let connection = storage.lock().expect("lock");
        connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 INSERT INTO agent_version_preference (id, auto_probe) VALUES (1, 2)",
            )
            .expect("seed invalid row");
        drop(connection);
        assert_eq!(
            load_auto_version_probe(&storage),
            Err(SettingsError::InvalidPreference)
        );
    }

    #[test]
    fn auto_version_probe_schema_rejects_unknown_values_and_extra_rows() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open storage");
        let connection = storage.lock().expect("lock");
        assert!(
            connection
                .execute(
                    "INSERT INTO agent_version_preference (id, auto_probe) VALUES (1, 2)",
                    []
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO agent_version_preference (id, auto_probe) VALUES (2, 1)",
                    []
                )
                .is_err()
        );
    }

    #[test]
    fn auto_version_probe_wire_format_uses_camel_case() {
        // The frontend sends and reads `{ "autoProbe": boolean }`. Without
        // `rename_all = "camelCase"` serde would use the Rust field name
        // `auto_probe`, and every IPC call would fail to deserialize.
        let enabled = AutoVersionProbePreference { auto_probe: true };
        let json = serde_json::to_value(enabled).expect("serialize");
        assert_eq!(json, serde_json::json!({ "autoProbe": true }));
        let parsed: AutoVersionProbePreference = serde_json::from_value(json).expect("deserialize");
        assert_eq!(parsed, enabled);

        let disabled = AutoVersionProbePreference { auto_probe: false };
        let json = serde_json::to_value(disabled).expect("serialize");
        assert_eq!(json, serde_json::json!({ "autoProbe": false }));
        let parsed: AutoVersionProbePreference = serde_json::from_value(json).expect("deserialize");
        assert_eq!(parsed, disabled);
    }
}
