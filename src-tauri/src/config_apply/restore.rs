//! Restore planning and rollback for recorded write operations (P21).
//!
//! [`plan_restore`] reads an operation and says what undoing each of its steps
//! would do. The answer carries paths, digests, actions and the owner the log can
//! prove; file content never leaves the private backup directory.
//!
//! [`apply_restore`] rebuilds that plan, compares the tokens the interface
//! reviewed, and then puts the stored copies back through the same single-file
//! write the apply step uses, so a rollback keeps the same guarantees: the file
//! must still hold what the operation wrote, the replacement goes through a
//! temporary file, and the result is re-read.
//!
//! [`roll_back_operation`] is the rollback shared by [`apply_restore`] and the
//! automatic compensation of a failed apply ([`super::compensate_operation`]).
//! It records what happened to each step, which is what lets a partly rolled back
//! operation be retried: the first attempt may stop at a file the user edited,
//! and the next attempt must skip the steps already undone.

use std::{fs, path::Path};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::{
    AppliedFile, Backup, BackupStore, MAX_TARGET_BYTES, OperationKind, OperationRecord,
    OperationState, RestoreState, StepRecord, StepState, WriteError, WriteTarget, apply_file,
    apply_recorded_file, digest_hex, file_name, finish_operation, is_read_only, operation,
    operation_id, read_replaced_bytes, read_target, record_step_restored,
    revisions::{RevisionContext, RevisionIntent},
    sync_directory,
};
use crate::storage::Storage;

/// Reason code for a step an older log cannot classify.
///
/// A step that keeps no copy either created its file or found the planned content
/// already in place. A row written before the log stated which one it was cannot
/// be told apart, and guessing would delete a file the user already had.
const HISTORY_UNKNOWN: &str = "restore_history_unknown";

/// What a restore would do to one file of a recorded operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreAction {
    /// Write the stored copy back over the file.
    Restore,
    /// Remove a file the recorded operation created.
    Remove,
    /// The file changed after the recorded write; leave it untouched.
    Conflict,
    /// The previous content cannot be produced safely; leave the file untouched.
    Blocked,
    /// Nothing to undo for this step.
    Skip,
}

impl RestoreAction {
    /// Whether carrying out this action changes the file system.
    pub(crate) fn changes_files(self) -> bool {
        matches!(self, Self::Restore | Self::Remove)
    }
}

/// One file of a restore preview, without any file content.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStepPlan {
    /// Position of the step in the operation, counted from zero.
    pub ordinal: u32,
    /// Absolute path this step changed.
    pub target_path: String,
    /// What the restore would do.
    pub action: RestoreAction,
    /// Stable code explaining a `conflict` or `blocked` step.
    pub reason: Option<String>,
    /// SHA-256 now on disk; `None` when the file is missing.
    pub current_digest: Option<String>,
    /// SHA-256 the recorded write left; `None` when the step never wrote.
    pub applied_digest: Option<String>,
    /// Owner of the content a restore would put back, when the log can tell.
    pub restored_owner_ref: Option<String>,
    /// What an earlier rollback attempt recorded, when there was one.
    pub restore_state: Option<RestoreState>,
}

/// Everything the interface needs to show one restore preview.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePlan {
    /// Identity of the operation that would be undone.
    pub operation_id: String,
    /// Feature that owns the operation.
    pub kind: OperationKind,
    /// Reference the operation wrote configuration for, when it recorded one.
    pub owner_ref: Option<String>,
    /// How far the operation got.
    pub state: OperationState,
    /// Unix milliseconds when the operation was recorded.
    pub created_at: i64,
    /// Whether at least one step would change the file system.
    pub restorable: bool,
    /// One entry per recorded step, in the order they were applied.
    pub steps: Vec<RestoreStepPlan>,
}

/// One step as the interface saw it when the user confirmed the restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedRestoreStep {
    /// Position of the step in the operation, counted from zero.
    pub ordinal: u32,
    /// SHA-256 the preview showed for the file at that moment.
    pub current_digest: Option<String>,
}

/// What one step of a restore did.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStepOutcome {
    /// Position of the step in the operation, counted from zero.
    pub ordinal: u32,
    /// Absolute path this step changed.
    pub target_path: String,
    /// Action that was attempted.
    pub action: RestoreAction,
    /// Result recorded for the step.
    pub state: RestoreState,
    /// Stable code of a write that did not complete in this attempt.
    pub error_code: Option<String>,
    /// Legacy cleanup field retained for IPC compatibility. Journaled snapshots
    /// are retained versions, not temporary copies awaiting deletion.
    pub leftover_path: Option<String>,
}

/// Result of one rollback or one confirmed restore.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    /// Identity of the operation that was undone.
    pub operation_id: String,
    /// State the operation settled into.
    pub state: OperationState,
    /// One entry per step that was attempted, newest first.
    pub steps: Vec<RestoreStepOutcome>,
    /// Legacy cleanup paths, kept for IPC compatibility; new snapshots are
    /// retained in the revision log instead of being reported as leftovers.
    pub leftovers: Vec<String>,
}

/// Preview what undoing a recorded operation would do, without writing anything.
///
/// Returns `Ok(None)` when no operation has that identity, so a caller can answer
/// "no such operation" without treating it as a failure of the write layer. Every
/// step is classified from the file as it is now, so a step whose file the user
/// edited shows as a conflict instead of a restore.
///
/// # Errors
/// Returns `InvalidRequest` for a malformed identity, `InvalidStoredOperation`
/// when the log cannot be trusted, and `StorageUnavailable` or `LogReadFailed`
/// when the operation cannot be read.
pub fn plan_restore(
    storage: &Storage,
    operation_id: &str,
) -> Result<Option<RestorePlan>, WriteError> {
    let Some(record) = operation(storage, operation_id)? else {
        return Ok(None);
    };
    plan_record(storage, &record).map(Some)
}

/// Build the plan for an operation the caller already read.
fn plan_record(storage: &Storage, record: &OperationRecord) -> Result<RestorePlan, WriteError> {
    let mut steps = Vec::with_capacity(record.steps.len());
    for step in &record.steps {
        steps.push(classify(storage, record, step)?);
    }
    let restorable = steps.iter().any(|step| step.action.changes_files());
    Ok(RestorePlan {
        operation_id: record.id.clone(),
        kind: record.kind,
        owner_ref: record.owner_ref.clone(),
        state: record.state,
        created_at: record.created_at,
        restorable,
        steps,
    })
}

/// Carry out a preview the user reviewed, or refuse when it no longer holds.
///
/// The plan is rebuilt from the files as they are now and compared with the
/// reviewed tokens: the same steps, each with the digest the preview showed. Any
/// difference means the approval no longer describes the files, and nothing is
/// written ([`WriteError::PlanStale`]). Steps classified as `conflict` or
/// `blocked` are not restorable and are reported as such instead of being
/// overwritten.
///
/// # Errors
/// Returns `PlanStale` when the reviewed plan no longer matches, `InvalidRequest`
/// when the operation does not exist, and the log or write errors of
/// [`super::apply_files`].
pub fn apply_restore(
    storage: &Storage,
    operation_id: &str,
    reviewed: &[ReviewedRestoreStep],
    backups: &BackupStore,
) -> Result<RestoreOutcome, WriteError> {
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    let record = operation(storage, operation_id)?.ok_or(WriteError::InvalidRequest)?;
    super::revisions::require_known_revisions(storage, operation_id)?;
    let plan = plan_record(storage, &record)?;
    let planned: Vec<&RestoreStepPlan> = plan
        .steps
        .iter()
        .filter(|step| step.action.changes_files())
        .collect();
    // Nothing to restore means the interface offered a state that no longer
    // exists, which is the same situation as a stale approval.
    if planned.is_empty() || !review_matches(&planned, reviewed) {
        return Err(WriteError::PlanStale);
    }
    let ordinals: Vec<u32> = planned.iter().map(|step| step.ordinal).collect();
    let outcome = roll_back_operation(
        storage,
        &record,
        &ordinals,
        backups,
        RevisionIntent::Restore,
    )?;
    finish_operation(storage, &record.id, outcome.state)?;
    Ok(outcome)
}

/// Whether the reviewed steps describe exactly the steps a restore would do.
///
/// Order does not matter, but every reviewed ordinal must match one planned step
/// with the digest the preview showed, and no ordinal may repeat.
fn review_matches(planned: &[&RestoreStepPlan], reviewed: &[ReviewedRestoreStep]) -> bool {
    if planned.len() != reviewed.len() {
        return false;
    }
    let mut seen = Vec::with_capacity(reviewed.len());
    for entry in reviewed {
        if seen.contains(&entry.ordinal) {
            return false;
        }
        seen.push(entry.ordinal);
        let Some(step) = planned.iter().find(|step| step.ordinal == entry.ordinal) else {
            return false;
        };
        if step.current_digest != entry.current_digest {
            return false;
        }
    }
    true
}

/// Undo the listed steps of a recorded operation, newest first.
///
/// The caller decides which steps to attempt: [`apply_restore`] lists exactly the
/// steps its preview said it would change, and
/// [`compensate_operation`](super::compensate_operation) lists every applied step
/// that still needs undoing. A step already restored, one that never wrote, and
/// an applied step that never changed its file are skipped whatever the list
/// says, so a retry continues instead of starting over. Each attempt is recorded
/// on the step row, and the operation's final state comes from those results.
///
/// # Errors
/// Returns `InvalidStoredOperation` when a step lacks the digest of what it
/// wrote, and the log errors of [`record_step_restored`].
pub(crate) fn roll_back_operation(
    storage: &Storage,
    record: &OperationRecord,
    ordinals: &[u32],
    backups: &BackupStore,
    intent: RevisionIntent,
) -> Result<RestoreOutcome, WriteError> {
    super::revisions::require_known_revisions(storage, &record.id)?;
    let batch_id = operation_id();
    let mut attempted = Vec::new();
    let mut leftovers = Vec::new();
    for step in record.steps.iter().rev() {
        if !ordinals.contains(&step.ordinal)
            || step.state != StepState::Applied
            || step_is_settled(step)
        {
            continue;
        }
        let revision = RevisionContext {
            storage,
            operation_id: &record.id,
            ordinal: step.ordinal,
            batch_id: &batch_id,
            intent,
            direct: false,
        };
        let outcome = roll_back_step(step, backups, &revision)?;
        if let Some(path) = &outcome.leftover_path {
            leftovers.push(path.clone());
        }
        record_step_restored(storage, &record.id, step.ordinal, outcome.state)?;
        attempted.push(outcome);
    }
    let state = settle(record, &attempted);
    Ok(RestoreOutcome {
        operation_id: record.id.clone(),
        state,
        steps: attempted,
        leftovers,
    })
}

/// Undo one applied step and report what happened, without touching the log.
fn roll_back_step(
    step: &StepRecord,
    backups: &BackupStore,
    revision: &RevisionContext<'_>,
) -> Result<RestoreStepOutcome, WriteError> {
    let digest = step
        .applied_digest
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    let path = Path::new(&step.target_path);
    let attempt = match &step.backup {
        Some(backup) => restore_from_copy(path, digest, backup, backups, revision)?,
        None => remove_created(path, digest, backups, revision)?,
    };
    Ok(RestoreStepOutcome {
        ordinal: step.ordinal,
        target_path: step.target_path.clone(),
        action: if step.backup.is_some() {
            RestoreAction::Restore
        } else {
            RestoreAction::Remove
        },
        state: attempt.state,
        error_code: attempt.error_code,
        leftover_path: attempt.leftover,
    })
}

/// What one rollback attempt produced, before the log records it.
struct RollbackAttempt {
    state: RestoreState,
    error_code: Option<String>,
    leftover: Option<String>,
}

impl RollbackAttempt {
    /// The step is back to its pre-operation content.
    fn restored() -> Self {
        Self {
            state: RestoreState::Restored,
            error_code: None,
            leftover: None,
        }
    }

    /// The step did not reach its pre-operation content, for this reason.
    fn failed(state: RestoreState, error: WriteError) -> Self {
        Self {
            state,
            error_code: Some(error.code().to_owned()),
            leftover: None,
        }
    }

    /// Map a write failure onto a rollback result.
    ///
    /// Errors that say the log or the storage itself is unusable are handed back
    /// as errors: their outcome cannot be recorded honestly. Everything else
    /// becomes a result the interface shows.
    fn from_error(error: WriteError) -> Result<Self, WriteError> {
        match error {
            WriteError::StorageUnavailable
            | WriteError::LogWriteFailed
            | WriteError::LogReadFailed
            | WriteError::InvalidStoredOperation => Err(error),
            WriteError::BackupUnavailable => Ok(Self::failed(RestoreState::Unavailable, error)),
            WriteError::OutcomeUnknown => Ok(Self::failed(RestoreState::Unknown, error)),
            WriteError::TargetChanged => Ok(Self::failed(RestoreState::Conflict, error)),
            _ => Ok(Self::failed(RestoreState::Failed, error)),
        }
    }
}

/// Put the content back while retaining an independently journaled pre-image.
fn restore_from_copy(
    path: &Path,
    applied_digest: &str,
    backup: &Backup,
    backups: &BackupStore,
    revision: &RevisionContext<'_>,
) -> Result<RollbackAttempt, WriteError> {
    let bytes = match read_backup(backup) {
        Ok(bytes) => bytes,
        Err(error) => return RollbackAttempt::from_error(error),
    };
    let target = WriteTarget {
        path: path.to_path_buf(),
        expected_digest: Some(applied_digest.to_owned()),
    };
    match apply_recorded_file(&target, &bytes, backups, false, revision) {
        Ok(_) => Ok(RollbackAttempt::restored()),
        Err(WriteError::TargetChanged) => {
            // Another restore may already have put this content back. That is the
            // state this step wanted, not a file the user changed to something
            // else, and the difference decides whether a retry can ever finish.
            match current_digest(path) {
                Ok(Some(digest)) if digest == backup.content_digest => {
                    Ok(RollbackAttempt::restored())
                }
                Ok(_) => Ok(RollbackAttempt::failed(
                    RestoreState::Conflict,
                    WriteError::TargetChanged,
                )),
                Err(error) => RollbackAttempt::from_error(error),
            }
        }
        Err(error) => RollbackAttempt::from_error(error),
    }
}

/// Remove a file the recorded step created, when it still holds that content.
fn remove_created(
    path: &Path,
    applied_digest: &str,
    backups: &BackupStore,
    revision: &RevisionContext<'_>,
) -> Result<RollbackAttempt, WriteError> {
    match remove_recorded_file(path, applied_digest, backups, revision) {
        Ok(()) => Ok(RollbackAttempt::restored()),
        Err(error) => RollbackAttempt::from_error(error),
    }
}

/// Preserve the complete current file before restoring an absent-file state.
pub(super) fn remove_recorded_file(
    path: &Path,
    applied_digest: &str,
    backups: &BackupStore,
    revision: &RevisionContext<'_>,
) -> Result<(), WriteError> {
    let current = read_target(path)?;
    if current.symlink {
        return Err(WriteError::SymlinkRefused);
    }
    if !current.exists {
        return Ok(());
    }
    if current.content_digest.as_deref() != Some(applied_digest) {
        return Err(WriteError::TargetChanged);
    }
    if is_read_only(path)? {
        return Err(WriteError::TargetReadOnly);
    }
    let bytes = read_replaced_bytes(path)?;
    if digest_hex(&bytes) != applied_digest {
        return Err(WriteError::TargetChanged);
    }
    let id = operation_id();
    let backup = backups.store(&id, &file_name(path), &bytes)?;
    let target = WriteTarget {
        path: path.to_path_buf(),
        expected_digest: Some(applied_digest.to_owned()),
    };
    revision.prepare(&id, &target, Some(&backup), None)?;
    let result = (|| {
        remove_created_file(path, applied_digest)?;
        if let Some(parent) = path.parent() {
            sync_directory(parent)?;
        }
        let after = read_target(path).map_err(|_| WriteError::OutcomeUnknown)?;
        if after.exists || after.symlink {
            return Err(WriteError::OutcomeUnknown);
        }
        Ok(())
    })();
    revision.finish(&id, &result)?;
    result
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
    let current = read_target(path)?;
    if current.symlink {
        return Err(WriteError::SymlinkRefused);
    }
    if !current.exists {
        return Ok(());
    }
    if current.content_digest.as_deref() != Some(applied_digest) {
        return Err(WriteError::TargetChanged);
    }
    fs::remove_file(path).map_err(|_| WriteError::WriteFailed)
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

/// SHA-256 currently on disk, or `None` when the file is absent.
fn current_digest(path: &Path) -> Result<Option<String>, WriteError> {
    Ok(read_target(path)?.content_digest)
}

/// Classify what undoing one step would do, from the file as it is now.
fn classify(
    storage: &Storage,
    record: &OperationRecord,
    step: &StepRecord,
) -> Result<RestoreStepPlan, WriteError> {
    let mut plan = RestoreStepPlan {
        ordinal: step.ordinal,
        target_path: step.target_path.clone(),
        action: RestoreAction::Skip,
        reason: None,
        current_digest: None,
        applied_digest: step.applied_digest.clone(),
        restored_owner_ref: None,
        restore_state: step.restore_state,
    };
    // A step that never wrote, and one a rollback already undid, need nothing.
    if step.state != StepState::Applied || step.restore_state == Some(RestoreState::Restored) {
        return Ok(plan);
    }
    let Some(applied_digest) = step.applied_digest.as_deref() else {
        return Err(WriteError::InvalidStoredOperation);
    };
    let current = match read_target(Path::new(&step.target_path)) {
        Ok(current) => current,
        // A file this module cannot read or that is too large cannot be replaced
        // safely; the reason travels to the interface instead of a guess.
        Err(error) => return Ok(plan.stopped(RestoreAction::Blocked, error.code())),
    };
    plan.current_digest = current.content_digest.clone();
    if current.symlink {
        return Ok(plan.stopped(RestoreAction::Blocked, WriteError::SymlinkRefused.code()));
    }
    match current.content_digest.as_deref() {
        // The file a replacing write changed is gone. Recreating it would put
        // configuration back that the user removed, so it is a conflict.
        None if step.backup.is_some() => {
            Ok(plan.stopped(RestoreAction::Conflict, WriteError::TargetChanged.code()))
        }
        None => Ok(without_copy(plan, step)),
        Some(current) if current == applied_digest => match &step.backup {
            Some(backup) => match read_backup(backup) {
                Ok(_) => {
                    plan.action = RestoreAction::Restore;
                    plan.restored_owner_ref =
                        owner_for_digest(storage, record.kind, &backup.content_digest)?;
                    Ok(plan)
                }
                Err(error) => Ok(plan.stopped(RestoreAction::Blocked, error.code())),
            },
            None => Ok(without_copy(plan, step)),
        },
        Some(_) => Ok(plan.stopped(RestoreAction::Conflict, WriteError::TargetChanged.code())),
    }
}

/// Decide what an applied step that kept no copy needs.
///
/// The log says whether the file existed before the step ran: a step that created
/// it is removed, and one that found the planned content already in place needs
/// nothing. A row an older build wrote does not say, and guessing would delete a
/// file the user already had.
fn without_copy(mut plan: RestoreStepPlan, step: &StepRecord) -> RestoreStepPlan {
    match step.previous_existed {
        Some(false) => plan.action = RestoreAction::Remove,
        Some(true) => plan.action = RestoreAction::Skip,
        None => {
            plan.action = RestoreAction::Blocked;
            plan.reason = Some(HISTORY_UNKNOWN.to_owned());
        }
    }
    plan
}

impl RestoreStepPlan {
    /// Mark this step as one a restore will not carry out, with its reason.
    fn stopped(mut self, action: RestoreAction, reason: &str) -> Self {
        self.action = action;
        self.reason = Some(reason.to_owned());
        self
    }
}

/// The operation that last wrote this exact content, when the log knows one.
///
/// A restore puts back the bytes a step replaced. Those bytes were written by an
/// earlier operation when its applied digest matches, and that operation's owner
/// is the Provider whose key the restored file will carry. `None` means no
/// recorded write produced this content, so it came from outside vibemate.
pub(super) fn owner_for_digest(
    storage: &Storage,
    kind: OperationKind,
    digest: &str,
) -> Result<Option<String>, WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    connection
        .query_row(
            "SELECT owner.owner_ref
             FROM config_operation_step AS step
             JOIN config_operation AS owner ON owner.id = step.operation_id
             WHERE owner.kind = ?1 AND step.applied_digest = ?2
             ORDER BY owner.created_at DESC, owner.id DESC
             LIMIT 1",
            params![kind.as_str(), digest],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)
        .map(Option::flatten)
}

/// Whether a step needs no further rollback.
///
/// A recorded result of `restored` says so, and so does a step that never changed
/// its file: it found the planned content already in place.
fn step_is_settled(step: &StepRecord) -> bool {
    matches!(
        effective_restore_state(step, &[]),
        Some(RestoreState::Restored)
    )
}

/// Ordinals of the applied steps that still need undoing.
///
/// The automatic compensation passes this list, so it retries a conflict the user
/// has since resolved and never touches a step it already restored.
pub(crate) fn pending_ordinals(record: &OperationRecord) -> Vec<u32> {
    record
        .steps
        .iter()
        .filter(|step| step.state == StepState::Applied && !step_is_settled(step))
        .map(|step| step.ordinal)
        .collect()
}

/// The result that counts for one step: this attempt's, or the recorded one.
fn effective_restore_state(
    step: &StepRecord,
    attempted: &[RestoreStepOutcome],
) -> Option<RestoreState> {
    if let Some(outcome) = attempted
        .iter()
        .find(|outcome| outcome.ordinal == step.ordinal)
    {
        return Some(outcome.state);
    }
    if step.backup.is_none() && step.previous_existed == Some(true) {
        // The step never changed the file, so there is nothing to undo.
        return Some(RestoreState::Restored);
    }
    step.restore_state
}

/// The state an operation settled into after a rollback attempt.
///
/// `Failed` means no step ever replaced a file, `Compensated` means every applied
/// step is back to its pre-operation content, `Unknown` means at least one
/// outcome could not be verified, and `Partial` covers everything else, including
/// a conflict the user still has to resolve.
pub(super) fn settle(record: &OperationRecord, attempted: &[RestoreStepOutcome]) -> OperationState {
    let applied: Vec<&StepRecord> = record
        .steps
        .iter()
        .filter(|step| step.state == StepState::Applied)
        .collect();
    if applied.is_empty() {
        return OperationState::Failed;
    }
    let states: Vec<Option<RestoreState>> = applied
        .iter()
        .map(|step| effective_restore_state(step, attempted))
        .collect();
    if states
        .iter()
        .all(|state| *state == Some(RestoreState::Restored))
    {
        return OperationState::Compensated;
    }
    if states.contains(&Some(RestoreState::Unknown)) {
        return OperationState::Unknown;
    }
    OperationState::Partial
}
