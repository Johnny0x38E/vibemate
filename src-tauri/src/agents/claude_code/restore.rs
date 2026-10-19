//! Claude Code restore surface for the detail page (P21).
//!
//! Previews what undoing one recorded write would do and applies the preview the
//! user confirmed. The history list itself is adapter-scoped and paginated in
//! `agents/history.rs`; this layer adds what the interface needs to label a
//! preview — the Provider name — and nothing else. The file content stays in the
//! private backup directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::is_digest;
use crate::config_apply::{
    BackupStore, OperationKind, OperationState, RestoreAction, RestoreOutcome, RestorePlan,
    RestoreState, RestoreStepPlan, ReviewedRestoreStep, WriteError, apply_restore, plan_restore,
};
use crate::storage::Storage;

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
    /// The history entry belongs to an unfinished recorded write.
    HistoryActive,
    /// Interrupted mutations must be inspected before deletion.
    HistoryUnresolved,
    /// Other retained history still depends on this entry.
    HistoryReferenced,
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
            Self::HistoryActive => "history_active",
            Self::HistoryUnresolved => "history_unresolved",
            Self::HistoryReferenced => "history_referenced",
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
            Self::HistoryActive => "The history entry belongs to an unfinished write.",
            Self::HistoryUnresolved => {
                "Interrupted history must be resolved before this entry can be deleted."
            }
            Self::HistoryReferenced => "Other retained history still depends on this entry.",
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
            WriteError::HistoryActive => Self::HistoryActive,
            WriteError::HistoryUnresolved => Self::HistoryUnresolved,
            WriteError::HistoryReferenced => Self::HistoryReferenced,
            WriteError::StorageUnavailable
            | WriteError::LogWriteFailed
            | WriteError::LogReadFailed
            | WriteError::InvalidStoredOperation => Self::StorageUnavailable,
            _ => Self::OperationFailed,
        }
    }
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
