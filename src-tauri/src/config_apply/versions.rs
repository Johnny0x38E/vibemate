//! Independent, complete written-configuration copies, separate from recovery.
//!
//! Each apply owns one private copy of every planned file, including unchanged
//! files. Version selection never borrows another version's or journal's bytes.
//! Legacy journals are materialized under the configuration lock before writes
//! or deletion can consume their old copies. Reads do not migrate or write.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    BackupStore, OperationRecord, PlannedFile, StepState, WriteError, WriteTarget, file_name,
    operations::read_operation, path_text, snapshots::read_snapshot,
};
use crate::{config_plan::read_target_bytes, storage::Storage};

/// Private metadata for one version-owned complete file.
pub(super) struct VersionFile {
    pub ordinal: u32,
    pub target_path: String,
    pub digest: String,
    pub copy_id: String,
    pub copy_path: String,
}

/// Record copy ownership before storing bytes and before any target is changed.
/// The caller holds the configuration lock. A failure leaves a deletable,
/// prepared version; the operation must not continue to modify target files.
pub(super) fn prepare(
    storage: &Storage,
    id: &str,
    files: &[PlannedFile],
    backups: &BackupStore,
) -> Result<(), WriteError> {
    {
        let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
        let transaction = connection
            .transaction()
            .map_err(|_| WriteError::LogWriteFailed)?;
        transaction
            .execute(
                "INSERT INTO config_version(id, state) VALUES (?1, 'prepared')
             ON CONFLICT(id) DO UPDATE SET state = 'migrating' WHERE state = 'legacy'",
                [id],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
        for (index, file) in files.iter().enumerate() {
            let ordinal = u32::try_from(index).map_err(|_| WriteError::InvalidRequest)?;
            let copy_id = format!("version-{id}-{ordinal}");
            let copy_path = backups
                .root()
                .join(&copy_id)
                .join(file_name(&file.target.path));
            transaction
                .execute(
                    "INSERT INTO config_version_file
                 (version_id, ordinal, target_path, digest, copy_id, copy_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id,
                        ordinal,
                        path_text(&file.target.path)?,
                        crate::config_plan::digest_hex(&file.content),
                        copy_id,
                        path_text(&copy_path)?
                    ],
                )
                .map_err(|_| WriteError::LogWriteFailed)?;
        }
        transaction
            .commit()
            .map_err(|_| WriteError::LogWriteFailed)?;
    }
    for (index, file) in files.iter().enumerate() {
        backups.store(
            &format!("version-{id}-{index}"),
            &file_name(&file.target.path),
            &file.content,
        )?;
    }
    Ok(())
}

/// Publish only after every target and the apply journal reported success.
pub(super) fn publish(storage: &Storage, id: &str) -> Result<(), WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let changed = connection
        .execute(
            "UPDATE config_version SET state = 'available' WHERE id = ?1 AND state IN ('prepared','migrating')",
            [id],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if changed != 1 {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(())
}

/// Read independently owned file metadata, never bytes from other records.
pub(super) fn files(connection: &Connection, id: &str) -> Result<Vec<VersionFile>, WriteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, target_path, digest, copy_id, copy_path
         FROM config_version_file WHERE version_id = ?1 ORDER BY ordinal",
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = statement
        .query_map([id], |row| {
            Ok(VersionFile {
                ordinal: row.get(0)?,
                target_path: row.get(1)?,
                digest: row.get(2)?,
                copy_id: row.get(3)?,
                copy_path: row.get(4)?,
            })
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    let files = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| WriteError::LogReadFailed)?;
    for (index, file) in files.iter().enumerate() {
        if index >= 16
            || usize::try_from(file.ordinal).ok() != Some(index)
            || file.copy_id != format!("version-{id}-{}", file.ordinal)
            || path_text(Path::new(&file.target_path)).is_err()
            || path_text(Path::new(&file.copy_path)).is_err()
            || file.digest.len() != 64
            || !file
                .digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(WriteError::InvalidStoredOperation);
        }
    }
    Ok(files)
}

/// Copy legacy result bytes into independent version storage without changing
/// Agent files. Intended for startup; errors keep legacy records intact and
/// cause later mutations to refuse until migration succeeds. Missing historical
/// bytes become explicitly unavailable, never reconstructed from a model name.
///
/// # Errors
/// Returns safe lock, journal and private-copy errors. Existing journal rows and
/// copies are retained, including when conversion is interrupted.
pub fn materialize_configuration_versions(
    storage: &Storage,
    backups: &BackupStore,
) -> Result<(), WriteError> {
    let _lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    materialize_locked(storage, backups)
}

/// The caller already owns the shared cross-process configuration lock.
pub(super) fn materialize_locked(
    storage: &Storage,
    backups: &BackupStore,
) -> Result<(), WriteError> {
    let ids = {
        let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
        let mut statement = connection.prepare(
            "SELECT id, state FROM config_version WHERE state IN ('legacy','migrating','prepared')",
        ).map_err(|_| WriteError::LogReadFailed)?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| WriteError::LogReadFailed)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| WriteError::LogReadFailed)?
    };
    for (id, state) in ids {
        let source = {
            let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
            read_operation(&connection, &id)?.ok_or(WriteError::InvalidStoredOperation)?
        };
        if state == "prepared" {
            // Recover publication after a crash, but never infer success from
            // planned bytes when the operation itself did not finish.
            if source.state.is_unfinished()
                || source
                    .steps
                    .iter()
                    .any(|step| step.state != StepState::Applied)
            {
                continue;
            }
            let own_files = {
                let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
                files(&connection, &id)?
            };
            let complete = own_files.len() == source.steps.len()
                && own_files.iter().all(|file| {
                    read_snapshot(
                        backups,
                        &file.copy_id,
                        &file.target_path,
                        &file.copy_path,
                        &file.digest,
                    )
                    .is_ok()
                });
            if complete {
                publish(storage, &id)?;
            } else {
                mark_unavailable(storage, &id)?;
            }
            continue;
        }
        let Some(planned) = legacy_bytes(storage, &source, backups)? else {
            mark_unavailable(storage, &id)?;
            continue;
        };
        if state == "legacy" {
            prepare(storage, &id, &planned, backups)?;
        } else {
            // Resume one-time conversion without overwriting linked or corrupt
            // present copies. Legacy journals still exist until conversion ends.
            for (index, file) in planned.iter().enumerate() {
                let copy_id = format!("version-{id}-{index}");
                let path = backups
                    .root()
                    .join(&copy_id)
                    .join(file_name(&file.target.path));
                if !path
                    .try_exists()
                    .map_err(|_| WriteError::BackupUnavailable)?
                {
                    backups.store(&copy_id, &file_name(&file.target.path), &file.content)?;
                }
                read_snapshot(
                    backups,
                    &copy_id,
                    &path_text(&file.target.path)?,
                    &path_text(&path)?,
                    &crate::config_plan::digest_hex(&file.content),
                )?;
            }
        }
        publish(storage, &id)?;
    }
    Ok(())
}

fn mark_unavailable(storage: &Storage, id: &str) -> Result<(), WriteError> {
    storage
        .lock()
        .map_err(|_| WriteError::StorageUnavailable)?
        .execute(
            "UPDATE config_version SET state = 'unavailable' WHERE id = ?1",
            [id],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(())
}

/// Only the one-time conversion may read old journal copies. The resulting
/// files are copied, not linked; deleting any later version cannot affect them.
fn legacy_bytes(
    storage: &Storage,
    source: &OperationRecord,
    backups: &BackupStore,
) -> Result<Option<Vec<PlannedFile>>, WriteError> {
    let mut files = Vec::new();
    for step in &source.steps {
        let Some(digest) = step
            .applied_digest
            .as_deref()
            .filter(|_| step.state == StepState::Applied)
        else {
            return Ok(None);
        };
        let own_id = format!("version-{}-{}", source.id, step.ordinal);
        let own_path = backups
            .root()
            .join(&own_id)
            .join(file_name(Path::new(&step.target_path)));
        let own = read_snapshot(
            backups,
            &own_id,
            &step.target_path,
            &path_text(&own_path)?,
            digest,
        )
        .ok();
        let live = own.or_else(|| {
            read_target_bytes(Path::new(&step.target_path))
                .ok()
                .filter(|(target, _)| {
                    !target.symlink && target.content_digest.as_deref() == Some(digest)
                })
                .map(|(_, bytes)| bytes)
        });
        let bytes = match live {
            Some(bytes) => Some(bytes),
            None => {
                let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
                let mut statement = connection
                    .prepare(
                        "SELECT id, backup_path FROM config_file_revision
                     WHERE target_path = ?1 AND previous_digest = ?2 AND backup_path IS NOT NULL
                     ORDER BY created_at DESC, id DESC",
                    )
                    .map_err(|_| WriteError::LogReadFailed)?;
                let rows = statement
                    .query_map(params![step.target_path, digest], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .map_err(|_| WriteError::LogReadFailed)?;
                let mut found = None;
                for row in rows {
                    let (copy_id, path) = row.map_err(|_| WriteError::LogReadFailed)?;
                    if let Ok(bytes) =
                        read_snapshot(backups, &copy_id, &step.target_path, &path, digest)
                    {
                        found = Some(bytes);
                        break;
                    }
                }
                found
            }
        };
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        files.push(PlannedFile {
            target: WriteTarget {
                path: step.target_path.clone().into(),
                expected_digest: None,
            },
            content: bytes,
        });
    }
    Ok((!files.is_empty()).then_some(files))
}

/// Whether an entry was explicitly deleted from the configuration-version list.
pub(super) fn is_deleted(connection: &Connection, id: &str) -> Result<bool, WriteError> {
    connection
        .query_row(
            "SELECT state = 'deleted' FROM config_version WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map(|value| value.unwrap_or(true))
        .map_err(|_| WriteError::LogReadFailed)
}
