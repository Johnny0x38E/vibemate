//! Explicit, metadata-only resolution of interrupted file mutations.
//!
//! Approval binds the whole operation and its interrupted revisions. A SQLite
//! write transaction keeps the audit, revision result and legacy-step repair
//! together. No configuration is written here, and a digest observation is not
//! treated as proof of who performed the original write.

use rusqlite::{Connection, TransactionBehavior, params};
use serde::Serialize;

use super::{
    Backup, BackupStore, OperationRecord, OperationState, RestoreState, RevisionBackupState,
    RevisionInspection, RevisionMatch, StepRecord, StepState, WriteError, digest_hex, file_name,
    inspection::inspect_record, operations::read_operation, path_text,
};
use crate::{shared, storage::Storage};

/// Reviewed observations and the exact source metadata they apply to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResolutionPlan {
    /// Original operation, already scoped to an Agent by the caller.
    pub operation_id: String,
    /// Digest of the complete operation metadata, including every legacy step.
    pub operation_digest: String,
    /// All interrupted revisions in inspection order; a subset cannot be approved.
    pub revisions: Vec<RevisionInspection>,
}

/// Prepare a metadata-only resolution approval without changing files or logs.
///
/// Returns `None` for a missing operation. A plan can contain blocked observations
/// so the eventual interface can explain why resolution is unavailable. An empty
/// plan has nothing to resolve. This is not approval to restore a configuration.
///
/// # Errors
/// Returns the storage, identity and malformed-log errors of
/// [`super::inspect_interrupted_revisions`].
pub fn plan_revision_resolution(
    storage: &Storage,
    operation_id: &str,
    backups: &BackupStore,
) -> Result<Option<RevisionResolutionPlan>, WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let Some(record) = read_operation(&connection, operation_id)? else {
        return Ok(None);
    };
    Ok(Some(build_plan(&connection, &record, backups)?))
}

/// Resolve a reviewed set of observations and repair its legacy results atomically.
///
/// Only unambiguous pre-state or intended-state matches with intact independent
/// snapshots are accepted. Completed revisions with a missing legacy result must
/// still match the intended state. Each revision must be its step's latest
/// mutation. The audit explicitly records an observation, not historical proof.
/// Configuration files, backups, credentials and Provider bindings are untouched;
/// callers must not infer runtime effectiveness from the returned operation state.
///
/// The transaction excludes other log writers, not external filesystem writers.
/// Files are rechecked before recording; a subsequent restore must still perform
/// its normal preview and concurrent-change checks. No compensation runs here.
///
/// # Errors
/// Returns `PlanStale` for changed or repeated approvals, `OutcomeUnknown` for
/// ambiguous/unreadable/diverged observations or a superseded mutation,
/// `BackupUnavailable` for a missing/damaged snapshot, `InvalidStoredOperation`
/// for inconsistent step relationships, and storage/log errors on transaction
/// failure. All log updates roll back together on any error.
pub fn resolve_interrupted_revisions(
    storage: &Storage,
    operation_id: &str,
    reviewed: &RevisionResolutionPlan,
    backups: &BackupStore,
) -> Result<OperationState, WriteError> {
    let _mutation_lock = storage
        .lock_configuration()
        .map_err(|_| WriteError::StorageUnavailable)?;
    let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| WriteError::LogWriteFailed)?;
    let record = read_operation(&transaction, operation_id)?.ok_or(WriteError::PlanStale)?;
    let current = build_plan(&transaction, &record, backups)?;
    if &current != reviewed || current.revisions.is_empty() {
        return Err(WriteError::PlanStale);
    }
    for revision in &current.revisions {
        validate_resolution(&transaction, &record, revision)?;
    }
    // Recheck after validation and backup reads. SQLite cannot lock an external
    // editor; this observation grants no permission for a later blind write.
    if build_plan(&transaction, &record, backups)? != current {
        return Err(WriteError::PlanStale);
    }
    let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
    for revision in &current.revisions {
        repair_revision(&transaction, operation_id, revision, backups, now)?;
    }
    let repaired =
        read_operation(&transaction, operation_id)?.ok_or(WriteError::InvalidStoredOperation)?;
    let state = resolved_operation_state(&repaired);
    transaction.execute(
        "UPDATE config_operation SET state = ?2, updated_at = max(updated_at, ?3) WHERE id = ?1",
        params![operation_id, state.as_str(), now],
    ).map_err(|_| WriteError::LogWriteFailed)?;
    transaction
        .commit()
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(state)
}

fn build_plan(
    connection: &Connection,
    record: &OperationRecord,
    backups: &BackupStore,
) -> Result<RevisionResolutionPlan, WriteError> {
    let metadata = serde_json::to_vec(record).map_err(|_| WriteError::InvalidStoredOperation)?;
    Ok(RevisionResolutionPlan {
        operation_id: record.id.clone(),
        operation_digest: digest_hex(&metadata),
        revisions: inspect_record(connection, record, backups)?,
    })
}

fn validate_resolution(
    connection: &Connection,
    record: &OperationRecord,
    revision: &RevisionInspection,
) -> Result<(), WriteError> {
    if !matches!(
        revision.current_match,
        RevisionMatch::Previous | RevisionMatch::Intended
    ) || (revision.state == "completed" && revision.current_match != RevisionMatch::Intended)
    {
        return Err(WriteError::OutcomeUnknown);
    }
    if revision.backup_state == RevisionBackupState::Unavailable {
        return Err(WriteError::BackupUnavailable);
    }
    let latest: String = connection
        .query_row(
            "SELECT id FROM config_file_revision WHERE operation_id = ?1 AND ordinal = ?2
         ORDER BY rowid DESC LIMIT 1",
            params![record.id, revision.ordinal],
            |row| row.get(0),
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    if latest != revision.id {
        return Err(WriteError::OutcomeUnknown);
    }
    let step = record
        .steps
        .iter()
        .find(|step| step.ordinal == revision.ordinal)
        .ok_or(WriteError::InvalidStoredOperation)?;
    let valid = if revision.intent == "apply" {
        step.expected_digest == revision.previous_digest
            && revision.intended_digest.is_some()
            && step.restore_state.is_none()
            && (step.state == StepState::Pending
                || (step.state == StepState::Applied
                    && step.applied_digest == revision.intended_digest
                    && step.previous_existed == Some(revision.previous_digest.is_some())
                    && step.backup.as_ref().map(|copy| copy.id.as_str())
                        == revision
                            .previous_digest
                            .as_ref()
                            .map(|_| revision.id.as_str())))
    } else {
        step.state == StepState::Applied
            && step.applied_digest == revision.previous_digest
            && restore_destination_matches(step, revision)
    };
    if !valid {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(())
}

fn restore_destination_matches(step: &StepRecord, revision: &RevisionInspection) -> bool {
    match &step.backup {
        Some(copy) => revision.intended_digest.as_deref() == Some(copy.content_digest.as_str()),
        None => step.previous_existed == Some(false) && revision.intended_digest.is_none(),
    }
}

fn repair_revision(
    connection: &Connection,
    operation_id: &str,
    revision: &RevisionInspection,
    backups: &BackupStore,
    now: i64,
) -> Result<(), WriteError> {
    let intended = revision.current_match == RevisionMatch::Intended;
    connection
        .execute(
            "INSERT INTO config_revision_resolution
         (revision_id, previous_state, observed_digest, matched_state, resolved_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                revision.id,
                revision.state,
                revision.current_digest,
                if intended { "intended" } else { "previous" },
                now
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    let updated = connection
        .execute(
            "UPDATE config_file_revision SET state = ?2, error_code = ?3,
         updated_at = max(updated_at, ?4) WHERE id = ?1 AND state = ?5",
            params![
                revision.id,
                if intended { "completed" } else { "failed" },
                if intended {
                    None
                } else {
                    Some(WriteError::WriteFailed.code())
                },
                now,
                revision.state
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated != 1 {
        return Err(WriteError::LogWriteFailed);
    }
    if revision.intent == "apply" {
        repair_apply(connection, operation_id, revision, backups, intended, now)?;
    } else {
        connection.execute(
            "UPDATE config_operation_step SET restore_state = ?3, updated_at = max(updated_at, ?4)
             WHERE operation_id = ?1 AND ordinal = ?2",
            params![operation_id, revision.ordinal,
                if intended { RestoreState::Restored } else { RestoreState::Failed }.as_str(), now],
        ).map_err(|_| WriteError::LogWriteFailed)?;
    }
    Ok(())
}

fn repair_apply(
    connection: &Connection,
    operation_id: &str,
    revision: &RevisionInspection,
    backups: &BackupStore,
    intended: bool,
    now: i64,
) -> Result<(), WriteError> {
    let copy = if intended {
        revision
            .previous_digest
            .as_ref()
            .map(|digest| {
                let path = backups
                    .root()
                    .join(&revision.id)
                    .join(file_name(std::path::Path::new(&revision.target_path)));
                Ok(Backup {
                    id: revision.id.clone(),
                    path: path_text(&path)?,
                    content_digest: digest.clone(),
                })
            })
            .transpose()?
    } else {
        None
    };
    connection
        .execute(
            "UPDATE config_operation_step SET state = ?3, error_code = ?4, applied_digest = ?5,
         previous_existed = ?6, backup_id = ?7, backup_path = ?8, backup_digest = ?9,
         updated_at = max(updated_at, ?10)
         WHERE operation_id = ?1 AND ordinal = ?2",
            params![
                operation_id,
                revision.ordinal,
                if intended {
                    StepState::Applied
                } else {
                    StepState::Failed
                }
                .as_str(),
                if intended {
                    None
                } else {
                    Some(WriteError::WriteFailed.code())
                },
                if intended {
                    revision.intended_digest.as_deref()
                } else {
                    None
                },
                intended.then_some(revision.previous_digest.is_some()),
                copy.as_ref().map(|copy| &copy.id),
                copy.as_ref().map(|copy| &copy.path),
                copy.as_ref().map(|copy| &copy.content_digest),
                now
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(())
}

fn resolved_operation_state(record: &OperationRecord) -> OperationState {
    // Once any restore was attempted, use the same aggregation as recovery.
    // In particular, a completed restore must not become another undo request.
    if record.steps.iter().any(|step| step.restore_state.is_some()) {
        return super::restore::settle(record, &[]);
    }
    if record
        .steps
        .iter()
        .all(|step| step.state == StepState::Applied)
    {
        return OperationState::Applied;
    }
    if record
        .steps
        .iter()
        .any(|step| step.state == StepState::Applied)
    {
        OperationState::Partial
    } else {
        OperationState::Failed
    }
}
