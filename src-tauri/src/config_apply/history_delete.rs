//! User-requested deletion of one history entry and its private backups (H3).
//!
//! Database rows and private backup files are not one atomic transaction. A
//! durable delete journal (`prepared` → file work → journal removal →
//! `completed`, or `failed` with leftovers) makes retries idempotent without
//! first hiding the history row while secret-bearing copies remain unmanaged.
//! Shared backup references are never removed. Agent configuration files,
//! Provider/central credentials and alias ownership bindings are untouched.
//! Ordinary file removal is not secure erasure.

use std::{
    collections::BTreeSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use serde_json::json;

use super::{
    BackupStore, HistoryScope, OperationRecord, WriteError,
    history::{MAX_SCOPE_FILES, valid_entry_key, valid_revision_id},
    inspection::has_interrupted_revisions,
    operations::read_operation,
    path_text, sync_directory,
};
use crate::{shared, storage::Storage};

/// Durable progress of one history-entry deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDeleteState {
    /// Journal rows are gone and every exclusive private copy is gone.
    Completed,
    /// At least one exclusive copy could not be removed; retry the same entry.
    Failed,
}

/// One exclusive private copy that survived a delete attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDeleteLeftover {
    /// Private-store directory identity under the backup root.
    pub backup_id: String,
    /// Absolute directory path that still exists after the attempt.
    pub path: String,
    /// Stable reason the copy could not be removed.
    pub error_code: String,
}

/// Result of deleting one history entry; never carries configuration bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDeleteOutcome {
    /// Entry the caller asked to remove.
    pub entry_key: String,
    /// Whether the durable delete finished or still has leftovers.
    pub state: HistoryDeleteState,
    /// Exclusive copies that remain; empty when `completed`.
    pub leftovers: Vec<HistoryDeleteLeftover>,
}

#[derive(Clone)]
struct BackupTarget {
    id: String,
    dir: PathBuf,
}

struct OpenDelete {
    id: String,
    entry_key: String,
    operation_id: String,
    state: String,
    targets: Vec<BackupTarget>,
}

/// Delete one history entry (`operation:<id>` or `batch:<id>`) and its private copies.
///
/// The scope is built by trusted Agent code from the active configuration root;
/// frontend paths are refused. Unfinished writes, interrupted revisions and
/// dependent restore/compensation batches refuse with stable codes. Shared
/// backup references stay on disk. Retrying the same entry after `failed`
/// continues from the durable leftovers list.
///
/// # Errors
/// Returns `InvalidRequest` for a malformed or missing entry, `InvalidStoredOperation`
/// for an out-of-scope or malformed journal, `HistoryActive` / `HistoryUnresolved` /
/// `HistoryReferenced` for protected rows, and storage/log errors when the delete
/// journal cannot be recorded honestly.
pub fn delete_history_entry(
    storage: &Storage,
    scope: &HistoryScope,
    entry_key: &str,
    backups: &BackupStore,
) -> Result<HistoryDeleteOutcome, WriteError> {
    if !valid_entry_key(entry_key) {
        return Err(WriteError::InvalidRequest);
    }
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;

    let open = {
        let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WriteError::LogWriteFailed)?;
        let prepared = prepare_delete(&transaction, scope, entry_key, backups)?;
        transaction
            .commit()
            .map_err(|_| WriteError::LogWriteFailed)?;
        prepared
    };

    // File work happens after the durable prepared/failed row exists so a crash
    // leaves a retryable claim rather than an orphaned secret copy with no row.
    let leftovers = remove_exclusive_backups(backups, &open.targets);

    let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| WriteError::LogWriteFailed)?;
    let outcome = finish_delete(&transaction, scope, &open, leftovers)?;
    transaction
        .commit()
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(outcome)
}

fn prepare_delete(
    connection: &Connection,
    scope: &HistoryScope,
    entry_key: &str,
    backups: &BackupStore,
) -> Result<OpenDelete, WriteError> {
    if let Some(existing) = read_open_delete(connection, entry_key)? {
        if existing.operation_id.is_empty() {
            return Err(WriteError::InvalidStoredOperation);
        }
        // Resume only claims that still match this Agent scope.
        let source = read_operation(connection, &existing.operation_id)?
            .ok_or(WriteError::InvalidStoredOperation)?;
        check_scope(scope, &source)?;
        return Ok(existing);
    }

    // Idempotent success: the entry is already gone and nothing is open.
    if entry_absent(connection, entry_key)? {
        let id = new_delete_id(connection)?;
        let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
        connection
            .execute(
                "INSERT INTO config_history_delete
                 (id, entry_key, operation_id, scope_kind, state, leftovers, error_code,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'completed', NULL, NULL, ?5, ?5)",
                params![
                    id,
                    entry_key,
                    "00000000000000000000000000000000",
                    scope.operation_kind().as_str(),
                    now
                ],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
        return Ok(OpenDelete {
            id,
            entry_key: entry_key.to_owned(),
            operation_id: "00000000000000000000000000000000".to_owned(),
            state: "completed".to_owned(),
            targets: Vec::new(),
        });
    }

    let (source, batch) = resolve_entry(connection, scope, entry_key)?;
    refuse_protected(connection, &source, batch)?;
    let targets = exclusive_backups(connection, backups, &source, batch)?;
    let id = new_delete_id(connection)?;
    let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
    connection
        .execute(
            "INSERT INTO config_history_delete
             (id, entry_key, operation_id, scope_kind, state, leftovers, error_code,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'prepared', NULL, NULL, ?5, ?5)",
            params![
                id,
                entry_key,
                source.id,
                scope.operation_kind().as_str(),
                now
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    for target in &targets {
        let dir = path_text(&target.dir)?;
        connection
            .execute(
                "INSERT INTO config_history_delete_backup (delete_id, backup_id, backup_dir)
                 VALUES (?1, ?2, ?3)",
                params![id, target.id, dir],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
    }
    Ok(OpenDelete {
        id,
        entry_key: entry_key.to_owned(),
        operation_id: source.id,
        state: "prepared".to_owned(),
        targets,
    })
}

fn finish_delete(
    connection: &Connection,
    scope: &HistoryScope,
    open: &OpenDelete,
    leftovers: Vec<HistoryDeleteLeftover>,
) -> Result<HistoryDeleteOutcome, WriteError> {
    let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
    if open.state == "completed" && leftovers.is_empty() {
        return Ok(HistoryDeleteOutcome {
            entry_key: open.entry_key.clone(),
            state: HistoryDeleteState::Completed,
            leftovers: Vec::new(),
        });
    }
    if !leftovers.is_empty() {
        let payload = json!(
            leftovers
                .iter()
                .map(|item| json!({
                    "backupId": item.backup_id,
                    "path": item.path,
                    "errorCode": item.error_code,
                }))
                .collect::<Vec<_>>()
        )
        .to_string();
        connection
            .execute(
                "UPDATE config_history_delete SET state = 'failed', leftovers = ?2,
                 error_code = ?3, updated_at = max(created_at, ?4) WHERE id = ?1",
                params![open.id, payload, WriteError::BackupUnavailable.code(), now],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
        return Ok(HistoryDeleteOutcome {
            entry_key: open.entry_key.clone(),
            state: HistoryDeleteState::Failed,
            leftovers,
        });
    }

    // Files are clear. Remove journal rows only when the entry still exists;
    // a prior successful attempt may have already cleared them.
    if !entry_absent(connection, &open.entry_key)? {
        let source = read_operation(connection, &open.operation_id)?
            .ok_or(WriteError::InvalidStoredOperation)?;
        check_scope(scope, &source)?;
        let batch = open.entry_key.strip_prefix("batch:");
        // Re-check protections under the same write transaction.
        refuse_protected(connection, &source, batch)?;
        remove_journal_rows(connection, &source, batch)?;
    }

    connection
        .execute(
            "UPDATE config_history_delete SET state = 'completed', leftovers = NULL,
             error_code = NULL, updated_at = max(created_at, ?2) WHERE id = ?1",
            params![open.id, now],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    // Drop the target list once the claim is complete; leftovers already empty.
    connection
        .execute(
            "DELETE FROM config_history_delete_backup WHERE delete_id = ?1",
            params![open.id],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(HistoryDeleteOutcome {
        entry_key: open.entry_key.clone(),
        state: HistoryDeleteState::Completed,
        leftovers: Vec::new(),
    })
}

fn refuse_protected(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<(), WriteError> {
    if source.state.is_unfinished() {
        return Err(WriteError::HistoryActive);
    }
    if has_interrupted_revisions(connection, &source.id)? {
        return Err(WriteError::HistoryUnresolved);
    }
    if batch.is_none() {
        let dependent: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM config_file_revision
                    WHERE operation_id = ?1 AND intent != 'apply'
                )",
                [&source.id],
                |row| row.get(0),
            )
            .map_err(|_| WriteError::LogReadFailed)?;
        if dependent {
            return Err(WriteError::HistoryReferenced);
        }
    }
    Ok(())
}

fn resolve_entry<'a>(
    connection: &Connection,
    scope: &HistoryScope,
    entry_key: &'a str,
) -> Result<(OperationRecord, Option<&'a str>), WriteError> {
    if let Some(operation_id) = entry_key.strip_prefix("operation:") {
        let source = read_operation(connection, operation_id)?.ok_or(WriteError::InvalidRequest)?;
        check_scope(scope, &source)?;
        return Ok((source, None));
    }
    let batch = entry_key
        .strip_prefix("batch:")
        .filter(|value| valid_revision_id(value))
        .ok_or(WriteError::InvalidRequest)?;
    let operation_id: String = connection
        .query_row(
            "SELECT operation_id FROM config_file_revision
             WHERE batch_id = ?1 ORDER BY rowid LIMIT 1",
            [batch],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?
        .ok_or(WriteError::InvalidRequest)?;
    let source = read_operation(connection, &operation_id)?.ok_or(WriteError::InvalidRequest)?;
    check_scope(scope, &source)?;
    // Confirm the batch still has rows (not a stale key).
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM config_file_revision
                WHERE batch_id = ?1 AND operation_id = ?2 AND intent != 'apply'
            )",
            params![batch, source.id],
            |row| row.get(0),
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    if !exists {
        return Err(WriteError::InvalidRequest);
    }
    Ok((source, Some(batch)))
}

fn entry_absent(connection: &Connection, entry_key: &str) -> Result<bool, WriteError> {
    if let Some(operation_id) = entry_key.strip_prefix("operation:") {
        let present = read_operation(connection, operation_id)?.is_some();
        return Ok(!present);
    }
    let batch = entry_key
        .strip_prefix("batch:")
        .filter(|value| valid_revision_id(value))
        .ok_or(WriteError::InvalidRequest)?;
    let present: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM config_file_revision
                WHERE batch_id = ?1 AND intent != 'apply'
            )",
            [batch],
            |row| row.get(0),
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    Ok(!present)
}

fn exclusive_backups(
    connection: &Connection,
    backups: &BackupStore,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<Vec<BackupTarget>, WriteError> {
    let candidates = backup_candidates(connection, source, batch)?;
    let mut exclusive = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.id.clone()) {
            continue;
        }
        if references_remain(connection, source, batch, &candidate)? {
            continue;
        }
        let dir = backups.root().join(&candidate.id);
        // Recorded path must live under this exclusive directory when present.
        if let Some(path) = &candidate.path {
            let recorded = Path::new(path);
            if !recorded.starts_with(&dir) {
                return Err(WriteError::InvalidStoredOperation);
            }
        }
        exclusive.push(BackupTarget {
            id: candidate.id,
            dir,
        });
    }
    Ok(exclusive)
}

struct CandidateBackup {
    id: String,
    path: Option<String>,
}

fn backup_candidates(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<Vec<CandidateBackup>, WriteError> {
    let mut candidates = Vec::new();
    if let Some(batch_id) = batch {
        let mut query = connection
            .prepare(
                "SELECT id, backup_path FROM config_file_revision
                 WHERE batch_id = ?1 AND operation_id = ?2 AND intent != 'apply'",
            )
            .map_err(|_| WriteError::LogReadFailed)?;
        let rows = query
            .query_map(params![batch_id, source.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|_| WriteError::LogReadFailed)?;
        for row in rows {
            let (id, path) = row.map_err(|_| WriteError::LogReadFailed)?;
            if !valid_revision_id(&id) {
                return Err(WriteError::InvalidStoredOperation);
            }
            candidates.push(CandidateBackup { id, path });
        }
        return Ok(candidates);
    }

    // Operation entry: apply revisions and legacy step backups.
    let mut query = connection
        .prepare(
            "SELECT id, backup_path FROM config_file_revision
             WHERE operation_id = ?1 AND intent = 'apply'",
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map([&source.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    for row in rows {
        let (id, path) = row.map_err(|_| WriteError::LogReadFailed)?;
        if !valid_revision_id(&id) {
            return Err(WriteError::InvalidStoredOperation);
        }
        candidates.push(CandidateBackup { id, path });
    }
    for step in &source.steps {
        if let Some(backup) = &step.backup {
            candidates.push(CandidateBackup {
                id: backup.id.clone(),
                path: Some(backup.path.clone()),
            });
        }
    }
    Ok(candidates)
}

fn references_remain(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
    candidate: &CandidateBackup,
) -> Result<bool, WriteError> {
    // Load remaining journal references and compare in Rust so path wildcards
    // cannot make an unrelated copy look shared.
    let mut query = connection
        .prepare(
            "SELECT backup_id, backup_path, 1, operation_id, NULL, NULL
             FROM config_operation_step
             WHERE backup_id IS NOT NULL OR backup_path IS NOT NULL
             UNION ALL
             SELECT id, backup_path, 0, operation_id, batch_id, intent
             FROM config_file_revision",
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    let candidate_dir = candidate
        .path
        .as_deref()
        .and_then(|path| Path::new(path).parent().map(Path::to_path_buf));
    for row in rows {
        let (id, path, is_step, operation_id, batch_id, intent) =
            row.map_err(|_| WriteError::LogReadFailed)?;
        let excluded = match batch {
            Some(batch_id_target) => {
                !is_step
                    && operation_id == source.id
                    && batch_id.as_deref() == Some(batch_id_target)
                    && intent.as_deref().is_some_and(|value| value != "apply")
            }
            None => {
                if is_step {
                    operation_id == source.id
                } else {
                    operation_id == source.id && intent.as_deref() == Some("apply")
                }
            }
        };
        if excluded {
            continue;
        }
        let Some(id) = id else {
            return Err(WriteError::InvalidStoredOperation);
        };
        if id == candidate.id {
            return Ok(true);
        }
        if let (Some(path), Some(dir)) = (path.as_deref(), candidate_dir.as_ref())
            && Path::new(path).starts_with(dir)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn remove_journal_rows(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<(), WriteError> {
    match batch {
        Some(batch_id) => {
            connection
                .execute(
                    "DELETE FROM config_revision_resolution WHERE revision_id IN (
                        SELECT id FROM config_file_revision
                        WHERE batch_id = ?1 AND operation_id = ?2 AND intent != 'apply'
                    )",
                    params![batch_id, source.id],
                )
                .map_err(|_| WriteError::LogWriteFailed)?;
            let removed = connection
                .execute(
                    "DELETE FROM config_file_revision
                     WHERE batch_id = ?1 AND operation_id = ?2 AND intent != 'apply'",
                    params![batch_id, source.id],
                )
                .map_err(|_| WriteError::LogWriteFailed)?;
            if removed == 0 {
                return Err(WriteError::InvalidRequest);
            }
        }
        None => {
            connection
                .execute(
                    "DELETE FROM config_revision_resolution WHERE revision_id IN (
                        SELECT id FROM config_file_revision WHERE operation_id = ?1
                    )",
                    [&source.id],
                )
                .map_err(|_| WriteError::LogWriteFailed)?;
            connection
                .execute(
                    "DELETE FROM config_file_revision WHERE operation_id = ?1",
                    [&source.id],
                )
                .map_err(|_| WriteError::LogWriteFailed)?;
            let removed = connection
                .execute("DELETE FROM config_operation WHERE id = ?1", [&source.id])
                .map_err(|_| WriteError::LogWriteFailed)?;
            if removed != 1 {
                return Err(WriteError::InvalidStoredOperation);
            }
        }
    }
    Ok(())
}

fn remove_exclusive_backups(
    backups: &BackupStore,
    targets: &[BackupTarget],
) -> Vec<HistoryDeleteLeftover> {
    let mut leftovers = Vec::new();
    for target in targets {
        if let Err(error) = remove_backup_directory(backups, target) {
            let path = path_text(&target.dir).unwrap_or_else(|_| target.dir.display().to_string());
            leftovers.push(HistoryDeleteLeftover {
                backup_id: target.id.clone(),
                path,
                error_code: error.code().to_owned(),
            });
        }
    }
    leftovers
}

fn remove_backup_directory(backups: &BackupStore, target: &BackupTarget) -> Result<(), WriteError> {
    if target.dir.parent() != Some(backups.root()) {
        return Err(WriteError::BackupUnavailable);
    }
    match fs::symlink_metadata(&target.dir) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(WriteError::BackupUnavailable),
        Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => {}
        Ok(_) => return Err(WriteError::BackupUnavailable),
    }
    // Remove each regular file, then the empty directory. Never follow links.
    let entries = fs::read_dir(&target.dir).map_err(|_| WriteError::BackupUnavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| WriteError::BackupUnavailable)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| WriteError::BackupUnavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(WriteError::BackupUnavailable);
        }
        fs::remove_file(&path).map_err(|_| WriteError::BackupUnavailable)?;
    }
    sync_directory(&target.dir).map_err(|_| WriteError::BackupUnavailable)?;
    fs::remove_dir(&target.dir).map_err(|_| WriteError::BackupUnavailable)?;
    sync_directory(backups.root()).map_err(|_| WriteError::BackupUnavailable)
}

fn read_open_delete(
    connection: &Connection,
    entry_key: &str,
) -> Result<Option<OpenDelete>, WriteError> {
    let row = connection
        .query_row(
            "SELECT id, entry_key, operation_id, state FROM config_history_delete
             WHERE entry_key = ?1 AND state IN ('prepared', 'failed')",
            [entry_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?;
    let Some((id, entry_key, operation_id, state)) = row else {
        return Ok(None);
    };
    let mut query = connection
        .prepare(
            "SELECT backup_id, backup_dir FROM config_history_delete_backup
             WHERE delete_id = ?1 ORDER BY backup_id",
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map([&id], |row| {
            Ok(BackupTarget {
                id: row.get(0)?,
                dir: PathBuf::from(row.get::<_, String>(1)?),
            })
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    let mut targets = Vec::new();
    for row in rows {
        targets.push(row.map_err(|_| WriteError::LogReadFailed)?);
    }
    Ok(Some(OpenDelete {
        id,
        entry_key,
        operation_id,
        state,
        targets,
    }))
}

fn check_scope(scope: &HistoryScope, source: &OperationRecord) -> Result<(), WriteError> {
    if source.kind != scope.operation_kind()
        || source.steps.len() > MAX_SCOPE_FILES
        || source
            .steps
            .iter()
            .any(|step| !scope.owns_target(&step.target_path))
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(())
}

fn new_delete_id(connection: &Connection) -> Result<String, WriteError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| WriteError::LogWriteFailed)
}
