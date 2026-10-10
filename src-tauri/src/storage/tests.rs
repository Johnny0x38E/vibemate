//! Behavior tests for the parent module, kept separate from runtime code.

use super::*;
use std::path::PathBuf;
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique folder under the system temp directory, removed when the test ends.
struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    /// Create a fresh folder. The process id and a counter keep parallel tests apart.
    fn new(name: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vibemate-storage-{name}-{}-{id}",
            std::process::id()
        ));
        // Remove leftovers from an interrupted run so every test starts empty.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test folder");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Cleanup is best effort so a removal problem cannot hide the test result.
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn mcp_migration_preserves_provider_models_and_preferences() {
    let directory = TestDirectory::new("mcp-upgrade");
    let path = directory.path.join(DATABASE_FILE_NAME);
    {
        let old = Storage::open_file(&path, &MIGRATIONS[..7]).expect("v7");
        old.lock().unwrap().execute_batch("INSERT INTO locale_preference VALUES(1,'zh-CN');
            INSERT INTO provider_instance VALUES('0123456789abcdef0123456789abcdef','deepseek','Work','https://api.deepseek.com','chat_completions',1,1,1);
            INSERT INTO provider_model(provider_id,model_id,source,selected,created_at,updated_at) VALUES('0123456789abcdef0123456789abcdef','vendor/model','manual',1,1,1);").unwrap();
    }
    let upgraded = Storage::open_in_directory(&directory.path).unwrap();
    assert_eq!(upgraded.schema_version().unwrap(), latest_schema_version());
    let connection = upgraded.lock().unwrap();
    assert_eq!(
        connection
            .query_row("SELECT preference FROM locale_preference", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "zh-CN"
    );
    assert_eq!(
        connection
            .query_row("SELECT model_id FROM provider_model", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "vendor/model"
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM mcp_definition", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn notion_migration_preserves_existing_preferences_and_supports_restart() {
    use crate::appearance::{Appearance, AppearancePreference, Theme};
    for theme in [
        Theme::Forest,
        Theme::Graphite,
        Theme::Linen,
        Theme::Iris,
        Theme::Ocean,
    ] {
        let directory = TestDirectory::new("notion-upgrade");
        let path = directory.path.join(DATABASE_FILE_NAME);
        let old = Storage::open_file(&path, &MIGRATIONS[..3]).expect("v3 database");
        let previous = AppearancePreference {
            appearance: Appearance::Dark,
            theme,
        };
        crate::appearance::save_appearance_preference(&old, previous).expect("save old pair");
        crate::settings::save_locale_preference(&old, crate::settings::LocalePreference::English)
            .expect("save language");
        old.lock()
            .expect("lock")
            .execute_batch("CREATE TABLE sample (value TEXT); INSERT INTO sample VALUES ('kept');")
            .expect("unrelated row");
        drop(old);
        let upgraded = Storage::open_in_directory(&directory.path).expect("upgrade");
        assert_eq!(
            upgraded.schema_version().expect("schema"),
            latest_version(MIGRATIONS)
        );
        assert_eq!(
            crate::appearance::load_appearance_preference(&upgraded),
            Ok(previous)
        );
        assert_eq!(
            crate::settings::load_locale_preference(&upgraded),
            Ok(crate::settings::LocalePreference::English)
        );
        assert_eq!(read_sample_value(&upgraded), "kept");
        let choice = AppearancePreference {
            appearance: Appearance::Dark,
            theme: Theme::Notion,
        };
        crate::appearance::save_appearance_preference(&upgraded, choice).expect("save Notion");
        assert!(
            upgraded
                .lock()
                .expect("lock")
                .execute("UPDATE appearance_preference SET theme = 'unknown'", [])
                .is_err()
        );
        drop(upgraded);
        let reopened = Storage::open_in_directory(&directory.path).expect("restart");
        assert_eq!(
            crate::appearance::load_appearance_preference(&reopened),
            Ok(choice)
        );
    }
}

#[test]
fn failed_notion_migration_keeps_the_v3_table_and_data() {
    let directory = TestDirectory::new("notion-failed-upgrade");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let old = Storage::open_file(&path, &MIGRATIONS[..3]).expect("v3 database");
    old.lock().expect("lock").execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO appearance_preference VALUES (1, 'dark', 'unknown');").expect("corrupt v3 fixture");
    drop(old);
    assert!(matches!(
        Storage::open_in_directory(&directory.path),
        Err(StorageError::MigrationFailed { version: 4, .. })
    ));
    let raw = Connection::open(&path).expect("original database");
    assert_eq!(read_schema_version(&raw).expect("schema"), 3);
    let theme: String = raw
        .query_row("SELECT theme FROM appearance_preference", [], |row| {
            row.get(0)
        })
        .expect("original row");
    assert_eq!(theme, "unknown");
    let temporary_tables: u32 = raw
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'appearance_preference_v4'",
            [],
            |row| row.get(0),
        )
        .expect("rolled back table");
    assert_eq!(temporary_tables, 0);
}

#[test]
fn providers_migration_from_v4_keeps_preferences_and_adds_an_empty_table() {
    use crate::appearance::{Appearance, AppearancePreference, Theme};
    let directory = TestDirectory::new("providers-upgrade");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let old = Storage::open_file(&path, &MIGRATIONS[..4]).expect("v4 database");
    let appearance = AppearancePreference {
        appearance: Appearance::Light,
        theme: Theme::Notion,
    };
    crate::appearance::save_appearance_preference(&old, appearance).expect("save appearance");
    crate::settings::save_locale_preference(
        &old,
        crate::settings::LocalePreference::SimplifiedChinese,
    )
    .expect("save language");
    drop(old);

    // Stop at v5 so this test keeps checking exactly the v4 -> v5 step.
    let upgraded = Storage::open_file(&path, &MIGRATIONS[..5]).expect("upgrade to v5");
    assert_eq!(upgraded.schema_version().expect("schema"), 5);
    assert_eq!(
        crate::appearance::load_appearance_preference(&upgraded),
        Ok(appearance)
    );
    assert_eq!(
        crate::settings::load_locale_preference(&upgraded),
        Ok(crate::settings::LocalePreference::SimplifiedChinese)
    );
    drop(upgraded);
    // Selected-model counts need the v6 `provider_model` table; finish migrating before listing.
    let current = Storage::open_file(&path, MIGRATIONS).expect("upgrade to current");
    let page = crate::providers::list_providers(
        &current,
        &crate::providers::ListProvidersRequest {
            after: None,
            limit: 10,
        },
    )
    .expect("list after upgrade");
    assert!(page.items.is_empty());
}

/// Insert a provider row directly, as a P10 build without keys would have saved it.
fn insert_keyless_provider(connection: &Connection, id: &str) {
    connection
        .execute(
            "INSERT INTO provider_instance
                 (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
             VALUES (?1, 'deepseek', 'Legacy', 'https://api.deepseek.com', 'chat_completions', 1, 10, 10)",
            [id],
        )
        .expect("insert provider");
}

#[test]
fn credential_migration_from_v5_keeps_providers_and_preferences() {
    let directory = TestDirectory::new("credential-upgrade");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let old = Storage::open_file(&path, &MIGRATIONS[..5]).expect("v5 database");
    crate::settings::save_locale_preference(&old, crate::settings::LocalePreference::English)
        .expect("save language");
    let legacy_id = "a".repeat(32);
    insert_keyless_provider(&old.lock().expect("lock"), &legacy_id);
    drop(old);

    let upgraded = Storage::open_file(&path, &MIGRATIONS[..6]).expect("upgrade to v6");
    assert_eq!(upgraded.schema_version().expect("schema"), 6);
    assert_eq!(
        crate::settings::load_locale_preference(&upgraded),
        Ok(crate::settings::LocalePreference::English)
    );
    let connection = upgraded.lock().expect("lock");
    let providers: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM provider_instance WHERE id = ?1",
            [&legacy_id],
            |row| row.get(0),
        )
        .expect("count providers");
    assert_eq!(providers, 1, "the keyless legacy row is kept");
    let credentials: u32 = connection
        .query_row("SELECT COUNT(*) FROM provider_credential", [], |row| {
            row.get(0)
        })
        .expect("count credentials");
    assert_eq!(credentials, 0, "no reference is invented for it");
}

#[test]
fn credential_references_are_tied_to_existing_provider_ids() {
    let directory = TestDirectory::new("credential-constraints");
    let storage = Storage::open_in_directory(&directory.path).expect("open");
    let connection = storage.lock().expect("lock");
    let id = "b".repeat(32);
    let other = "c".repeat(32);
    insert_keyless_provider(&connection, &id);
    insert_keyless_provider(&connection, &other);
    let insert = |provider_id: &str, reference: &str| {
        connection.execute(
            "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
             VALUES (?1, ?2, 20)",
            [provider_id, reference],
        )
    };

    // A reference must be exactly `provider-<id>`: not a name, not another ID.
    assert!(insert(&id, "Legacy").is_err());
    assert!(insert(&id, &format!("provider-{other}")).is_err());
    // The provider must exist (foreign key).
    let missing = "d".repeat(32);
    assert!(insert(&missing, &format!("provider-{missing}")).is_err());
    assert!(insert(&id, &format!("provider-{id}")).is_ok());
    // One reference per provider.
    assert!(insert(&id, &format!("provider-{id}")).is_err());

    // Deleting the provider removes its reference with it.
    connection
        .execute("DELETE FROM provider_instance WHERE id = ?1", [&id])
        .expect("delete provider");
    let remaining: u32 = connection
        .query_row("SELECT COUNT(*) FROM provider_credential", [], |row| {
            row.get(0)
        })
        .expect("count credentials");
    assert_eq!(remaining, 0);
}

#[test]
fn model_migration_from_v6_keeps_providers_and_key_references() {
    let directory = TestDirectory::new("model-upgrade");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let old = Storage::open_file(&path, &MIGRATIONS[..6]).expect("v6 database");
    let id = "e".repeat(32);
    {
        let connection = old.lock().expect("lock");
        insert_keyless_provider(&connection, &id);
        connection
            .execute(
                "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
                 VALUES (?1, 'provider-' || ?1, 20)",
                [&id],
            )
            .expect("insert reference");
    }
    drop(old);

    let upgraded = Storage::open_file(&path, &MIGRATIONS[..7]).expect("upgrade to v7");
    assert_eq!(upgraded.schema_version().expect("schema"), 7);
    let connection = upgraded.lock().expect("lock");
    let count = |table: &str| -> u32 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rows")
    };
    assert_eq!(count("provider_instance"), 1);
    assert_eq!(count("provider_credential"), 1);
    assert_eq!(count("provider_model"), 0);
    assert_eq!(count("provider_model_fetch"), 0);
}

#[test]
fn model_rows_are_constrained_and_follow_their_provider() {
    let directory = TestDirectory::new("model-constraints");
    let storage = Storage::open_in_directory(&directory.path).expect("open");
    let connection = storage.lock().expect("lock");
    let id = "f".repeat(32);
    insert_keyless_provider(&connection, &id);
    let insert = |provider_id: &str, model_id: &str, source: &str, endpoints: Option<&str>| {
        connection.execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, supported_endpoints,
                  created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, 10, 10)",
            rusqlite::params![provider_id, model_id, source, endpoints],
        )
    };

    // IDs keep `/`, `.` and `:`; the JSON column must hold valid JSON.
    assert!(
        insert(
            &id,
            "mistral/mistral-large-4",
            "fetched",
            Some(r#"["/messages"]"#)
        )
        .is_ok()
    );
    assert!(insert(&id, "ling-3.1-flash:free", "manual", None).is_ok());
    assert!(insert(&id, "bad-json", "fetched", Some("[")).is_err());
    assert!(insert(&id, "bad-source", "imported", None).is_err());
    assert!(insert(&id, "", "manual", None).is_err());
    assert!(insert(&id, &"m".repeat(257), "manual", None).is_err());
    // One row per (provider, model); the provider must exist.
    assert!(insert(&id, "ling-3.1-flash:free", "fetched", None).is_err());
    assert!(insert(&"0".repeat(32), "other", "manual", None).is_err());
    // A model cannot be missing without having been seen.
    assert!(
        connection
            .execute(
                "INSERT INTO provider_model
                     (provider_id, model_id, source, selected, created_at, updated_at,
                      missing_since)
                 VALUES (?1, 'never-seen', 'fetched', 1, 10, 10, 20)",
                [&id],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO provider_model_fetch (provider_id, fetched_at, complete, listed_count)
             VALUES (?1, 30, 1, 2)",
            [&id],
        )
        .expect("insert fetch record");

    connection
        .execute("DELETE FROM provider_instance WHERE id = ?1", [&id])
        .expect("delete provider");
    for table in ["provider_model", "provider_model_fetch"] {
        let remaining: u32 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(remaining, 0, "{table} follows its provider");
    }
}

/// Read the single row written by the test migrations.
fn read_sample_value(storage: &Storage) -> String {
    let connection = storage.lock().expect("lock test connection");
    connection
        .query_row("SELECT value FROM sample", [], |row| row.get(0))
        .expect("read sample row")
}

#[test]
fn first_open_creates_the_folder_and_database_at_the_latest_version() {
    let directory = TestDirectory::new("first-open");
    let app_data = directory.path.join("app-data");

    let storage = Storage::open_in_directory(&app_data).expect("first open");

    assert!(app_data.join(DATABASE_FILE_NAME).is_file());
    assert_eq!(
        storage.schema_version().expect("read version"),
        latest_version(MIGRATIONS)
    );
}

#[test]
fn reopening_keeps_rows_and_skips_applied_migrations() {
    let directory = TestDirectory::new("reopen");
    let path = directory.path.join(DATABASE_FILE_NAME);
    // Running this SQL a second time would fail, so a repeated run would be visible.
    let migrations = [Migration {
        version: 1,
        sql: "CREATE TABLE sample (value TEXT NOT NULL); INSERT INTO sample (value) VALUES ('kept');",
    }];

    Storage::open_file(&path, &migrations).expect("first open");
    let storage = Storage::open_file(&path, &migrations).expect("second open must skip version 1");

    assert_eq!(storage.schema_version().expect("read version"), 1);
    assert_eq!(read_sample_value(&storage), "kept");
}

#[test]
fn failed_migration_rolls_back_and_keeps_previous_data_readable() {
    let directory = TestDirectory::new("failed-migration");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let first = Migration {
        version: 1,
        sql: "CREATE TABLE sample (value TEXT NOT NULL); INSERT INTO sample (value) VALUES ('kept');",
    };
    // The first statement is valid and the second fails. The rollback must undo both.
    let broken = Migration {
        version: 2,
        sql: "CREATE TABLE partial (id INTEGER); INSERT INTO missing_table (id) VALUES (1);",
    };

    let error = Storage::open_file(&path, &[first, broken])
        .err()
        .expect("broken migration must fail");

    assert!(matches!(
        error,
        StorageError::MigrationFailed { version: 2, .. }
    ));
    // The message is shown to users, so it must not reveal SQL or the file location.
    let message = error.to_string();
    assert!(!message.contains("missing_table"));
    assert!(!message.contains(&path.display().to_string()));

    // Reopening with only the first migration shows that version 1 and its data survived.
    let storage = Storage::open_file(&path, &[first]).expect("previous schema must still open");
    assert_eq!(storage.schema_version().expect("read version"), 1);
    assert_eq!(read_sample_value(&storage), "kept");
    let connection = storage.lock().expect("lock test connection");
    let partial_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'partial'",
            [],
            |row| row.get(0),
        )
        .expect("count partial table");
    assert_eq!(partial_tables, 0);
}

#[test]
fn database_from_a_newer_build_is_rejected_without_changes() {
    let directory = TestDirectory::new("newer-schema");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let known = [Migration {
        version: 1,
        sql: "",
    }];
    {
        let raw = Connection::open(&path).expect("create raw database");
        raw.pragma_update(None, "user_version", 9)
            .expect("simulate a newer schema");
    }

    let error = Storage::open_file(&path, &known)
        .err()
        .expect("newer schema must be rejected");

    assert!(matches!(
        error,
        StorageError::NewerSchema {
            found: 9,
            supported: 1
        }
    ));
    let raw = Connection::open(&path).expect("reopen raw database");
    let version: u32 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read version");
    assert_eq!(version, 9);
}

#[test]
fn upgrade_applies_each_missing_version_and_keeps_existing_rows() {
    let directory = TestDirectory::new("multi-step");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let first = Migration {
        version: 1,
        sql: "CREATE TABLE sample (value TEXT NOT NULL); INSERT INTO sample (value) VALUES ('kept');",
    };
    let second = Migration {
        version: 2,
        sql: "ALTER TABLE sample ADD COLUMN note TEXT;",
    };
    let third = Migration {
        version: 3,
        sql: "CREATE TABLE later (id INTEGER PRIMARY KEY);",
    };
    Storage::open_file(&path, &[first]).expect("open at version 1");

    let storage = Storage::open_file(&path, &[first, second, third]).expect("upgrade to version 3");

    assert_eq!(storage.schema_version().expect("read version"), 3);
    assert_eq!(read_sample_value(&storage), "kept");
    let connection = storage.lock().expect("lock test connection");
    let later_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'later'",
            [],
            |row| row.get(0),
        )
        .expect("count later table");
    assert_eq!(later_tables, 1);
}

#[test]
fn concurrent_openers_apply_each_migration_exactly_once() {
    let directory = TestDirectory::new("concurrent");
    let path = directory.path.join(DATABASE_FILE_NAME);
    // Applying this twice would fail on CREATE TABLE, and the row count would double.
    let migrations = [Migration {
        version: 1,
        sql: "CREATE TABLE sample (value TEXT NOT NULL); INSERT INTO sample (value) VALUES ('once');",
    }];
    let barrier = Barrier::new(4);

    // Four threads open the same file at the same moment, like four app instances.
    let opened: Vec<bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    Storage::open_file(&path, &migrations).is_ok()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("opener thread finished"))
            .collect()
    });

    assert!(opened.iter().all(|succeeded| *succeeded));
    let storage = Storage::open_file(&path, &migrations).expect("reopen after the race");
    assert_eq!(storage.schema_version().expect("read version"), 1);
    let connection = storage.lock().expect("lock test connection");
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM sample", [], |row| row.get(0))
        .expect("count sample rows");
    assert_eq!(rows, 1);
}

#[test]
fn shipped_migrations_are_numbered_consecutively_from_one() {
    for (index, migration) in MIGRATIONS.iter().enumerate() {
        let expected = u32::try_from(index + 1).expect("migration count fits in u32");
        assert_eq!(migration.version, expected);
    }
}

#[test]
fn unreadable_database_is_reported_and_left_unchanged() {
    let directory = TestDirectory::new("unreadable");
    let path = directory.path.join(DATABASE_FILE_NAME);
    let original = b"this file is not a SQLite database".to_vec();
    fs::write(&path, &original).expect("write unreadable file");

    // Startup must still produce a status, so the window can open.
    let status = StorageStatus::open_in_directory(&directory.path);

    assert!(matches!(status.storage(), Err(StorageError::Database(_))));
    assert_eq!(fs::read(&path).expect("read file back"), original);
}

#[test]
fn storage_status_can_be_shared_between_threads() {
    // Tauri stores the status as managed state across threads. This stops compiling
    // if the status, or the storage it wraps, loses `Send` or `Sync`.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StorageStatus>();
}
