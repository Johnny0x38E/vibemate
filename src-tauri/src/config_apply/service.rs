//! Multi-file apply steps: record the plan, replace each file, and undo what the
//! step already replaced when a later file fails.
//!
//! [`apply_files`] is the entry point a feature uses. A failure inside it, and
//! [`compensate_operation`] for an operation recorded earlier, both call the
//! shared rollback in [`super::restore`], so an automatic compensation and a
//! user-confirmed restore judge conflicts and record results the same way. Every
//! replacement keeps the same guarantees: a token check, a private backup, a
//! same-directory temporary file and a verified re-read.

use serde::Serialize;

use super::{
    BackupStore, OperationKind, OperationState, StepState, WriteError, WriteTarget,
    apply_recorded_file, begin_operation, finish_operation, operation, path_text,
    record_step_applied, record_step_failed,
    restore::{pending_ordinals, roll_back_operation},
    revisions::{RevisionContext, RevisionIntent},
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
    apply_with_mode(storage, kind, owner, files, backups, false)
}

/// Apply a credential-bearing file set, keeping Unix replacements at 0600.
/// All other recording, backup, conflict and compensation behavior is shared.
pub(crate) fn apply_private_files(
    storage: &Storage,
    kind: OperationKind,
    owner: Option<&str>,
    files: &[PlannedFile],
    backups: &BackupStore,
) -> Result<AppliedOperation, WriteError> {
    apply_with_mode(storage, kind, owner, files, backups, true)
}

fn apply_with_mode(
    storage: &Storage,
    kind: OperationKind,
    owner: Option<&str>,
    files: &[PlannedFile],
    backups: &BackupStore,
    private: bool,
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
        let revision = RevisionContext {
            storage,
            operation_id: &operation_id,
            ordinal,
            batch_id: &operation_id,
            intent: RevisionIntent::Apply,
        };
        let result = apply_recorded_file(&file.target, &file.content, backups, private, &revision);
        match result {
            Ok(applied) => {
                let digest = applied
                    .target
                    .content_digest
                    .clone()
                    .ok_or(WriteError::OutcomeUnknown)?;
                let backup_path = applied.backup.as_ref().map(|backup| backup.path.clone());
                // A write that found the planned content already in place keeps
                // no copy, exactly like one that created the file; the recorded
                // fact keeps a later restore from deleting an existing file.
                let previously_existed = applied.backup.is_some() || !applied.changed;
                record_step_applied(
                    storage,
                    &operation_id,
                    ordinal,
                    applied.backup.as_ref(),
                    &digest,
                    previously_existed,
                )?;
                steps.push(StepOutcome {
                    ordinal,
                    target_path,
                    state: StepState::Applied,
                    error_code: None,
                    backup_path,
                });
            }
            Err(
                error @ (WriteError::LogWriteFailed
                | WriteError::StorageUnavailable
                | WriteError::OutcomeUnknown),
            ) => {
                // A post-write journal failure may leave changed bytes and a
                // prepared revision. Do not classify that as an untouched step
                // or automatically compensate using an incomplete legacy row.
                return Err(error);
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
                // Stop at the first failure and undo what this step replaced,
                // through the same rollback a user-confirmed restore uses. The
                // record is read again so the rollback sees the recorded steps.
                let recorded =
                    operation(storage, &operation_id)?.ok_or(WriteError::InvalidStoredOperation)?;
                let rolled_back = roll_back_operation(
                    storage,
                    &recorded,
                    &pending_ordinals(&recorded),
                    backups,
                    RevisionIntent::Compensation,
                )?;
                // The rollback records each step; this call settles the operation
                // itself, exactly like a user-confirmed restore does.
                finish_operation(storage, &operation_id, rolled_back.state)?;
                return Ok(AppliedOperation {
                    operation_id,
                    state: rolled_back.state,
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
/// Kept for the automatic compensation of a failed apply and for the recovery
/// entry (P34); a user-confirmed restore goes through [`super::apply_restore`]
/// instead, which additionally compares what the interface reviewed. Both use
/// the same rollback, so an operation a user partly restored reports the same
/// state here as it does there.
///
/// A file another writer changed after the write is left alone; the operation
/// then settles as `partial`, or as `unknown` when a rollback outcome could not
/// be verified, so the recovery entry can pick it up again. An operation whose
/// steps all failed settles as `failed`, and steps that were never applied are
/// not touched.
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
    let outcome = roll_back_operation(
        storage,
        &record,
        &pending_ordinals(&record),
        backups,
        RevisionIntent::Compensation,
    )?;
    finish_operation(storage, operation_id, outcome.state)?;
    Ok(outcome.state)
}
