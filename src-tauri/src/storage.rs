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
mod tests;
