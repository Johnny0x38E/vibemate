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
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: "",
}];

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
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
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
