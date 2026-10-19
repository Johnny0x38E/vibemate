//! Direct restore of any retained pre-mutation version (H2).
//!
//! A history entry names one apply attempt or one restore/compensation batch.
//! Every revision in that entry retained the exact bytes before one file
//! mutation, so the entry describes the configuration as it was just before the
//! entry ran. This module plans changing the current files to that state and,
//! after the user confirms, writes it through the shared recorded-write path.
//!
//! A version restore is a new restore batch under the source operation; it does
//! not edit the source operation's legacy rollback result. Before each write the
//! shared writer stores the current bytes as an independent snapshot, so the
//! user can restore the state this restore replaced.
//!
//! Planning validates the entry's Agent operation kind and exact file scope,
//! verifies every private snapshot against its recorded layout and digest, and
//! returns the current digest that the confirmation must send back. Applying
//! rebuilds the same plan under the cross-instance configuration lock and
//! refuses when any file changed since the preview ([`WriteError::PlanStale`]).

use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde::Serialize;

use super::{
    BackupStore, HistoryIntent, HistoryScope, OperationKind, RestoreState, WriteError, WriteTarget,
    apply_recorded_file,
    history::{
        MAX_SCOPE_FILES, StoredRevision, read_revisions, valid_entry_key, valid_revision_id,
    },
    inspection::has_interrupted_revisions,
    operations::read_operation,
    read_target,
    restore::{owner_for_digest, remove_recorded_file},
    revisions::{RevisionContext, RevisionIntent},
    snapshots::read_snapshot,
};
use crate::storage::Storage;

/// The entry has no proven retained bytes for a file that changed since.
const SNAPSHOT_UNAVAILABLE: &str = "snapshot_unavailable";
/// Interrupted mutations must be inspected and recorded before any restore.
const HISTORY_UNRESOLVED: &str = "history_unresolved";

/// What restoring one retained version would do to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRestoreAction {
    /// Write the retained bytes back.
    Restore,
    /// The retained state was absence, so remove the current file.
    Remove,
    /// The file already holds the retained state.
    Skip,
    /// The retained state cannot be produced safely; leave the file untouched.
    Blocked,
}

impl VersionRestoreAction {
    /// Whether carrying out this action changes the file system.
    fn changes_files(self) -> bool {
        matches!(self, Self::Restore | Self::Remove)
    }
}

/// One file of a version-restore preview, without content or backup paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreFile {
    /// Retained revision this version comes from; absent for a legacy step that
    /// never recorded a snapshot.
    pub revision_id: Option<String>,
    /// Position in the source operation.
    pub ordinal: u32,
    /// Absolute path this file would change.
    pub target_path: String,
    /// What the restore would do.
    pub action: VersionRestoreAction,
    /// Stable code explaining a `blocked` action.
    pub reason: Option<String>,
    /// SHA-256 on disk at preview time; the confirmation sends this back.
    pub current_digest: Option<String>,
    /// SHA-256 of the retained version; null means the file did not exist.
    pub target_digest: Option<String>,
    /// Reference of the Provider whose recorded write produced the bytes that
    /// come back, when the journal can prove one.
    pub restored_owner_ref: Option<String>,
}

/// Everything the interface needs to show one version-restore preview.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestorePlan {
    /// Entry whose pre-mutation state would be restored.
    pub entry_key: String,
    /// Source operation identity shared by all files.
    pub operation_id: String,
    /// Kind of entry (apply, manual restore or automatic compensation).
    pub intent: HistoryIntent,
    /// Reference recorded on the source operation, when it has one.
    pub owner_ref: Option<String>,
    /// Unix milliseconds when the source entry was created.
    pub created_at: i64,
    /// One file per mutation in the entry (or per planned source step).
    pub files: Vec<VersionRestoreFile>,
    /// Whether at least one file would change and none is blocked.
    pub restorable: bool,
    /// Plan-level refusal, currently only for interrupted journals.
    pub reason: Option<String>,
}

/// One file as the interface saw it when the user confirmed the restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedVersionFile {
    /// Retained revision whose pre-state the user reviewed.
    pub revision_id: String,
    /// SHA-256 the preview showed for the current file.
    pub current_digest: Option<String>,
}

/// What one file of a version restore did.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreFileOutcome {
    /// Retained revision that was used as the source.
    pub revision_id: String,
    /// Absolute path this file changed.
    pub target_path: String,
    /// Action that was attempted.
    pub action: VersionRestoreAction,
    /// Result recorded for this file.
    pub state: RestoreState,
    /// Stable code of a write that did not complete.
    pub error_code: Option<String>,
}

/// Result of one confirmed version restore.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreOutcome {
    /// Entry whose pre-mutation state was requested.
    pub entry_key: String,
    /// One entry per file that was attempted, in reverse plan order.
    pub files: Vec<VersionRestoreFileOutcome>,
}

/// Preview restoring one history entry's pre-mutation state, without writing.
///
/// The scope is built by trusted Agent code from the active configuration root;
/// a frontend cannot supply paths. An unresolvable entry or one outside the
/// scope is refused rather than reported as an empty plan.
///
/// # Errors
/// Returns `InvalidRequest` for a malformed entry key or a missing operation,
/// `InvalidStoredOperation` for malformed or out-of-scope journal rows, and
/// storage/log errors when the journal cannot be read.
pub fn plan_version_restore(
    storage: &Storage,
    scope: &HistoryScope,
    entry_key: &str,
    backups: &BackupStore,
) -> Result<VersionRestorePlan, WriteError> {
    Ok(build_plan(storage, scope, entry_key, backups)?.into_view())
}

/// Carry out a reviewed version restore, or refuse when its tokens no longer hold.
///
/// Only the files the preview said it would change travel back as tokens. The
/// plan is rebuilt under the configuration lock, so a file, snapshot or journal
/// change after the preview returns [`WriteError::PlanStale`] and writes nothing.
/// Per-file write failures are outcomes, not errors; errors are returned only
/// when the journal itself cannot be recorded honestly.
///
/// # Errors
/// As [`plan_version_restore`], plus `PlanStale` when the reviewed plan no longer
/// matches the files or entry.
pub fn apply_version_restore(
    storage: &Storage,
    scope: &HistoryScope,
    entry_key: &str,
    reviewed: &[ReviewedVersionFile],
    backups: &BackupStore,
) -> Result<VersionRestoreOutcome, WriteError> {
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    let plan = build_plan(storage, scope, entry_key, backups)?;
    if plan.reason.is_some()
        || plan
            .files
            .iter()
            .any(|file| file.action == VersionRestoreAction::Blocked)
    {
        return Err(WriteError::PlanStale);
    }
    let planned: Vec<&ResolvedVersionFile> = plan
        .files
        .iter()
        .filter(|file| file.action.changes_files())
        .collect();
    if planned.is_empty() || !review_matches(&planned, reviewed) {
        return Err(WriteError::PlanStale);
    }
    let kind = scope.operation_kind();
    let batch_id = super::operation_id();
    let mut outcomes = Vec::with_capacity(planned.len());
    // Restore the latest mutation first, mirroring how a rollback walks steps.
    for file in planned.iter().rev() {
        let revision_id = file
            .revision_id
            .clone()
            .ok_or(WriteError::InvalidStoredOperation)?;
        let revision = RevisionContext {
            storage,
            operation_id: &plan.operation_id,
            ordinal: file.ordinal,
            batch_id: &batch_id,
            intent: RevisionIntent::Restore,
            direct: true,
        };
        let result = match file.action {
            VersionRestoreAction::Restore => restore_retained(file, backups, kind, &revision),
            VersionRestoreAction::Remove => remove_current(file, backups, &revision),
            VersionRestoreAction::Skip | VersionRestoreAction::Blocked => {
                return Err(WriteError::InvalidStoredOperation);
            }
        };
        let (state, error_code) = match result {
            Ok(()) => (RestoreState::Restored, None),
            Err(error) => outcome_state(error)?,
        };
        outcomes.push(VersionRestoreFileOutcome {
            revision_id,
            target_path: file.target_path.clone(),
            action: file.action,
            state,
            error_code,
        });
    }
    Ok(VersionRestoreOutcome {
        entry_key: plan.entry_key,
        files: outcomes,
    })
}

/// Write one retained version through the shared recorded-write path.
fn restore_retained(
    file: &ResolvedVersionFile,
    backups: &BackupStore,
    kind: OperationKind,
    revision: &RevisionContext<'_>,
) -> Result<(), WriteError> {
    let revision_id = file
        .revision_id
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    let digest = file
        .target_digest
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    let recorded_path = file
        .snapshot_path
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    // Read again at write time; the plan is only an observation.
    let bytes = read_snapshot(
        backups,
        revision_id,
        &file.target_path,
        recorded_path,
        digest,
    )?;
    let target = WriteTarget {
        path: PathBuf::from(&file.target_path),
        expected_digest: file.current_digest.clone(),
    };
    apply_recorded_file(&target, &bytes, backups, private_mode(kind), revision).map(|_| ())
}

/// Remove the current file because the retained state was absence.
fn remove_current(
    file: &ResolvedVersionFile,
    backups: &BackupStore,
    revision: &RevisionContext<'_>,
) -> Result<(), WriteError> {
    let current_digest = file
        .current_digest
        .as_deref()
        .ok_or(WriteError::InvalidStoredOperation)?;
    remove_recorded_file(
        Path::new(&file.target_path),
        current_digest,
        backups,
        revision,
    )
}

/// Whether this Agent's replacements restore the private 0600 mode.
fn private_mode(kind: OperationKind) -> bool {
    // Pi and Grok Build hold credentials in their configuration files; Claude
    // Code keeps the user's existing mode, matching its apply path.
    !matches!(kind, OperationKind::ClaudeCodeSettings)
}

/// Whether the reviewed files are exactly the files the rebuilt plan would change.
fn review_matches(planned: &[&ResolvedVersionFile], reviewed: &[ReviewedVersionFile]) -> bool {
    if planned.len() != reviewed.len() {
        return false;
    }
    let mut seen = Vec::with_capacity(reviewed.len());
    for entry in reviewed {
        if seen.contains(&entry.revision_id) {
            return false;
        }
        seen.push(entry.revision_id.clone());
        let Some(file) = planned
            .iter()
            .find(|file| file.revision_id.as_deref() == Some(entry.revision_id.as_str()))
        else {
            return false;
        };
        if file.current_digest != entry.current_digest {
            return false;
        }
    }
    true
}

/// Map a write failure to a per-file result, or stop when the log is unusable.
fn outcome_state(error: WriteError) -> Result<(RestoreState, Option<String>), WriteError> {
    let code = Some(error.code().to_owned());
    match error {
        WriteError::StorageUnavailable
        | WriteError::LogWriteFailed
        | WriteError::LogReadFailed
        | WriteError::InvalidStoredOperation => Err(error),
        WriteError::TargetChanged => Ok((RestoreState::Conflict, code)),
        WriteError::BackupUnavailable => Ok((RestoreState::Unavailable, code)),
        WriteError::OutcomeUnknown => Ok((RestoreState::Unknown, code)),
        _ => Ok((RestoreState::Failed, code)),
    }
}

/// Plan data with the private snapshot path kept out of the IPC view.
#[derive(Debug)]
struct ResolvedVersionPlan {
    entry_key: String,
    operation_id: String,
    intent: HistoryIntent,
    owner_ref: Option<String>,
    created_at: i64,
    files: Vec<ResolvedVersionFile>,
    reason: Option<String>,
}

impl ResolvedVersionPlan {
    /// Drop private fields and decide whether the plan can be confirmed.
    fn into_view(self) -> VersionRestorePlan {
        let restorable = self.reason.is_none()
            && self
                .files
                .iter()
                .all(|file| file.action != VersionRestoreAction::Blocked)
            && self.files.iter().any(|file| file.action.changes_files());
        VersionRestorePlan {
            entry_key: self.entry_key,
            operation_id: self.operation_id,
            intent: self.intent,
            owner_ref: self.owner_ref,
            created_at: self.created_at,
            files: self.files.iter().map(ResolvedVersionFile::view).collect(),
            restorable,
            reason: self.reason,
        }
    }
}

/// One planned file, including the private snapshot path.
#[derive(Debug, Clone)]
struct ResolvedVersionFile {
    revision_id: Option<String>,
    ordinal: u32,
    target_path: String,
    target_digest: Option<String>,
    snapshot_path: Option<String>,
    current_digest: Option<String>,
    action: VersionRestoreAction,
    reason: Option<String>,
    restored_owner_ref: Option<String>,
}

impl ResolvedVersionFile {
    /// Clone the fields safe to show the interface.
    fn view(&self) -> VersionRestoreFile {
        VersionRestoreFile {
            revision_id: self.revision_id.clone(),
            ordinal: self.ordinal,
            target_path: self.target_path.clone(),
            action: self.action,
            reason: self.reason.clone(),
            current_digest: self.current_digest.clone(),
            target_digest: self.target_digest.clone(),
            restored_owner_ref: self.restored_owner_ref.clone(),
        }
    }
}

/// One file's retained version, before reading the current target.
#[derive(Debug)]
struct PendingVersion {
    revision_id: Option<String>,
    ordinal: u32,
    target_path: String,
    target_digest: Option<String>,
    snapshot_path: Option<String>,
}

/// Build the complete plan, including private paths.
fn build_plan(
    storage: &Storage,
    scope: &HistoryScope,
    entry_key: &str,
    backups: &BackupStore,
) -> Result<ResolvedVersionPlan, WriteError> {
    if !valid_entry_key(entry_key) {
        return Err(WriteError::InvalidRequest);
    }
    let (source, intent, created_at, stored) = read_source(storage, scope, entry_key)?;
    if has_interrupted_revisions_for(storage, &source.id)? {
        return Ok(ResolvedVersionPlan {
            entry_key: entry_key.to_owned(),
            operation_id: source.id,
            intent,
            owner_ref: source.owner_ref,
            created_at,
            files: Vec::new(),
            reason: Some(HISTORY_UNRESOLVED.to_owned()),
        });
    }
    let pending = match entry_key.strip_prefix("operation:") {
        Some(_) => {
            let mut pending = Vec::with_capacity(source.steps.len());
            for step in &source.steps {
                // A revision and its source step must describe the same
                // pre-state; otherwise the journal cannot be trusted.
                match stored
                    .iter()
                    .find(|row| row.revision.ordinal == step.ordinal)
                {
                    Some(row) => {
                        if row.revision.previous_digest != step.expected_digest {
                            return Err(WriteError::InvalidStoredOperation);
                        }
                        pending.push(PendingVersion {
                            revision_id: Some(row.revision.revision_id.clone()),
                            ordinal: step.ordinal,
                            target_path: step.target_path.clone(),
                            target_digest: row.revision.previous_digest.clone(),
                            snapshot_path: row.backup_path.clone(),
                        });
                    }
                    // Old logs and no-change steps have no retained bytes. The
                    // plan can still skip a file that is already at the state,
                    // but a changed file cannot be produced.
                    None => pending.push(PendingVersion {
                        revision_id: None,
                        ordinal: step.ordinal,
                        target_path: step.target_path.clone(),
                        target_digest: step.expected_digest.clone(),
                        snapshot_path: None,
                    }),
                }
            }
            pending
        }
        None => stored
            .into_iter()
            .map(|row| PendingVersion {
                revision_id: Some(row.revision.revision_id),
                ordinal: row.revision.ordinal,
                target_path: row.target_path,
                target_digest: row.revision.previous_digest,
                snapshot_path: row.backup_path,
            })
            .collect(),
    };
    let mut files = Vec::with_capacity(pending.len());
    for file in pending {
        files.push(resolve_file(storage, source.kind, file, backups)?);
    }
    Ok(ResolvedVersionPlan {
        entry_key: entry_key.to_owned(),
        operation_id: source.id,
        intent,
        owner_ref: source.owner_ref,
        created_at,
        files,
        reason: None,
    })
}

/// Read and validate the source operation and its entry revisions.
fn read_source(
    storage: &Storage,
    scope: &HistoryScope,
    entry_key: &str,
) -> Result<
    (
        super::OperationRecord,
        HistoryIntent,
        i64,
        Vec<StoredRevision>,
    ),
    WriteError,
> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    if let Some(operation_id) = entry_key.strip_prefix("operation:") {
        let source =
            read_operation(&connection, operation_id)?.ok_or(WriteError::InvalidRequest)?;
        check_scope(scope, &source)?;
        let (intent, stored) = read_revisions(&connection, &source, None)?;
        let created_at = source.created_at;
        return Ok((source, intent, created_at, stored));
    }
    let batch = entry_key
        .strip_prefix("batch:")
        .filter(|value| valid_revision_id(value))
        .ok_or(WriteError::InvalidRequest)?;
    let (operation_id, created_at): (String, i64) = connection
        .query_row(
            "SELECT operation_id, created_at FROM config_file_revision
             WHERE batch_id = ?1 ORDER BY rowid LIMIT 1",
            [batch],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?
        .ok_or(WriteError::InvalidRequest)?;
    let source = read_operation(&connection, &operation_id)?.ok_or(WriteError::InvalidRequest)?;
    check_scope(scope, &source)?;
    let (intent, stored) = read_revisions(&connection, &source, Some(batch))?;
    Ok((source, intent, created_at, stored))
}

/// Refuse an operation that this Agent/root scope does not own as a whole.
fn check_scope(scope: &HistoryScope, source: &super::OperationRecord) -> Result<(), WriteError> {
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

/// Whether the source operation still has interrupted mutations.
fn has_interrupted_revisions_for(
    storage: &Storage,
    operation_id: &str,
) -> Result<bool, WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    has_interrupted_revisions(&connection, operation_id)
}

/// Classify one file from the current target and verify its retained bytes.
fn resolve_file(
    storage: &Storage,
    kind: OperationKind,
    file: PendingVersion,
    backups: &BackupStore,
) -> Result<ResolvedVersionFile, WriteError> {
    let path = Path::new(&file.target_path);
    let current = match read_target(path) {
        Ok(current) => current,
        Err(error) => return Ok(file.blocked(error.code(), None)),
    };
    if current.symlink {
        return Ok(file.blocked(WriteError::SymlinkRefused.code(), current.content_digest));
    }
    let current_digest = current.content_digest.clone();
    if current_digest == file.target_digest {
        return Ok(file.into_state(VersionRestoreAction::Skip, None, current_digest, None));
    }
    if let Some(digest) = file.target_digest.clone() {
        let (Some(revision_id), Some(snapshot_path)) = (&file.revision_id, &file.snapshot_path)
        else {
            return Ok(file.blocked(SNAPSHOT_UNAVAILABLE, current_digest));
        };
        if let Err(error) = read_snapshot(
            backups,
            revision_id,
            &file.target_path,
            snapshot_path,
            &digest,
        ) {
            return Ok(file.blocked(error.code(), current_digest));
        }
        if current.exists {
            match super::is_read_only(path) {
                Ok(true) => {
                    return Ok(file.blocked(WriteError::TargetReadOnly.code(), current_digest));
                }
                Ok(false) => {}
                Err(error) => return Ok(file.blocked(error.code(), current_digest)),
            }
        }
        let owner = owner_for_digest(storage, kind, &digest)?;
        return Ok(file.into_state(
            VersionRestoreAction::Restore,
            Some(digest),
            current_digest,
            owner,
        ));
    }
    // The retained state is absence, and the current file is present.
    match super::is_read_only(path) {
        Ok(true) => Ok(file.blocked(WriteError::TargetReadOnly.code(), current_digest)),
        Ok(false) => Ok(file.into_state(VersionRestoreAction::Remove, None, current_digest, None)),
        Err(error) => Ok(file.blocked(error.code(), current_digest)),
    }
}

impl PendingVersion {
    /// Build a file result with an explicit outcome.
    fn into_state(
        self,
        action: VersionRestoreAction,
        target_digest: Option<String>,
        current_digest: Option<String>,
        restored_owner_ref: Option<String>,
    ) -> ResolvedVersionFile {
        ResolvedVersionFile {
            revision_id: self.revision_id,
            ordinal: self.ordinal,
            target_path: self.target_path,
            target_digest,
            snapshot_path: self.snapshot_path,
            current_digest,
            action,
            reason: None,
            restored_owner_ref,
        }
    }

    /// Build a blocked file result, keeping the private snapshot path for apply.
    fn blocked(self, reason: &str, current_digest: Option<String>) -> ResolvedVersionFile {
        ResolvedVersionFile {
            revision_id: self.revision_id,
            ordinal: self.ordinal,
            target_path: self.target_path,
            target_digest: self.target_digest,
            snapshot_path: self.snapshot_path,
            current_digest,
            action: VersionRestoreAction::Blocked,
            reason: Some(reason.to_owned()),
            restored_owner_ref: None,
        }
    }
}
