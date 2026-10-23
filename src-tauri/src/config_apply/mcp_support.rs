//! Narrow, non-IPC access to Pi MCP's single-file written copies and recovery.
//! Entry-level restores reuse verified private copies without rolling unrelated
//! MCP entries back to an old whole-file version.

use super::{BackupStore, OperationKind, OperationState, StepState, WriteError};
use crate::{config_plan::read_target_bytes, storage::Storage};
use rusqlite::OptionalExtension;
use std::path::Path;

fn source(storage: &Storage, id: &str, root: &Path) -> Result<super::OperationRecord, WriteError> {
    let record = super::operation(storage, id)?.ok_or(WriteError::InvalidRequest)?;
    if record.kind != OperationKind::PiMcp
        || record.steps.len() != 1
        || Path::new(&record.steps[0].target_path) != root.join("mcp.json")
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(record)
}

fn copy(
    storage: &Storage,
    id: &str,
    root: &Path,
    backups: &BackupStore,
    available: bool,
) -> Result<Vec<u8>, WriteError> {
    source(storage, id, root)?;
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let state: Option<String> = connection
        .query_row("SELECT state FROM config_version WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?;
    if !matches!(state.as_deref(), Some("available"))
        && (available || state.as_deref() != Some("prepared"))
    {
        return Err(WriteError::BackupUnavailable);
    }
    let files = super::versions::files(&connection, id)?;
    drop(connection);
    if files.len() != 1
        || files[0].ordinal != 0
        || files[0].copy_id != format!("version-{id}-0")
        || Path::new(&files[0].target_path) != root.join("mcp.json")
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    let file = &files[0];
    super::snapshots::read_snapshot(
        backups,
        &file.copy_id,
        &file.target_path,
        &file.copy_path,
        &file.digest,
    )
}

/// Read the exact independent result of one successful MCP write for a domain
/// entry restore. No bytes may be returned through IPC. Scope and copy digest
/// are verified; absent, corrupt or unpublished copies are refused.
pub(crate) fn read_mcp_written_file(
    storage: &Storage,
    id: &str,
    root: &Path,
    backups: &BackupStore,
) -> Result<Vec<u8>, WriteError> {
    let record = source(storage, id, root)?;
    if record.state != OperationState::Applied || record.steps[0].state != StepState::Applied {
        return Err(WriteError::BackupUnavailable);
    }
    let bytes = copy(storage, id, root, backups, true)?;
    if record.steps[0].applied_digest.as_deref()
        != Some(crate::config_plan::digest_hex(&bytes).as_str())
    {
        return Err(WriteError::BackupUnavailable);
    }
    Ok(bytes)
}

/// Finalize a user-reviewed exact before/intended observation. The outer domain
/// holds the configuration guard and verifies its durable intent and approval.
/// Existing revision resolution repairs only unambiguous journal observations;
/// this function never rewrites an Agent file or invents missing snapshots.
pub(crate) fn resolve_mcp_observation_locked(
    storage: &Storage,
    id: &str,
    root: &Path,
    backups: &BackupStore,
    intended: bool,
    guard: &std::fs::File,
) -> Result<(), WriteError> {
    source(storage, id, root)?;
    if let Some(plan) = super::plan_revision_resolution(storage, id, backups)?
        && !plan.revisions.is_empty()
    {
        super::resolution::resolve_interrupted_revisions_locked(
            storage, id, &plan, backups, guard,
        )?;
    }
    if intended {
        let bytes = copy(storage, id, root, backups, false)?;
        let (target, _) =
            read_target_bytes(&root.join("mcp.json")).map_err(|_| WriteError::TargetUnreadable)?;
        if target.symlink
            || target.content_digest.as_deref()
                != Some(crate::config_plan::digest_hex(&bytes).as_str())
        {
            return Err(WriteError::PlanStale);
        }
        let record = source(storage, id, root)?;
        if record.steps[0].state != StepState::Applied {
            return Err(WriteError::HistoryUnresolved);
        }
        super::finish_operation(storage, id, OperationState::Applied)?;
        let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
        let state: String = connection
            .query_row("SELECT state FROM config_version WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|_| WriteError::LogReadFailed)?;
        drop(connection);
        if state == "prepared" {
            super::versions::publish(storage, id)?;
        } else if state != "available" {
            return Err(WriteError::BackupUnavailable);
        }
    } else {
        super::finish_operation(storage, id, OperationState::Failed)?;
    }
    Ok(())
}
