//! Multi-file apply steps: record the plan, replace each file, and undo what the
//! step already replaced when a later file fails.
//!
//! [`apply_files`] is the entry point a feature uses; [`compensate_operation`]
//! rolls back an operation recorded earlier, which is also what the recovery
//! entry (P34) calls after a restart. Both build on the single-file write in the
//! parent module, so every replacement keeps the same guarantees: a token check,
//! a private backup, a same-directory temporary file and a verified re-read.

use std::{fs, path::Path};

use serde::Serialize;

use super::{
    AppliedFile, Backup, BackupStore, MAX_TARGET_BYTES, OperationKind, OperationState, StepRecord,
    StepState, WriteError, WriteTarget, apply_file, begin_operation, digest_hex, finish_operation,
    operation, path_text, read_target, record_step_applied, record_step_failed,
};
use crate::storage::Storage;

/// One file of an apply step: the target a plan described, and the bytes to write.
///
/// The bytes are written exactly as given, so the caller injects a credential it
/// read from OS storage beforehand. Content never travels back out of this
/// module except through the private backup directory.
pub struct PlannedFile {
    /// Target identity and change token taken from the plan.
    pub target: WriteTarget,
    /// Exact file content, already rendered.
    pub content: Vec<u8>,
}

/// What one file of a settled operation ended up as.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutcome {
    /// Position of the file in the operation, counted from zero.
    pub ordinal: u32,
    /// Absolute path of the file.
    pub target_path: String,
    /// Final state of this file.
    pub state: StepState,
    /// Stable code of the failure; present exactly when the step failed.
    pub error_code: Option<String>,
    /// Absolute path of the copy holding the content this step replaced, when
    /// there was one; the interface shows it so the user can find the backup.
    pub backup_path: Option<String>,
}

/// Result of one apply step over one or more files.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOperation {
    /// Identity of the recorded operation.
    pub operation_id: String,
    /// Final recorded state: `applied`, or what the rollback reached.
    pub state: OperationState,
    /// One entry per file the step attempted.
    pub steps: Vec<StepOutcome>,
}

/// Replace every file of one apply step, or undo the ones already replaced.
///
/// The operation is recorded before the first file is touched, and each file is
/// recorded as it finishes. `owner` names what the step writes configuration for
/// (the Provider ID for a model link) and is stored with the operation row, so a
/// later restore can label which key the written file carried.
/// A failure at any file stops the step, rolls back the
/// files already replaced, and returns the settled state instead of an error:
/// `failed` when no file was replaced, `compensated` when the rollback undid all
/// of them, `partial` or `unknown` when a file could not be restored. The
/// failure reason travels in the step's `error_code`.
///
/// An error is returned only when the log itself cannot be trusted, because the
/// result then cannot be reported honestly; the recorded operation stays
/// discoverable through [`unfinished_operations`](super::unfinished_operations).
///
/// # Errors
/// Returns `InvalidRequest` for an empty file list, a path that cannot be shown
/// or an owner reference of the wrong shape, and `StorageUnavailable`,
/// `LogWriteFailed` or `LogReadFailed` when the operation log cannot be written
/// or read.
pub fn apply_files(
    storage: &Storage,
    kind: OperationKind,
    owner: Option<&str>,
    files: &[PlannedFile],
    backups: &BackupStore,
) -> Result<AppliedOperation, WriteError> {
    if files.is_empty() {
        return Err(WriteError::InvalidRequest);
    }
    let targets: Vec<WriteTarget> = files.iter().map(|file| file.target.clone()).collect();
    let operation_id = begin_operation(storage, kind, owner, &targets)?;
    let mut steps = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| WriteError::InvalidRequest)?;
        let target_path = path_text(&file.target.path)?;
        match apply_file(&file.target, &file.content, backups) {
            Ok(applied) => {
                let digest = applied
                    .target
                    .content_digest
                    .ok_or(WriteError::OutcomeUnknown)?;
                let backup_path = applied.backup.as_ref().map(|backup| backup.path.clone());
                record_step_applied(
                    storage,
                    &operation_id,
                    ordinal,
                    applied.backup.as_ref(),
                    &digest,
                )?;
                steps.push(StepOutcome {
                    ordinal,
                    target_path,
                    state: StepState::Applied,
                    error_code: None,
                    backup_path,
                });
            }
            Err(error) => {
                record_step_failed(storage, &operation_id, ordinal, error)?;
                steps.push(StepOutcome {
                    ordinal,
                    target_path,
                    state: StepState::Failed,
                    error_code: Some(error.code().to_owned()),
                    backup_path: None,
                });
                let state = compensate_operation(storage, &operation_id, backups)?;
                return Ok(AppliedOperation {
                    operation_id,
                    state,
                    steps,
                });
            }
        }
    }
    finish_operation(storage, &operation_id, OperationState::Applied)?;
    Ok(AppliedOperation {
        operation_id,
        state: OperationState::Applied,
        steps,
    })
}

/// Undo the files an unfinished operation already replaced, newest first.
///
/// Each file is restored from the copy stored before it was replaced, or removed
/// when the operation created it. A file another writer changed after the write
/// is left alone; the operation then settles as `partial`, or as `unknown` when
/// a rollback outcome could not be verified, so the recovery entry can pick it
/// up again. An operation whose steps all failed settles as `failed`, and steps
/// that were never applied are not touched.
///
/// # Errors
/// Returns `InvalidRequest` when the operation does not exist or has already
/// settled, and the log errors of [`apply_files`].
pub fn compensate_operation(
    storage: &Storage,
    operation_id: &str,
    backups: &BackupStore,
) -> Result<OperationState, WriteError> {
    let record = operation(storage, operation_id)?.ok_or(WriteError::InvalidRequest)?;
    if !record.state.is_unfinished() {
        return Err(WriteError::InvalidRequest);
    }
    // Nothing was replaced, so there is nothing to undo.
    if !record
        .steps
        .iter()
        .any(|step| step.state == StepState::Applied)
    {
        finish_operation(storage, operation_id, OperationState::Failed)?;
        return Ok(OperationState::Failed);
    }
    let mut restored = true;
    let mut unknown = false;
    for step in record.steps.iter().rev() {
        if step.state != StepState::Applied {
            continue;
        }
        if let Err(error) = restore_step(step, backups) {
            restored = false;
            unknown |= error == WriteError::OutcomeUnknown;
        }
    }
    let state = match (restored, unknown) {
        (true, _) => OperationState::Compensated,
        (false, true) => OperationState::Unknown,
        (false, false) => OperationState::Partial,
    };
    finish_operation(storage, operation_id, state)?;
    Ok(state)
}

/// Put back the content a recorded step replaced.
///
/// The stored copy is checked against the digest recorded with it, and the write
/// goes through [`apply_file`], so the rollback keeps the same guarantees: the
/// file must still hold what this module wrote, the replacement goes through a
/// temporary file, and the content it removes is copied first.
///
/// # Errors
/// Returns `BackupUnavailable` when the stored copy cannot be read or no longer
/// matches its digest, `TargetChanged` when the file changed after the write
/// being undone, and the [`apply_file`] errors for the new write.
pub fn restore_file(
    path: &Path,
    applied_digest: &str,
    backup: &Backup,
    backups: &BackupStore,
) -> Result<AppliedFile, WriteError> {
    let bytes = read_backup(backup)?;
    apply_file(
        &WriteTarget {
            path: path.to_path_buf(),
            expected_digest: Some(applied_digest.to_owned()),
        },
        &bytes,
        backups,
    )
}

/// Remove a file this module created, when it still holds exactly that content.
///
/// The file did not exist before the write, so a rollback removes it instead of
/// restoring content. A file that another writer changed is left in place, and a
/// file that is already gone counts as removed.
///
/// # Errors
/// Returns `SymlinkRefused` for a link, `TargetChanged` when the content is not
/// the one this module wrote, `TargetUnreadable` when the path cannot be read,
/// and `WriteFailed` when the removal failed.
pub fn remove_created_file(path: &Path, applied_digest: &str) -> Result<(), WriteError> {
    let document = read_target(path)?;
    if document.target.symlink {
        return Err(WriteError::SymlinkRefused);
    }
    if !document.target.exists {
        return Ok(());
    }
    if document.target.content_digest.as_deref() != Some(applied_digest) {
        return Err(WriteError::TargetChanged);
    }
    fs::remove_file(path).map_err(|_| WriteError::WriteFailed)
}

/// Roll back one applied step and leave no copy of the content it removed.
fn restore_step(step: &StepRecord, backups: &BackupStore) -> Result<(), WriteError> {
    let digest = step
        .applied_digest
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    let path = Path::new(&step.target_path);
    let Some(backup) = &step.backup else {
        return remove_created_file(path, digest);
    };
    let applied = restore_file(path, digest, backup, backups)?;
    // The restored file is what the user had; the copy this rollback took of the
    // content it removed is not referenced by any step, so it is discarded.
    if let Some(created) = &applied.backup {
        backups.discard(created);
    }
    Ok(())
}

/// Read a stored copy, bounded like a target file and checked against its digest.
fn read_backup(backup: &Backup) -> Result<Vec<u8>, WriteError> {
    let path = Path::new(&backup.path);
    let metadata = fs::metadata(path).map_err(|_| WriteError::BackupUnavailable)?;
    if metadata.len() > MAX_TARGET_BYTES {
        return Err(WriteError::BackupUnavailable);
    }
    let bytes = fs::read(path).map_err(|_| WriteError::BackupUnavailable)?;
    if digest_hex(&bytes) != backup.content_digest {
        return Err(WriteError::BackupUnavailable);
    }
    Ok(bytes)
}
