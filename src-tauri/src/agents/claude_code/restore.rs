//! Claude Code restore surface for the detail page (P21).
//!
//! Lists the settings writes vibemate recorded, previews what undoing one would
//! do, and applies the preview the user confirmed. The file content stays in the
//! private backup directory: this layer adds what the interface needs to label a
//! write — the Provider name — and nothing else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::is_digest;
use crate::config_apply::{
    BackupStore, OperationKind, OperationState, RestoreAction, RestoreOutcome, RestorePlan,
    RestoreState, RestoreStepPlan, ReviewedRestoreStep, StepState, WriteError, apply_restore,
    plan_restore, recent_operations,
};
use crate::storage::Storage;

/// How many writes the detail page lists, newest first.
const LISTED_OPERATIONS: u32 = 10;

/// Upper bound on the reviewed tokens one request may carry.
///
/// A Claude Code write covers one file today; the bound keeps a malformed or
/// hostile payload from describing an unbounded number of steps.
const MAX_REVIEWED_STEPS: usize = 16;

/// Why a restore could not be listed, planned or applied.
///
/// The kind check lives in the log reader: this build understands exactly the
/// kinds it can read, and an unknown kind is reported as an invalid row, so a
/// write another feature recorded cannot reach this surface. When a second kind
/// is added, each adapter must filter by its own kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeCodeRestoreError {
    /// No recorded operation has that identity.
    OperationNotFound,
    /// The reviewed preview no longer matches the files on disk.
    PlanStale,
    /// The request cannot be described: a malformed token or too many steps.
    InvalidRequest,
    /// The private database is not open, or its log cannot be read or written.
    StorageUnavailable,
    /// The restore did not complete for a reason the interface cannot act on.
    OperationFailed,
}

impl ClaudeCodeRestoreError {
    /// Stable code for the interface. The variant serializes to the same text; a
    /// test keeps the two in step.
    pub fn code(self) -> &'static str {
        match self {
            Self::OperationNotFound => "operation_not_found",
            Self::PlanStale => "plan_stale",
            Self::InvalidRequest => "invalid_request",
            Self::StorageUnavailable => "storage_unavailable",
            Self::OperationFailed => "operation_failed",
        }
    }
}

impl std::fmt::Display for ClaudeCodeRestoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::OperationNotFound => "No recorded configuration write has that identity.",
            Self::PlanStale => "The configuration changed since the restore preview.",
            Self::InvalidRequest => "The restore request is invalid.",
            Self::StorageUnavailable => "Private storage is unavailable.",
            Self::OperationFailed => "The restore could not be completed.",
        })
    }
}

impl std::error::Error for ClaudeCodeRestoreError {}

/// Map a write-layer failure onto the restore error namespace.
///
/// A file-level refusal is a result, not an error: it travels in the outcome's
/// step states, so only log and request failures reach this mapping.
impl From<WriteError> for ClaudeCodeRestoreError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::PlanStale => Self::PlanStale,
            WriteError::InvalidRequest => Self::InvalidRequest,
            WriteError::StorageUnavailable
            | WriteError::LogWriteFailed
            | WriteError::LogReadFailed
            | WriteError::InvalidStoredOperation => Self::StorageUnavailable,
            _ => Self::OperationFailed,
        }
    }
}

/// One recorded Claude Code settings write, as the detail page lists it.
///
/// Backup paths, copy digests and file content stay in the write layer; the
/// interface needs the time, the state, the Provider and what each step did.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeOperation {
    /// Identity a restore request names.
    pub operation_id: String,
    /// How far the write got.
    pub state: OperationState,
    /// Provider the write was made for, when it recorded one.
    pub provider_id: Option<String>,
    /// Display name of that Provider, when it is still in the library.
    pub provider_label: Option<String>,
    /// Unix milliseconds when the write was recorded.
    pub created_at: i64,
    /// Unix milliseconds of its last change.
    pub updated_at: i64,
    /// Whether the files as they are now leave something to restore.
    pub restorable: bool,
    /// Interrupted mutation metadata must be explicitly reviewed before restoring.
    pub requires_resolution: bool,
    /// One entry per file the write covers.
    pub steps: Vec<ClaudeCodeOperationStep>,
}

/// One file of a recorded write, without backup paths or file content.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeOperationStep {
    /// Position of the step in the write, counted from zero.
    pub ordinal: u32,
    /// Absolute path the write changed.
    pub target_path: String,
    /// What the write did to this file.
    pub state: StepState,
    /// What a rollback did to it, when one was attempted.
    pub restore_state: Option<RestoreState>,
}

/// A restore preview prepared for the detail page.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeRestorePlan {
    /// Identity of the write that would be undone.
    pub operation_id: String,
    /// How far the write got.
    pub state: OperationState,
    /// Unix milliseconds when the write was recorded.
    pub created_at: i64,
    /// Whether at least one step would change a file.
    pub restorable: bool,
    /// Provider the write was made for, when it recorded one.
    pub provider_id: Option<String>,
    /// Display name of that Provider, when it is still in the library.
    pub provider_label: Option<String>,
    /// One entry per recorded step, in the order they were applied.
    pub steps: Vec<ClaudeCodeRestoreStep>,
}

/// One file of a restore preview.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeRestoreStep {
    /// Position of the step in the write, counted from zero.
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
    /// What an earlier rollback attempt recorded, when there was one.
    pub restore_state: Option<RestoreState>,
    /// Provider whose key the restored content carries, when the log can tell.
    pub restored_provider_id: Option<String>,
    /// Display name of that Provider, when it is still in the library.
    pub restored_provider_label: Option<String>,
}

/// One restore request: the write to undo, and the tokens the user reviewed.
///
/// The exact key set is enforced on the way in: a payload this build does not
/// understand is refused instead of being applied with missing tokens.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyClaudeCodeRestoreRequest {
    /// Recorded write the preview described.
    pub operation_id: String,
    /// Steps exactly as the preview showed them.
    pub reviewed: Vec<ReviewedRestoreStepRequest>,
}

/// One reviewed step, as the preview showed it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedRestoreStepRequest {
    /// Position of the step in the write, counted from zero.
    pub ordinal: u32,
    /// SHA-256 the preview showed for the file at that moment.
    pub current_digest: Option<String>,
}

impl ApplyClaudeCodeRestoreRequest {
    /// Convert the reviewed steps into the write layer's tokens.
    ///
    /// A malformed digest, a repeated ordinal or an oversized list is a request
    /// error, not a stale plan: the payload itself is wrong, so it is refused
    /// before any file is read.
    pub fn reviewed_steps(&self) -> Result<Vec<ReviewedRestoreStep>, ClaudeCodeRestoreError> {
        if self.reviewed.len() > MAX_REVIEWED_STEPS {
            return Err(ClaudeCodeRestoreError::InvalidRequest);
        }
        let mut ordinals = Vec::with_capacity(self.reviewed.len());
        let mut reviewed = Vec::with_capacity(self.reviewed.len());
        for step in &self.reviewed {
            if ordinals.contains(&step.ordinal) {
                return Err(ClaudeCodeRestoreError::InvalidRequest);
            }
            ordinals.push(step.ordinal);
            if step
                .current_digest
                .as_deref()
                .is_some_and(|digest| !is_digest(digest))
            {
                return Err(ClaudeCodeRestoreError::InvalidRequest);
            }
            reviewed.push(ReviewedRestoreStep {
                ordinal: step.ordinal,
                current_digest: step.current_digest.clone(),
            });
        }
        Ok(reviewed)
    }
}

/// List the Claude Code settings writes vibemate recorded, newest first.
///
/// Read-only: the listing carries paths, states and digests, never file content,
/// and each row says whether the files as they are now leave something to
/// restore.
///
/// # Errors
/// Returns `StorageUnavailable` when the database or its log cannot be read, and
/// `OperationFailed` when a listed row cannot be planned.
pub fn list_claude_code_operations(
    storage: &Storage,
) -> Result<Vec<ClaudeCodeOperation>, ClaudeCodeRestoreError> {
    list_config_operations(storage, OperationKind::ClaudeCodeSettings)
}

/// List only writes owned by one adapter. No file contents are returned.
/// Returns safe storage/restore errors when the recorded state cannot be read.
pub fn list_config_operations(
    storage: &Storage,
    kind: OperationKind,
) -> Result<Vec<ClaudeCodeOperation>, ClaudeCodeRestoreError> {
    let records = recent_operations(storage, kind, LISTED_OPERATIONS)?;
    let mut listed = Vec::with_capacity(records.len());
    for record in records {
        // The listing says whether a restore is worth offering; whether a
        // specific step is a conflict is answered again by the preview.
        let requires_resolution =
            crate::config_apply::operation_requires_resolution(storage, &record.id)?;
        let restorable = if requires_resolution {
            false
        } else {
            plan_restore(storage, &record.id)?
                .ok_or(ClaudeCodeRestoreError::OperationFailed)?
                .restorable
        };
        listed.push(ClaudeCodeOperation {
            operation_id: record.id,
            state: record.state,
            provider_id: record.owner_ref.clone(),
            provider_label: record
                .owner_ref
                .as_deref()
                .and_then(|provider_id| provider_label(storage, provider_id)),
            created_at: record.created_at,
            updated_at: record.updated_at,
            restorable,
            requires_resolution,
            steps: record
                .steps
                .iter()
                .map(|step| ClaudeCodeOperationStep {
                    ordinal: step.ordinal,
                    target_path: step.target_path.clone(),
                    state: step.state,
                    restore_state: step.restore_state,
                })
                .collect(),
        });
    }
    Ok(listed)
}

/// Preview what undoing one recorded write would do, without writing anything.
///
/// # Errors
/// Returns `OperationNotFound` for an identity no row has, `StorageUnavailable`
/// when the log cannot be read, and `OperationFailed` when a listed row cannot be
/// planned.
pub fn plan_claude_code_restore(
    storage: &Storage,
    operation_id: &str,
) -> Result<ClaudeCodeRestorePlan, ClaudeCodeRestoreError> {
    plan_config_restore(storage, operation_id, OperationKind::ClaudeCodeSettings)
}

/// Preview only a write owned by this adapter; IDs from another adapter are refused.
/// Returns operation-not-found before reading another adapter's target files.
pub fn plan_config_restore(
    storage: &Storage,
    operation_id: &str,
    kind: OperationKind,
) -> Result<ClaudeCodeRestorePlan, ClaudeCodeRestoreError> {
    require_kind(storage, operation_id, kind)?;
    let plan =
        plan_restore(storage, operation_id)?.ok_or(ClaudeCodeRestoreError::OperationNotFound)?;
    Ok(restore_view(storage, plan))
}

/// Apply the restore the user confirmed from that preview.
///
/// The write layer rebuilds the plan and compares every reviewed token, so an
/// edit made after the preview is refused with `PlanStale` before any file is
/// touched. The key never enters this call: the previous content comes from the
/// private copy, and the restored file carries whichever key it held before.
///
/// # Errors
/// As [`plan_claude_code_restore`], plus `PlanStale` for a stale approval and the
/// log errors of the write layer.
pub fn apply_claude_code_restore(
    storage: &Storage,
    request: &ApplyClaudeCodeRestoreRequest,
    backups_root: &Path,
) -> Result<RestoreOutcome, ClaudeCodeRestoreError> {
    apply_config_restore(
        storage,
        request,
        backups_root,
        OperationKind::ClaudeCodeSettings,
    )
}

/// Restore only this adapter's recorded files after checking the reviewed tokens.
/// Returns operation-not-found for other adapters; never reads OS credentials.
pub fn apply_config_restore(
    storage: &Storage,
    request: &ApplyClaudeCodeRestoreRequest,
    backups_root: &Path,
    kind: OperationKind,
) -> Result<RestoreOutcome, ClaudeCodeRestoreError> {
    let reviewed = request.reviewed_steps()?;
    require_kind(storage, &request.operation_id, kind)?;
    let backups = BackupStore::new(PathBuf::from(backups_root));
    apply_restore(storage, &request.operation_id, &reviewed, &backups).map_err(Into::into)
}

/// Check ownership before the shared restore layer reads any target file.
fn require_kind(
    storage: &Storage,
    id: &str,
    kind: OperationKind,
) -> Result<(), ClaudeCodeRestoreError> {
    match crate::config_apply::operation(storage, id)? {
        Some(record) if record.kind == kind => Ok(()),
        _ => Err(ClaudeCodeRestoreError::OperationNotFound),
    }
}

/// Name the Provider a reference points at, when that Provider still exists.
fn provider_label(storage: &Storage, provider_id: &str) -> Option<String> {
    crate::providers::get_provider(storage, provider_id)
        .ok()
        .map(|record| record.display_name)
}

/// Turn the shared plan into the view the detail page shows.
fn restore_view(storage: &Storage, plan: RestorePlan) -> ClaudeCodeRestorePlan {
    ClaudeCodeRestorePlan {
        operation_id: plan.operation_id,
        state: plan.state,
        created_at: plan.created_at,
        restorable: plan.restorable,
        provider_id: plan.owner_ref.clone(),
        provider_label: plan
            .owner_ref
            .as_deref()
            .and_then(|provider_id| provider_label(storage, provider_id)),
        steps: plan
            .steps
            .into_iter()
            .map(|step| restore_step_view(storage, step))
            .collect(),
    }
}

/// Turn one shared step into the view the detail page shows, naming the owner of
/// the content a restore would put back.
fn restore_step_view(storage: &Storage, step: RestoreStepPlan) -> ClaudeCodeRestoreStep {
    ClaudeCodeRestoreStep {
        ordinal: step.ordinal,
        target_path: step.target_path,
        action: step.action,
        reason: step.reason,
        current_digest: step.current_digest,
        applied_digest: step.applied_digest,
        restore_state: step.restore_state,
        restored_provider_id: step.restored_owner_ref.clone(),
        restored_provider_label: step
            .restored_owner_ref
            .as_deref()
            .and_then(|provider_id| provider_label(storage, provider_id)),
    }
}
