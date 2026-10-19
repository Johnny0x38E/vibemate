//! Durable pre-mutation snapshots, separate from the legacy compensation queue.
//!
//! Every recorded mutation commits its backup reference before touching a target.
//! A crash leaves `prepared`, not an operation that an old recovery routine could
//! reverse automatically. The original operation/step and the batch identify why
//! the mutation happened; only paths and digests enter SQLite, never file bytes.

use rusqlite::params;

use super::{Backup, WriteError, WriteTarget, path_text};
use crate::{shared, storage::Storage};

/// Why a target is being changed; recovery must not conflate these directions.
#[derive(Clone, Copy)]
pub(super) enum RevisionIntent {
    /// A feature is applying a reviewed configuration plan.
    Apply,
    /// The user explicitly approved restoring an earlier version.
    Restore,
    /// A failed operation is undoing its own writes.
    Compensation,
}

impl RevisionIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
            Self::Compensation => "compensation",
        }
    }
}

/// Source and batch of one actual file mutation, not a new compensation job.
pub(super) struct RevisionContext<'a> {
    /// Private database used to commit the snapshot reference.
    pub storage: &'a Storage,
    /// Original configuration operation whose step owns this target.
    pub operation_id: &'a str,
    /// File's ordinal in that original operation.
    pub ordinal: u32,
    /// One apply, restore or compensation attempt, possibly over several files.
    pub batch_id: &'a str,
    /// Direction of this mutation, independent of the source operation kind.
    pub intent: RevisionIntent,
    /// A direct version restore (H2), not the legacy rollback of the source
    /// operation. Such a row must not update the source step's restore result.
    pub direct: bool,
}

/// Do not let legacy recovery reinterpret an interrupted mutation as an undo
/// request. Explicit resolution must inspect these targets and repair the old
/// step results, including completed revisions whose legacy result is missing.
///
/// Returns `OutcomeUnknown` for an unresolved revision and storage/log errors
/// when the query cannot be trusted. Does not modify files or the log.
pub(super) fn require_known_revisions(
    storage: &Storage,
    operation_id: &str,
) -> Result<(), WriteError> {
    let connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let unresolved = super::inspection::has_interrupted_revisions(&connection, operation_id)?;
    if unresolved {
        return Err(WriteError::OutcomeUnknown);
    }
    Ok(())
}

impl RevisionContext<'_> {
    /// Commit the pre-image reference before replacement or removal. Absence is
    /// represented by a null digest and no backup, not by an empty document.
    /// Returns a target mismatch, invalid source, or storage/log error without
    /// changing the target. The caller has already flushed the private backup.
    pub(super) fn prepare(
        &self,
        id: &str,
        target: &WriteTarget,
        backup: Option<&Backup>,
        next_digest: Option<&str>,
    ) -> Result<(), WriteError> {
        if target.expected_digest.as_deref() != backup.map(|copy| copy.content_digest.as_str()) {
            return Err(WriteError::TargetChanged);
        }
        let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
        let path = path_text(&target.path)?;
        let connection = self
            .storage
            .lock()
            .map_err(|_| WriteError::StorageUnavailable)?;
        // INSERT ... SELECT binds the snapshot to a real step and its exact
        // target. A mismatched source cannot create a seemingly valid version.
        let inserted = connection
            .execute(
                "INSERT INTO config_file_revision
                (id, batch_id, operation_id, ordinal, intent, target_path,
                 previous_digest, next_digest, backup_path, state, created_at, updated_at, direct)
             SELECT ?1, ?2, operation_id, ordinal, ?5, target_path,
                    ?7, ?8, ?9, 'prepared', ?10, ?10, ?11
             FROM config_operation_step
             WHERE operation_id = ?3 AND ordinal = ?4 AND target_path = ?6",
                params![
                    id,
                    self.batch_id,
                    self.operation_id,
                    i64::from(self.ordinal),
                    self.intent.as_str(),
                    path,
                    target.expected_digest,
                    next_digest,
                    backup.map(|copy| copy.path.as_str()),
                    now,
                    i64::from(self.direct)
                ],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
        if inserted != 1 {
            return Err(WriteError::InvalidRequest);
        }
        Ok(())
    }

    /// Finalize after verifying the target. If this update fails, the durable
    /// `prepared` row and its snapshot remain; callers must not claim no write.
    /// Returns storage/log errors without deleting the row or its backup.
    pub(super) fn finish<T>(
        &self,
        id: &str,
        result: &Result<T, WriteError>,
    ) -> Result<(), WriteError> {
        let (state, error) = match result {
            Ok(_) => ("completed", None),
            Err(WriteError::OutcomeUnknown) => ("unknown", Some(WriteError::OutcomeUnknown.code())),
            Err(error) => ("failed", Some(error.code())),
        };
        let now = shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)?;
        let connection = self
            .storage
            .lock()
            .map_err(|_| WriteError::StorageUnavailable)?;
        let updated = connection
            .execute(
                "UPDATE config_file_revision SET state = ?2, error_code = ?3,
                updated_at = max(created_at, ?4) WHERE id = ?1 AND state = 'prepared'",
                params![id, state, error, now],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
        if updated != 1 {
            return Err(WriteError::LogWriteFailed);
        }
        Ok(())
    }
}
