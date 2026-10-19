//! Read-only observations of interrupted mutations, not recovery approvals.
//!
//! An observed digest cannot prove which process wrote a file or that its mode
//! was updated. This module never settles a revision or unlocks legacy recovery.
//! The caller must separately approve and revalidate any later recovery action.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use super::{
    BackupStore, OperationRecord, WriteError, operations::read_operation, path_text, read_target,
};
use crate::storage::Storage;

/// Relationship of the current bytes (or absence) to an interrupted mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionMatch {
    /// Only the pre-mutation state matches; this does not prove no write occurred.
    Previous,
    /// Only the intended state matches; this does not prove this app wrote it.
    Intended,
    /// Both match, for example a mutation that only changed file permissions.
    Both,
    /// Neither matches, including unexpected creation or removal.
    Diverged,
    /// The file could not be safely read; the reason is reported separately.
    Unreadable,
}

/// Whether the independent pre-mutation copy can still reproduce its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionBackupState {
    /// The pre-mutation state was absence; no copy is needed.
    NotNeeded,
    /// A regular private-store file matches the recorded digest.
    Available,
    /// The copy is missing, unreadable, oversized, linked or damaged.
    Unavailable,
}

/// Safe metadata about one interrupted mutation; never contains file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionInspection {
    /// Identity of the independent pre-mutation snapshot.
    pub id: String,
    /// Apply, restore or compensation attempt grouping its file mutations.
    pub batch_id: String,
    /// Position in the source operation, not in this inspection result.
    pub ordinal: u32,
    /// Recorded direction: `apply`, `restore`, `compensation`, or
    /// `version_restore` for a direct H2 restore stored with `direct = 1`.
    pub intent: String,
    /// `prepared`, `unknown`, or `completed` with a missing legacy step result.
    pub state: String,
    /// Original target, verified against the source operation's step.
    pub target_path: String,
    /// Digest before the mutation; null means the file did not exist.
    pub previous_digest: Option<String>,
    /// Intended digest; null means the mutation intended to remove the file.
    pub intended_digest: Option<String>,
    /// Observed digest, absent for a missing or unreadable target; see `current_match`.
    pub current_digest: Option<String>,
    /// How the observation relates to the recorded states.
    pub current_match: RevisionMatch,
    /// Stable read failure, present only for an unreadable observation.
    pub error_code: Option<String>,
    /// Availability of the independent pre-mutation bytes, separate from the target.
    pub backup_state: RevisionBackupState,
}

/// Check whether a recorded operation needs explicit observation before recovery.
///
/// Reads journal metadata only, not target files or backups. A history listing
/// uses this to withhold the legacy restore action until reconciliation.
///
/// # Errors
/// Returns `InvalidRequest` for a malformed or absent identity and storage/log
/// errors for unreadable or malformed source records.
pub fn operation_requires_resolution(
    storage: &Storage,
    operation_id: &str,
) -> Result<bool, WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    read_operation(&connection, operation_id)?.ok_or(WriteError::InvalidRequest)?;
    has_interrupted_revisions(&connection, operation_id)
}

/// Inspect unresolved mutations belonging to one recorded operation.
///
/// Returns `None` for a missing operation and an empty list for an operation
/// without interrupted revisions. Also includes the latest completed revision
/// of a step whose legacy result was not recorded. Results are oldest first. Reads files
/// but never changes them, their backups, or any log state. The supplied store
/// must be the application's private store, not a path supplied over IPC.
/// Each result is only a point-in-time observation, not a write authorization.
///
/// # Errors
/// Returns `InvalidRequest` for a malformed operation identity,
/// `InvalidStoredOperation` for malformed or mismatched revision metadata, and
/// storage/log errors if records cannot be read. Individual file read failures
/// are reported in the result so one unreadable file does not hide other steps.
pub fn inspect_interrupted_revisions(
    storage: &Storage,
    operation_id: &str,
    backups: &BackupStore,
) -> Result<Option<Vec<RevisionInspection>>, WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let Some(source) = read_operation(&connection, operation_id)? else {
        return Ok(None);
    };
    Ok(Some(inspect_record(&connection, &source, backups)?))
}

/// Reuse the caller's connection so resolution can inspect inside its write transaction.
pub(super) fn inspect_record(
    connection: &Connection,
    source: &OperationRecord,
    backups: &BackupStore,
) -> Result<Vec<RevisionInspection>, WriteError> {
    let rows = read_pending(connection, &source.id)?;
    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let step = source.steps.iter().find(|step| step.ordinal == row.ordinal);
        if step.is_none_or(|step| step.target_path != row.target_path) {
            return Err(WriteError::InvalidStoredOperation);
        }
        row.validate()?;
        let backup_state = inspect_backup(&row, backups);
        let (current_digest, current_match, error_code) =
            match read_target(Path::new(&row.target_path)) {
                Ok(target) if !target.symlink => {
                    let previous = target.content_digest == row.previous_digest;
                    let intended = target.content_digest == row.intended_digest;
                    let matched = match (previous, intended) {
                        (true, true) => RevisionMatch::Both,
                        (true, false) => RevisionMatch::Previous,
                        (false, true) => RevisionMatch::Intended,
                        (false, false) => RevisionMatch::Diverged,
                    };
                    (target.content_digest, matched, None)
                }
                result => {
                    let error = match result {
                        Ok(_) => WriteError::SymlinkRefused,
                        Err(error) => error,
                    };
                    (
                        None,
                        RevisionMatch::Unreadable,
                        Some(error.code().to_owned()),
                    )
                }
            };
        results.push(RevisionInspection {
            id: row.id,
            batch_id: row.batch_id,
            ordinal: row.ordinal,
            intent: if row.direct == 1 {
                "version_restore".to_owned()
            } else {
                row.intent
            },
            state: row.state,
            target_path: row.target_path,
            previous_digest: row.previous_digest,
            intended_digest: row.intended_digest,
            current_digest,
            current_match,
            error_code,
            backup_state,
        });
    }
    Ok(results)
}

struct PendingRevision {
    id: String,
    batch_id: String,
    ordinal: u32,
    intent: String,
    state: String,
    target_path: String,
    previous_digest: Option<String>,
    intended_digest: Option<String>,
    backup_path: Option<String>,
    error_code: Option<String>,
    created_at: i64,
    updated_at: i64,
    direct: i64,
}

impl PendingRevision {
    fn validate(&self) -> Result<(), WriteError> {
        let identity = |value: &str| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        };
        let digest = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        // Identifiers become directory names on every supported platform. Reject
        // drive prefixes and traversal, not just this platform's separator.
        if !identity(&self.id)
            || !identity(&self.batch_id)
            || !matches!(self.intent.as_str(), "apply" | "restore" | "compensation")
            || !matches!(self.direct, 0 | 1)
            || (self.direct == 1 && self.intent != "restore")
            || !matches!(self.state.as_str(), "prepared" | "unknown" | "completed")
            || (self.state != "unknown" && self.error_code.is_some())
            || (self.state == "unknown" && self.error_code.as_deref() != Some("outcome_unknown"))
            || self.created_at < 0
            || self.updated_at < self.created_at
            || path_text(Path::new(&self.target_path)).is_err()
            || self
                .previous_digest
                .as_deref()
                .is_some_and(|value| !digest(value))
            || self
                .intended_digest
                .as_deref()
                .is_some_and(|value| !digest(value))
            || self.previous_digest.is_some() != self.backup_path.is_some()
            || self
                .backup_path
                .as_deref()
                .is_some_and(|path| path_text(Path::new(path)).is_err())
        {
            return Err(WriteError::InvalidStoredOperation);
        }
        Ok(())
    }
}

// A completed file result can precede a crash in legacy-step recording. Only
// the latest mutation of a step can repair that legacy result; older restore
// attempts must not override a newer attempt. Uncertain rows remain visible
// regardless of age, and resolution refuses them if a newer mutation exists.
const INTERRUPTED_PREDICATE: &str = "r.operation_id = ?1 AND (
    r.state IN ('prepared', 'unknown') OR (
        r.state = 'completed' AND r.direct = 0
        AND NOT EXISTS (SELECT 1 FROM config_file_revision newer
            WHERE newer.operation_id = r.operation_id AND newer.ordinal = r.ordinal
                AND newer.rowid > r.rowid)
        AND EXISTS (SELECT 1 FROM config_operation_step s
            WHERE s.operation_id = r.operation_id AND s.ordinal = r.ordinal AND (
                (r.intent = 'apply' AND s.state = 'pending') OR
                (r.intent IN ('restore', 'compensation') AND s.state = 'applied'
                    AND (s.restore_state IS NULL OR s.restore_state != 'restored'))
            ))
    ))";

/// Shared with the legacy-recovery guard: both crash windows must block replay.
pub(super) fn has_interrupted_revisions(
    connection: &Connection,
    operation_id: &str,
) -> Result<bool, WriteError> {
    connection
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM config_file_revision r WHERE {INTERRUPTED_PREDICATE})"
            ),
            [operation_id],
            |row| row.get(0),
        )
        .map_err(|_| WriteError::LogReadFailed)
}

fn read_pending(
    connection: &Connection,
    operation_id: &str,
) -> Result<Vec<PendingRevision>, WriteError> {
    let mut query = connection
        .prepare(&format!(
            "SELECT r.id, r.batch_id, r.ordinal, r.intent, r.state, r.target_path,
             r.previous_digest, r.next_digest, r.backup_path, r.error_code, r.created_at,
             r.updated_at, r.direct
             FROM config_file_revision r WHERE {INTERRUPTED_PREDICATE} ORDER BY r.created_at, r.id"
        ))
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map([operation_id], |row| {
            Ok(PendingRevision {
                id: row.get(0)?,
                batch_id: row.get(1)?,
                ordinal: row.get(2)?,
                intent: row.get(3)?,
                state: row.get(4)?,
                target_path: row.get(5)?,
                previous_digest: row.get(6)?,
                intended_digest: row.get(7)?,
                backup_path: row.get(8)?,
                error_code: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
                direct: row.get(12)?,
            })
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| WriteError::InvalidStoredOperation)
}

fn inspect_backup(row: &PendingRevision, backups: &BackupStore) -> RevisionBackupState {
    recorded_backup_state(
        backups,
        &row.id,
        &row.target_path,
        row.backup_path.as_deref(),
        row.previous_digest.as_deref(),
    )
}

/// Whether a recorded private copy can still reproduce its bytes.
///
/// Absence before the mutation needs no copy (`NotNeeded`). A retained path
/// that fails the shared layout/digest check is `Unavailable`. Never returns
/// paths or file bytes. History listings reuse this so a journal reference is
/// never reported as available without opening the private store.
pub(super) fn recorded_backup_state(
    backups: &BackupStore,
    revision_id: &str,
    target_path: &str,
    recorded_path: Option<&str>,
    expected_digest: Option<&str>,
) -> RevisionBackupState {
    let (Some(path), Some(digest)) = (recorded_path, expected_digest) else {
        return if recorded_path.is_none() {
            RevisionBackupState::NotNeeded
        } else {
            RevisionBackupState::Unavailable
        };
    };
    // The shared reader refuses paths that do not match the private-store
    // layout, links, oversized or unreadable files, and digest mismatches.
    match super::snapshots::read_snapshot(backups, revision_id, target_path, path, digest) {
        Ok(_) => RevisionBackupState::Available,
        Err(_) => RevisionBackupState::Unavailable,
    }
}
