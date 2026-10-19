//! Explicit cleanup of private copies that never acquired a journal reference.
//!
//! No history row is removed. The remaining directories are the durable retry
//! list after a failure or crash; there is no separate queue that could hide a
//! still-present credential-bearing copy. Referenced snapshots never expire.

use std::{collections::BTreeSet, fs, io::ErrorKind, path::Path};

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

use super::{Backup, BackupStore, WriteError, path_text, read_target, sync_directory};
use crate::storage::Storage;

/// One unreferenced writer directory, with its exact reviewed file identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnreferencedBackup {
    /// Three-part write identifier; the directory is `<backup-root>/<id>`.
    pub id: String,
    /// The sole regular file, or `None` for an empty interrupted backup directory.
    pub file: Option<Backup>,
}

/// Private backup inventory, without any stored configuration bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInventory {
    /// Copies that can be explicitly reviewed for irreversible deletion.
    pub unreferenced: Vec<UnreferencedBackup>,
    /// Directory paths protected by any old-step or per-mutation reference.
    pub retained: Vec<String>,
    /// Unrecognized, linked, unreadable or oversized entries left untouched.
    pub blocked: Vec<String>,
}

/// Result for one explicitly approved unreferenced directory.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCleanupOutcome {
    /// Write identifier submitted for cleanup.
    pub id: String,
    /// `None` means the file and directory are gone; otherwise rescan and retry.
    pub error_code: Option<WriteError>,
}

/// Inspect all private copies and both generations of journal references.
///
/// Takes the same cross-instance lock as recorded configuration writes, then a
/// SQLite write transaction, so an active pre-log backup is never an orphan.
/// Does not create the backup root, delete files, or update history. Opening the
/// shared configuration lock can create its permanent companion file.
/// The root and its ancestors must come from trusted application storage, not
/// frontend input. Root/entry symlinks are refused; this is not a sandbox against
/// a hostile process swapping filesystem ancestors during an operation.
///
/// # Errors
/// Returns storage/log errors if references cannot be trusted, `InvalidStoredOperation`
/// for malformed references, and `BackupUnavailable` for an unsafe/unreadable
/// root or directory listing. No error contains configuration bytes.
pub fn inspect_backup_copies(
    storage: &Storage,
    backups: &BackupStore,
) -> Result<BackupInventory, WriteError> {
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| WriteError::LogReadFailed)?;
    inventory(&transaction, backups)
}

/// Delete only the explicitly reviewed, still-unreferenced private copies.
///
/// Validates the entire selection before deleting anything. New references or
/// changed bytes/layout make approval stale. Both locks remain held through the
/// deletes; SQLite reference writers cannot race the filesystem work. Only a
/// single reviewed regular file and its empty directory are removed, never an
/// arbitrary tree. No configuration, history row, credential-store entry or
/// ownership binding is changed. Ordinary deletion is not secure erasure.
///
/// A missing directory is already complete, making retries idempotent. On a
/// per-directory I/O failure, returns `BackupUnavailable` in that outcome and
/// continues with the other approved copies. Remaining directories survive a
/// restart and can be inspected again (including empty directories left after
/// file removal). Never automatically call this merely to display inventory.
///
/// # Errors
/// Returns `InvalidRequest` for an empty, duplicate or malformed selection,
/// `PlanStale` if any existing candidate differs or gained a reference, and the
/// storage/log/root errors of [`inspect_backup_copies`]. These errors occur
/// before deletion; per-directory failures are returned as outcomes instead.
pub fn cleanup_backup_copies(
    storage: &Storage,
    backups: &BackupStore,
    reviewed: &[UnreferencedBackup],
) -> Result<Vec<BackupCleanupOutcome>, WriteError> {
    validate_selection(backups, reviewed)?;
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| WriteError::LogReadFailed)?;
    let current = inventory(&transaction, backups)?;
    let references = References::read(&transaction)?;
    for candidate in reviewed {
        let directory = backups.root().join(&candidate.id);
        if references.contains(&directory, &candidate.id) {
            return Err(WriteError::PlanStale);
        }
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Ok(_) if current.unreferenced.contains(candidate) => {}
            _ => return Err(WriteError::PlanStale),
        }
    }
    Ok(reviewed
        .iter()
        .map(|candidate| BackupCleanupOutcome {
            id: candidate.id.clone(),
            error_code: remove_copy(backups, candidate).err(),
        })
        .collect())
}

/// Keep identifiers as well as paths: a malformed/moved path must not make a
/// directory with a recorded identity eligible for deletion.
struct References {
    ids: BTreeSet<String>,
    paths: BTreeSet<String>,
}

impl References {
    fn read(connection: &Connection) -> Result<Self, WriteError> {
        let mut statement = connection
            .prepare(
                "SELECT backup_id, backup_path, 1 FROM config_operation_step
                 WHERE backup_id IS NOT NULL OR backup_path IS NOT NULL
                 UNION ALL SELECT id, backup_path, 0 FROM config_file_revision",
            )
            .map_err(|_| WriteError::LogReadFailed)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(|_| WriteError::LogReadFailed)?;
        let mut references = Self {
            ids: BTreeSet::new(),
            paths: BTreeSet::new(),
        };
        for row in rows {
            let (id, path, requires_path) = row.map_err(|_| WriteError::LogReadFailed)?;
            if requires_path && path.is_none() {
                return Err(WriteError::InvalidStoredOperation);
            }
            let id = id.ok_or(WriteError::InvalidStoredOperation)?;
            if id.is_empty()
                || id.len() > 128
                || Path::new(&id).components().count() != 1
                || id.contains(['/', '\\'])
                || id == "."
                || id == ".."
                || id.chars().any(char::is_control)
            {
                return Err(WriteError::InvalidStoredOperation);
            }
            references.ids.insert(id);
            if let Some(path) = path {
                if !is_plain_absolute_path(Path::new(&path)) {
                    return Err(WriteError::InvalidStoredOperation);
                }
                references.paths.insert(path);
            }
        }
        Ok(references)
    }

    fn contains(&self, directory: &Path, id: &str) -> bool {
        self.ids.contains(id)
            || self
                .paths
                .iter()
                .any(|path| Path::new(path).starts_with(directory))
    }
}

fn inventory(
    connection: &Connection,
    backups: &BackupStore,
) -> Result<BackupInventory, WriteError> {
    let references = References::read(connection)?;
    let mut result = BackupInventory {
        unreferenced: vec![],
        retained: vec![],
        blocked: vec![],
    };
    if !validate_root(backups.root())? {
        return Ok(result);
    }
    let entries = fs::read_dir(backups.root()).map_err(|_| WriteError::BackupUnavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| WriteError::BackupUnavailable)?;
        let path = entry.path();
        let text = path_text(&path).map_err(|_| WriteError::BackupUnavailable)?;
        let id = entry.file_name();
        let Some(id) = id.to_str() else {
            return Err(WriteError::BackupUnavailable);
        };
        if references.contains(&path, id) {
            result.retained.push(text);
        } else if let Ok(candidate) = inspect_copy(backups, id) {
            result.unreferenced.push(candidate);
        } else {
            result.blocked.push(text);
        }
    }
    result.unreferenced.sort_by(|a, b| a.id.cmp(&b.id));
    result.retained.sort();
    result.blocked.sort();
    Ok(result)
}

/// Missing stores need no creation just to display a zero inventory.
fn validate_root(root: &Path) -> Result<bool, WriteError> {
    if !is_plain_absolute_path(root) {
        return Err(WriteError::BackupUnavailable);
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        _ => Err(WriteError::BackupUnavailable),
    }
}

fn is_plain_absolute_path(path: &Path) -> bool {
    path_text(path).is_ok()
        && path.components().all(|part| {
            !matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
}

/// The writer uses milliseconds, process ID and a monotonic process counter.
/// Unknown directories are not assumed disposable just because no row names them.
fn is_write_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('-').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            part.parse::<u64>()
                .is_ok_and(|number| number.to_string() == *part)
        })
        && parts[1].parse::<u32>().is_ok()
}

fn inspect_copy(backups: &BackupStore, id: &str) -> Result<UnreferencedBackup, WriteError> {
    if !is_write_id(id) {
        return Err(WriteError::BackupUnavailable);
    }
    let directory = backups.root().join(id);
    let metadata = fs::symlink_metadata(&directory).map_err(|_| WriteError::BackupUnavailable)?;
    if !metadata.is_dir() || metadata.is_symlink() {
        return Err(WriteError::BackupUnavailable);
    }
    let mut entries = fs::read_dir(&directory).map_err(|_| WriteError::BackupUnavailable)?;
    let file = if let Some(entry) = entries.next() {
        let entry = entry.map_err(|_| WriteError::BackupUnavailable)?;
        if entries.next().is_some() {
            return Err(WriteError::BackupUnavailable);
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| WriteError::BackupUnavailable)?;
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(WriteError::BackupUnavailable);
        }
        let target = read_target(&path).map_err(|_| WriteError::BackupUnavailable)?;
        if target.symlink || !target.exists {
            return Err(WriteError::BackupUnavailable);
        }
        Some(Backup {
            id: id.to_owned(),
            path: path_text(&path).map_err(|_| WriteError::BackupUnavailable)?,
            content_digest: target.content_digest.ok_or(WriteError::BackupUnavailable)?,
        })
    } else {
        None
    };
    Ok(UnreferencedBackup {
        id: id.to_owned(),
        file,
    })
}

fn validate_selection(
    backups: &BackupStore,
    reviewed: &[UnreferencedBackup],
) -> Result<(), WriteError> {
    let mut ids = BTreeSet::new();
    if reviewed.is_empty() {
        return Err(WriteError::InvalidRequest);
    }
    for candidate in reviewed {
        if !is_write_id(&candidate.id) || !ids.insert(&candidate.id) {
            return Err(WriteError::InvalidRequest);
        }
        if let Some(file) = &candidate.file {
            let path = Path::new(&file.path);
            if file.id != candidate.id
                || !super::operations::is_digest(&file.content_digest)
                || !is_plain_absolute_path(path)
                || path.parent() != Some(backups.root().join(&candidate.id).as_path())
            {
                return Err(WriteError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn remove_copy(backups: &BackupStore, candidate: &UnreferencedBackup) -> Result<(), WriteError> {
    let directory = backups.root().join(&candidate.id);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(WriteError::BackupUnavailable),
        Ok(_) => {}
    }
    // Recheck each entry immediately before deletion as well as during preflight.
    // The shared lock excludes vibemate writers, not unrelated external processes.
    if inspect_copy(backups, &candidate.id)? != *candidate {
        return Err(WriteError::BackupUnavailable);
    }
    if let Some(file) = &candidate.file {
        fs::remove_file(&file.path).map_err(|_| WriteError::BackupUnavailable)?;
        sync_directory(&directory).map_err(|_| WriteError::BackupUnavailable)?;
    }
    fs::remove_dir(&directory).map_err(|_| WriteError::BackupUnavailable)?;
    sync_directory(backups.root()).map_err(|_| WriteError::BackupUnavailable)
}
