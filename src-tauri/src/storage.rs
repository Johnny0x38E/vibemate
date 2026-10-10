//! Private configuration storage backed by SQLite.
//!
//! This module owns the database file, its schema version, and the one connection
//! the app shares. It does not depend on Tauri, so its behavior can be tested with
//! temporary folders. Secrets never belong here: API keys go to the OS credential
//! store, and this database keeps only non-secret settings and credential references.
//!
//! SQLite keeps the whole database in one file (the default rollback journal). That
//! makes later backup and restore steps simpler than with a write-ahead log.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

/// File name of the database inside the app-data folder.
const DATABASE_FILE_NAME: &str = "vibemate.sqlite3";

/// How long a connection waits for another process's write lock before failing.
///
/// Two app instances can open the same file. Waiting briefly lets the other
/// instance finish its write instead of failing with "database is locked".
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// One ordered schema change.
///
/// `sql` and the version bump run in one transaction. If the SQL fails, SQLite
/// rolls back everything, so the previous schema and its data stay readable.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Schema version reached after this migration succeeds. Versions start at 1.
    pub version: u32,
    /// SQL statements executed as one batch.
    pub sql: &'static str,
}

/// Schema changes shipped with this build, oldest first.
///
/// Version 1 only marks the database as versioned. Business tables arrive with
/// the task that first needs them; each new migration appends one version.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: "",
    },
    Migration {
        version: 2,
        // A singleton table makes the only current preference explicit instead of
        // introducing an unvalidated key/value settings store before it is needed.
        sql: "CREATE TABLE locale_preference (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            preference TEXT NOT NULL CHECK (preference IN ('system', 'zh-CN', 'en'))
        ) STRICT;",
    },
    Migration {
        version: 3,
        sql: "CREATE TABLE appearance_preference (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
            theme TEXT NOT NULL CHECK (theme IN ('forest', 'graphite', 'linen', 'iris', 'ocean'))
        ) STRICT;",
    },
    Migration {
        version: 4,
        // SQLite cannot extend a CHECK constraint in place. Rebuild only this
        // owned table and copy the saved pair inside the migration transaction.
        sql: "CREATE TABLE appearance_preference_v4 (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
            theme TEXT NOT NULL CHECK (theme IN ('forest', 'graphite', 'linen', 'iris', 'ocean', 'notion'))
        ) STRICT;
        INSERT INTO appearance_preference_v4 SELECT id, appearance, theme FROM appearance_preference;
        DROP TABLE appearance_preference;
        ALTER TABLE appearance_preference_v4 RENAME TO appearance_preference;",
    },
    Migration {
        version: 5,
        // Non-secret provider instances (see `crate::providers`). The CHECKs repeat
        // the Rust validation so a row written by other means cannot hold an unknown
        // kind or protocol. Which protocols a kind may use is decided in Rust, so it
        // can widen without rebuilding this table. The index serves the stable
        // `ORDER BY created_at, id` list order.
        sql: "CREATE TABLE provider_instance (
            id TEXT PRIMARY KEY CHECK (length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
            kind TEXT NOT NULL CHECK (kind IN ('command-code', 'deepseek', 'openrouter')),
            display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 64),
            base_url TEXT NOT NULL CHECK (length(base_url) BETWEEN 1 AND 2048),
            protocol TEXT NOT NULL CHECK (protocol IN ('chat_completions', 'responses', 'anthropic_messages')),
            revision INTEGER NOT NULL CHECK (revision >= 1),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            CHECK (updated_at >= created_at)
        ) STRICT;
        CREATE INDEX provider_instance_list_order ON provider_instance (created_at, id);",
    },
    Migration {
        version: 6,
        // One row per provider whose API key is in the OS credential store. The row
        // holds only the non-secret entry name, never the key. Status reads come from
        // this table, so showing a page never touches the credential store.
        // - The CHECK ties the name to the ID (`provider-<id>`), never to a display name.
        // - Deleting a provider removes its row (`foreign_keys` is on); the
        //   credential entry itself must be deleted by Rust before that.
        // Existing provider rows are left alone. Rows saved before keys existed have
        // no row here and report a missing key. A constraint cannot require a child
        // row, so "every provider has a key" is kept by the create flow instead.
        sql: "CREATE TABLE provider_credential (
            provider_id TEXT PRIMARY KEY
                REFERENCES provider_instance(id) ON DELETE CASCADE,
            credential_ref TEXT NOT NULL UNIQUE
                CHECK (credential_ref = 'provider-' || provider_id),
            updated_at INTEGER NOT NULL
        ) STRICT;",
    },
    Migration {
        version: 7,
        // Models per provider instance (see `crate::models`), fetched from the
        // vendor's list or added by hand, and the outcome of the last fetch.
        // - The primary key is the pair (provider, model ID); `WITHOUT ROWID` stores
        //   rows in that order, which also serves the `ORDER BY model_id` list.
        // - `model_id` keeps the vendor's exact text, including `/`, `.` and `:`.
        // - `selected` is the user's choice; merging fetched lists never changes it.
        // - `last_seen_at` is set when a complete fetch listed the model and
        //   `missing_since` when a later complete fetch no longer did.
        // - The JSON columns hold short string arrays validated in Rust.
        // - Deleting a provider deletes its models and fetch record.
        sql: "CREATE TABLE provider_model (
            provider_id TEXT NOT NULL
                REFERENCES provider_instance(id) ON DELETE CASCADE,
            model_id TEXT NOT NULL CHECK (length(model_id) BETWEEN 1 AND 256),
            source TEXT NOT NULL CHECK (source IN ('fetched', 'manual')),
            selected INTEGER NOT NULL CHECK (selected IN (0, 1)),
            alias TEXT CHECK (alias IS NULL OR length(alias) BETWEEN 1 AND 64),
            upstream_name TEXT CHECK (upstream_name IS NULL OR length(upstream_name) BETWEEN 1 AND 256),
            context_window INTEGER CHECK (context_window IS NULL OR context_window > 0),
            max_output_tokens INTEGER CHECK (max_output_tokens IS NULL OR max_output_tokens > 0),
            input_modalities TEXT CHECK (input_modalities IS NULL OR json_valid(input_modalities)),
            output_modalities TEXT CHECK (output_modalities IS NULL OR json_valid(output_modalities)),
            supported_endpoints TEXT CHECK (supported_endpoints IS NULL OR json_valid(supported_endpoints)),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_seen_at INTEGER,
            missing_since INTEGER,
            PRIMARY KEY (provider_id, model_id),
            CHECK (updated_at >= created_at),
            CHECK (missing_since IS NULL OR last_seen_at IS NOT NULL)
        ) STRICT, WITHOUT ROWID;
        CREATE TABLE provider_model_fetch (
            provider_id TEXT PRIMARY KEY
                REFERENCES provider_instance(id) ON DELETE CASCADE,
            fetched_at INTEGER NOT NULL,
            complete INTEGER NOT NULL CHECK (complete IN (0, 1)),
            listed_count INTEGER NOT NULL CHECK (listed_count >= 0)
        ) STRICT;",
    },
    Migration {
        version: 8,
        // Central definitions contain transport metadata only. Every env/header
        // value is an OS entry; old entries are queued in the metadata transaction
        // so failed post-commit cleanup can be retried without exposing values.
        sql: "CREATE TABLE mcp_definition (
            id TEXT PRIMARY KEY CHECK(length(id)=32 AND id NOT GLOB '*[^0-9a-f]*'),
            display_name TEXT NOT NULL CHECK(length(display_name) BETWEEN 1 AND 64),
            server_name TEXT NOT NULL CHECK(length(server_name) BETWEEN 1 AND 64),
            namespace TEXT NOT NULL UNIQUE,
            enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
            revision INTEGER NOT NULL CHECK(revision >= 1),
            connection TEXT NOT NULL CHECK(json_valid(connection)),
            created_at INTEGER NOT NULL CHECK(created_at >= 0),
            updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
        ) STRICT;
        CREATE TABLE mcp_secret (
            definition_id TEXT NOT NULL REFERENCES mcp_definition(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK(kind IN ('env','header')),
            name TEXT NOT NULL,
            credential_ref TEXT NOT NULL UNIQUE,
            PRIMARY KEY(definition_id,kind,name)
        ) STRICT, WITHOUT ROWID;
        CREATE TABLE mcp_cleanup (
            definition_id TEXT NOT NULL,
            credential_ref TEXT PRIMARY KEY
        ) STRICT;",
    },
];

/// Errors from opening or migrating the private database.
///
/// The `Display` text is safe to show to users: it never includes file paths, SQL,
/// or secret values. SQLite details remain available through `source()` for logs.
#[derive(Debug)]
pub enum StorageError {
    /// The app-data folder could not be created.
    DataDirectory(std::io::Error),
    /// Tauri could not resolve the app-data folder, so no database path exists.
    LocateDataDirectory,
    /// SQLite could not open the file or read its schema version.
    Database(rusqlite::Error),
    /// A migration failed and rolled back; the database stays at the previous version.
    MigrationFailed {
        version: u32,
        source: rusqlite::Error,
    },
    /// The file was written by a newer build, so this build must not modify it.
    NewerSchema { found: u32, supported: u32 },
    /// Another operation panicked while holding the connection.
    Unavailable,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataDirectory(_) => {
                formatter.write_str("vibemate could not create its private data folder.")
            }
            Self::LocateDataDirectory => formatter
                .write_str("vibemate could not find its private data folder on this device."),
            Self::Database(_) => {
                formatter.write_str("vibemate could not open its private configuration database.")
            }
            Self::MigrationFailed { version, .. } => write!(
                formatter,
                "vibemate could not upgrade its configuration database to schema version {version}. The existing data was left unchanged."
            ),
            Self::NewerSchema { found, supported } => write!(
                formatter,
                "The configuration database uses schema version {found}, but this vibemate build supports up to version {supported}. Install a newer vibemate build."
            ),
            Self::Unavailable => formatter.write_str(
                "The configuration database is unavailable after an internal error. Restart vibemate.",
            ),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DataDirectory(error) => Some(error),
            Self::Database(error) | Self::MigrationFailed { source: error, .. } => Some(error),
            Self::LocateDataDirectory | Self::NewerSchema { .. } | Self::Unavailable => None,
        }
    }
}

/// The app's private SQLite database.
///
/// Access is serialized by one `Mutex<Connection>`. Each method locks it for a
/// short operation, so commands on different threads never use the connection at
/// the same time. SQLite also serializes writes between processes, and
/// `BUSY_TIMEOUT` lets a second app instance wait for its turn.
pub struct Storage {
    connection: Mutex<Connection>,
}

impl Storage {
    /// Open the database in `directory`, creating the folder and file when needed.
    ///
    /// Migrations finish before this returns, so the caller receives either a
    /// database at the current schema version or an error. Only the given folder
    /// is touched; no project folder or credential store is used.
    ///
    /// # Errors
    ///
    /// Returns `DataDirectory` if the folder cannot be created, `Database` if SQLite
    /// cannot open the file, `NewerSchema` if the file came from a newer build, and
    /// `MigrationFailed` if an upgrade rolled back.
    pub fn open_in_directory(directory: &Path) -> Result<Self, StorageError> {
        fs::create_dir_all(directory).map_err(StorageError::DataDirectory)?;
        Self::open_file(&directory.join(DATABASE_FILE_NAME), MIGRATIONS)
    }

    /// Return the schema version currently recorded in the database.
    ///
    /// # Errors
    ///
    /// Returns `Unavailable` after an earlier panic, or `Database` if SQLite cannot
    /// read the version.
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        let connection = self.lock()?;
        read_schema_version(&connection)
    }

    /// Open `path` and bring it up to `migrations`.
    ///
    /// Tests pass their own migrations to exercise failure paths; the app always
    /// passes `MIGRATIONS`.
    fn open_file(path: &Path, migrations: &[Migration]) -> Result<Self, StorageError> {
        let mut connection = Connection::open(path).map_err(StorageError::Database)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(StorageError::Database)?;
        // SQLite ignores foreign keys unless every connection turns enforcement on.
        // This must happen before migrations, because it cannot change mid-transaction.
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(StorageError::Database)?;
        apply_migrations(&mut connection, migrations)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Lock the shared connection for one short operation.
    ///
    /// A poisoned lock means another thread panicked while using the connection.
    /// The app then refuses further database work instead of continuing with an
    /// operation that may have stopped halfway.
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection
            .lock()
            .map_err(|_| StorageError::Unavailable)
    }
}

/// The outcome of opening private storage at startup.
///
/// The app opens even when storage is unavailable. Keeping the failure in managed
/// state lets commands report it, and lets a later backup restore rebuild the
/// database, instead of hiding the whole window. Opening never deletes, renames,
/// or rewrites a file that cannot be read.
pub enum StorageStatus {
    /// The database is open and at the current schema version.
    Ready(Storage),
    /// The database could not be opened or upgraded. The file was left unchanged.
    Unavailable(StorageError),
}

impl StorageStatus {
    /// Open the database in `directory` and keep the outcome instead of returning an error.
    pub fn open_in_directory(directory: &Path) -> Self {
        match Storage::open_in_directory(directory) {
            Ok(storage) => Self::Ready(storage),
            Err(error) => Self::Unavailable(error),
        }
    }

    /// Borrow the open database, or the reason it could not be opened.
    ///
    /// # Errors
    ///
    /// Returns the stored `StorageError` when the database is unavailable.
    pub fn storage(&self) -> Result<&Storage, &StorageError> {
        match self {
            Self::Ready(storage) => Ok(storage),
            Self::Unavailable(error) => Err(error),
        }
    }
}

/// Apply each migration newer than the database's current version, in order.
///
/// The current version comes from SQLite's `user_version` header field. A version
/// above the newest migration in this build means a newer app wrote the file.
/// Refusing to touch it avoids guessing at tables this build does not understand.
///
/// This migrator is intentionally small: the tests cover rollback, refusal of newer
/// files, and concurrent openers. Consider a migration crate only if the project
/// needs down migrations or Rust-side data rewrites.
fn apply_migrations(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<(), StorageError> {
    let supported = latest_version(migrations);
    // Check before changing anything, so a newer file is untouched even when no
    // migration is pending.
    let current = read_schema_version(connection)?;
    if current > supported {
        return Err(StorageError::NewerSchema {
            found: current,
            supported,
        });
    }
    // Each step re-reads the version under its own write lock; see `apply_one`.
    for migration in migrations {
        apply_one(connection, *migration, supported)?;
    }
    Ok(())
}

/// Run one migration unless the database already reached its version.
///
/// The version is read inside an immediate transaction. SQLite takes the write lock
/// when such a transaction starts, so a second app instance waits here (see
/// `BUSY_TIMEOUT`) and then sees the upgrade that the first instance committed.
/// Reading the version before taking the lock could let two instances run the same
/// `CREATE TABLE`. A failing `?` drops the transaction, which rolls back this step.
fn apply_one(
    connection: &mut Connection,
    migration: Migration,
    supported: u32,
) -> Result<(), StorageError> {
    let failed = |source: rusqlite::Error| StorageError::MigrationFailed {
        version: migration.version,
        source,
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(StorageError::Database)?;
    let current = read_schema_version(&transaction)?;
    if current > supported {
        return Err(StorageError::NewerSchema {
            found: current,
            supported,
        });
    }
    if current >= migration.version {
        // Another app instance, or an earlier run, already applied this version.
        return Ok(());
    }
    transaction.execute_batch(migration.sql).map_err(failed)?;
    // `user_version` is stored in the database header, so this update commits with
    // the schema change instead of becoming visible on its own.
    transaction
        .pragma_update(None, "user_version", migration.version)
        .map_err(failed)?;
    transaction.commit().map_err(failed)
}

/// Read the schema number from SQLite's `user_version` header field.
fn read_schema_version(connection: &Connection) -> Result<u32, StorageError> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .map_err(StorageError::Database)
}

/// The schema version this build migrates to, for tests in other modules that
/// should keep passing when a later migration is added.
#[cfg(test)]
pub(crate) fn latest_schema_version() -> u32 {
    latest_version(MIGRATIONS)
}

/// Return the version reached after all of `migrations` have run (0 when empty).
fn latest_version(migrations: &[Migration]) -> u32 {
    migrations.last().map_or(0, |migration| migration.version)
}

#[cfg(test)]
mod tests {
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
            crate::settings::save_locale_preference(
                &old,
                crate::settings::LocalePreference::English,
            )
            .expect("save language");
            old.lock()
                .expect("lock")
                .execute_batch(
                    "CREATE TABLE sample (value TEXT); INSERT INTO sample VALUES ('kept');",
                )
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
        let storage =
            Storage::open_file(&path, &migrations).expect("second open must skip version 1");

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

        let storage =
            Storage::open_file(&path, &[first, second, third]).expect("upgrade to version 3");

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
}
