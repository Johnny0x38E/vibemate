//! SQLite bookkeeping for configuration writes.
//!
//! The log answers three questions after the fact: which files one operation
//! planned to replace, what each step did, and which operations are still
//! unfinished. A step row is written before the first file is replaced, so a
//! crash halfway leaves a `planned` operation whose applied steps are visible
//! after a restart. Rows hold paths, digests and stable error codes only; the
//! replaced content lives in the private backup directory, referenced by the
//! step's backup columns.

use std::{path::Path, sync::MutexGuard};

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    Backup, OperationKind, OperationRecord, OperationState, RestoreState, StepRecord, StepState,
    WriteError, WriteTarget, path_text,
};
use crate::{shared, storage::Storage};

/// Upper bound on a listing read, so a long-lived log is never read whole.
const MAX_RECENT_OPERATIONS: u32 = 50;

/// Lock the shared connection for one log statement.
fn lock(storage: &Storage) -> Result<MutexGuard<'_, Connection>, WriteError> {
    storage.lock().map_err(|_| WriteError::StorageUnavailable)
}

/// Read the clock for a stored timestamp.
fn now_millis() -> Result<i64, WriteError> {
    shared::current_unix_millis().map_err(|_| WriteError::LogWriteFailed)
}

/// Fresh operation identity, in the shape every other stored row uses.
fn random_id(connection: &Connection) -> Result<String, WriteError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| WriteError::LogWriteFailed)
}

/// Whether the text is a lowercase SHA-256 token.
///
/// The storage CHECK constraints enforce the same format; this check keeps a
/// malformed value out of the log before SQLite sees it, and catches it again
/// when a row written by other means is read back.
pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether the text is a stable error code in the log's shape.
fn is_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

/// Whether the text is a usable operation-owner reference.
///
/// The log stores a reference, not a display name, so renaming a Provider does
/// not rewrite history. The shape is bounded printable text; each feature
/// decides what its own reference means.
fn is_owner_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
}

/// Record an operation and every one of its steps.
///
/// The operation starts `planned` and each step `pending`; both are committed
/// before the caller replaces any file, so unfinished work is discoverable.
/// `owner` names what the operation writes configuration for (the Provider ID
/// for a model link); it is stored with the operation and never with a file.
///
/// # Errors
/// Returns `InvalidRequest` for an empty step list, a target path that cannot be
/// shown to the user, an owner reference of the wrong shape, or an expected
/// digest that is not a SHA-256 token, `StorageUnavailable` when the database is
/// closed, and `LogWriteFailed` when the insert did not commit.
pub fn begin_operation(
    storage: &Storage,
    kind: OperationKind,
    owner: Option<&str>,
    steps: &[WriteTarget],
) -> Result<String, WriteError> {
    if steps.is_empty() {
        return Err(WriteError::InvalidRequest);
    }
    if owner.is_some_and(|value| !is_owner_ref(value)) {
        return Err(WriteError::InvalidRequest);
    }
    let mut described = Vec::with_capacity(steps.len());
    for step in steps {
        let path = path_text(&step.path)?;
        if step
            .expected_digest
            .as_deref()
            .is_some_and(|digest| !is_digest(digest))
        {
            return Err(WriteError::InvalidRequest);
        }
        described.push((path, step.expected_digest.clone()));
    }
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogWriteFailed)?;
    let id = random_id(&transaction)?;
    let now = now_millis()?;
    transaction
        .execute(
            "INSERT INTO config_operation (id, kind, owner_ref, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'planned', ?4, ?4)",
            params![id, kind.as_str(), owner, now],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    for (ordinal, (path, digest)) in described.iter().enumerate() {
        let ordinal = i64::try_from(ordinal).map_err(|_| WriteError::InvalidRequest)?;
        transaction
            .execute(
                "INSERT INTO config_operation_step
                     (operation_id, ordinal, target_path, expected_digest, state, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
                params![id, ordinal, path, digest, now],
            )
            .map_err(|_| WriteError::LogWriteFailed)?;
    }
    transaction
        .commit()
        .map_err(|_| WriteError::LogWriteFailed)?;
    Ok(id)
}

/// Record that one step replaced its file.
///
/// `backup` is stored when the step replaced an existing file; `applied_digest`
/// is the SHA-256 now on disk, which a later operation compares against; and
/// `previously_existed` says whether the file was there before this step ran, so
/// a restore can tell a file the step created from one it found unchanged.
///
/// # Errors
/// Returns `InvalidRequest` for a digest that is not SHA-256 or for a step the
/// operation does not have, `StorageUnavailable` when the database is closed,
/// and `LogWriteFailed` when the update did not commit.
pub fn record_step_applied(
    storage: &Storage,
    operation_id: &str,
    ordinal: u32,
    backup: Option<&Backup>,
    applied_digest: &str,
    previously_existed: bool,
) -> Result<(), WriteError> {
    if !is_digest(applied_digest) {
        return Err(WriteError::InvalidRequest);
    }
    let (backup_id, backup_path, backup_digest) = match backup {
        Some(backup) => (
            Some(backup.id.as_str()),
            Some(backup.path.as_str()),
            Some(backup.content_digest.as_str()),
        ),
        None => (None, None, None),
    };
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogWriteFailed)?;
    let now = now_millis()?;
    let updated = transaction
        .execute(
            "UPDATE config_operation_step
             SET state = ?3, error_code = NULL, applied_digest = ?4,
                 previous_existed = ?5, backup_id = ?6, backup_path = ?7,
                 backup_digest = ?8, updated_at = ?9
             WHERE operation_id = ?1 AND ordinal = ?2",
            params![
                operation_id,
                i64::from(ordinal),
                StepState::Applied.as_str(),
                applied_digest,
                i64::from(previously_existed),
                backup_id,
                backup_path,
                backup_digest,
                now
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated == 0 {
        return Err(WriteError::InvalidRequest);
    }
    touch_operation(&transaction, operation_id, now)?;
    transaction.commit().map_err(|_| WriteError::LogWriteFailed)
}

/// Record that one step left its file as it was.
///
/// # Errors
/// As [`record_step_applied`], with the failure's stable code stored beside the step.
pub fn record_step_failed(
    storage: &Storage,
    operation_id: &str,
    ordinal: u32,
    error: WriteError,
) -> Result<(), WriteError> {
    let code = error.code();
    if !is_error_code(code) {
        return Err(WriteError::InvalidRequest);
    }
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogWriteFailed)?;
    let now = now_millis()?;
    let updated = transaction
        .execute(
            "UPDATE config_operation_step
             SET state = ?3, error_code = ?4, applied_digest = NULL,
                 previous_existed = NULL, backup_id = NULL, backup_path = NULL,
                 backup_digest = NULL, updated_at = ?5
             WHERE operation_id = ?1 AND ordinal = ?2",
            params![
                operation_id,
                i64::from(ordinal),
                StepState::Failed.as_str(),
                code,
                now
            ],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated == 0 {
        return Err(WriteError::InvalidRequest);
    }
    touch_operation(&transaction, operation_id, now)?;
    transaction.commit().map_err(|_| WriteError::LogWriteFailed)
}

/// Record the state an operation ended in.
///
/// # Errors
/// Returns `InvalidRequest` for an unknown operation, `StorageUnavailable` when
/// the database is closed, and `LogWriteFailed` when the update did not commit.
pub fn finish_operation(
    storage: &Storage,
    operation_id: &str,
    state: OperationState,
) -> Result<(), WriteError> {
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogWriteFailed)?;
    let now = now_millis()?;
    let updated = transaction
        .execute(
            "UPDATE config_operation SET state = ?2, updated_at = max(?3, created_at) WHERE id = ?1",
            params![operation_id, state.as_str(), now],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated == 0 {
        return Err(WriteError::InvalidRequest);
    }
    transaction.commit().map_err(|_| WriteError::LogWriteFailed)
}

/// Record what a rollback did to one applied step.
///
/// Only an applied step can be rolled back; a step the operation never wrote is
/// refused instead of being marked as restored. The value is overwritten by a
/// later attempt, so the row holds the latest result rather than a history.
///
/// # Errors
/// Returns `InvalidRequest` for a step the operation does not have or one that
/// is not applied, `StorageUnavailable` when the database is closed, and
/// `LogWriteFailed` when the update did not commit.
pub fn record_step_restored(
    storage: &Storage,
    operation_id: &str,
    ordinal: u32,
    state: RestoreState,
) -> Result<(), WriteError> {
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogWriteFailed)?;
    let now = now_millis()?;
    let updated = transaction
        .execute(
            "UPDATE config_operation_step
             SET restore_state = ?3, updated_at = ?4
             WHERE operation_id = ?1 AND ordinal = ?2 AND state = 'applied'",
            params![operation_id, i64::from(ordinal), state.as_str(), now],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated == 0 {
        return Err(WriteError::InvalidRequest);
    }
    touch_operation(&transaction, operation_id, now)?;
    transaction.commit().map_err(|_| WriteError::LogWriteFailed)
}

/// Operations that still need an answer, oldest first.
///
/// Everything that is not known to be settled is listed, so a state this build
/// does not understand is reported as [`WriteError::InvalidStoredOperation`]
/// instead of disappearing from the list. A `planned` operation may be either
/// untouched or half applied; its steps tell the two apart.
///
/// # Errors
/// Returns `StorageUnavailable` when the database is closed, `LogReadFailed`
/// when the query failed, and `InvalidStoredOperation` for a malformed row.
pub fn unfinished_operations(storage: &Storage) -> Result<Vec<OperationRecord>, WriteError> {
    let connection = lock(storage)?;
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM config_operation
                 WHERE state NOT IN ('applied', 'failed', 'compensated')
                 ORDER BY created_at, id",
            )
            .map_err(|_| WriteError::LogReadFailed)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| WriteError::LogReadFailed)?;
        rows.collect::<Result<Vec<String>, _>>()
            .map_err(|_| WriteError::LogReadFailed)?
    };
    let mut records = Vec::with_capacity(ids.len());
    for id in &ids {
        // The row was listed one statement ago; failing to read it means the log
        // changed under this read, which is reported instead of hidden.
        records.push(read_operation(&connection, id)?.ok_or(WriteError::InvalidStoredOperation)?);
    }
    Ok(records)
}

/// Whether this adapter previously wrote a target for the given owner.
///
/// Used before adopting a namespaced Pi provider entry: a coincidentally named
/// user entry must not be overwritten merely because its name has our prefix.
/// Returns storage/log errors without exposing database diagnostics.
pub fn has_recorded_target(
    storage: &Storage,
    kind: OperationKind,
    owner: &str,
    target: &str,
) -> Result<bool, WriteError> {
    lock(storage)?
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM config_operation o
         JOIN config_operation_step s ON s.operation_id = o.id
         WHERE o.kind = ?1 AND o.owner_ref = ?2 AND s.target_path = ?3
           AND s.applied_digest IS NOT NULL)",
            params![kind.as_str(), owner, target],
            |row| row.get(0),
        )
        .map_err(|_| WriteError::LogReadFailed)
}

/// Recent operations of one kind, newest first.
///
/// `limit` is clamped to [`MAX_RECENT_OPERATIONS`]; the listing is what the
/// restore view shows, so reading the whole log is never needed. A malformed row
/// is reported, not skipped: silence would hide a write that did happen.
///
/// # Errors
/// As [`unfinished_operations`].
pub fn recent_operations(
    storage: &Storage,
    kind: OperationKind,
    limit: u32,
) -> Result<Vec<OperationRecord>, WriteError> {
    let limit = limit.clamp(1, MAX_RECENT_OPERATIONS);
    let connection = lock(storage)?;
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM config_operation
                 WHERE kind = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?2",
            )
            .map_err(|_| WriteError::LogReadFailed)?;
        let rows = statement
            .query_map(params![kind.as_str(), i64::from(limit)], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|_| WriteError::LogReadFailed)?;
        rows.collect::<Result<Vec<String>, _>>()
            .map_err(|_| WriteError::LogReadFailed)?
    };
    let mut records = Vec::with_capacity(ids.len());
    for id in &ids {
        records.push(read_operation(&connection, id)?.ok_or(WriteError::InvalidStoredOperation)?);
    }
    Ok(records)
}

/// One operation with its steps, or `None` when no such operation is stored.
///
/// # Errors
/// As [`unfinished_operations`], without the state filter. A malformed identity
/// is refused rather than answered with `None`, so a caller cannot mistake it
/// for an operation that is simply absent.
pub fn operation(
    storage: &Storage,
    operation_id: &str,
) -> Result<Option<OperationRecord>, WriteError> {
    let connection = lock(storage)?;
    read_operation(&connection, operation_id)
}

/// Move an operation's `updated_at` forward, inside the caller's transaction.
fn touch_operation(
    connection: &Connection,
    operation_id: &str,
    now: i64,
) -> Result<(), WriteError> {
    let updated = connection
        .execute(
            "UPDATE config_operation SET updated_at = max(?2, created_at) WHERE id = ?1",
            params![operation_id, now],
        )
        .map_err(|_| WriteError::LogWriteFailed)?;
    if updated == 0 {
        return Err(WriteError::InvalidRequest);
    }
    Ok(())
}

/// Read one operation row and validate it instead of trusting the database.
///
/// A well-formed identity with no row is `Ok(None)`; a malformed identity, or a
/// row whose fields cannot be trusted, is an error.
pub(super) fn read_operation(
    connection: &Connection,
    id: &str,
) -> Result<Option<OperationRecord>, WriteError> {
    if !shared::is_random_id(id) {
        return Err(WriteError::InvalidRequest);
    }
    let row: Option<(String, String, Option<String>, i64, i64)> = connection
        .query_row(
            "SELECT kind, state, owner_ref, created_at, updated_at
             FROM config_operation WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?;
    let Some((kind, state, owner_ref, created_at, updated_at)) = row else {
        return Ok(None);
    };
    let kind = OperationKind::from_stored(&kind).ok_or(WriteError::InvalidStoredOperation)?;
    let state = OperationState::from_stored(&state).ok_or(WriteError::InvalidStoredOperation)?;
    if owner_ref
        .as_deref()
        .is_some_and(|value| !is_owner_ref(value))
        || created_at < 0
        || updated_at < created_at
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    let steps = read_steps(connection, id)?;
    if steps.is_empty() {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(Some(OperationRecord {
        id: id.to_owned(),
        kind,
        owner_ref,
        state,
        created_at,
        updated_at,
        steps,
    }))
}

/// Read an operation's steps in order and validate every field.
fn read_steps(connection: &Connection, id: &str) -> Result<Vec<StepRecord>, WriteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, target_path, expected_digest, state, error_code,
                    backup_id, backup_path, backup_digest, applied_digest,
                    previous_existed, restore_state
             FROM config_operation_step WHERE operation_id = ?1 ORDER BY ordinal",
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = statement
        .query_map([id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    let mut steps = Vec::new();
    for (index, row) in rows.enumerate() {
        let invalid = || WriteError::InvalidStoredOperation;
        let (
            ordinal,
            target_path,
            expected_digest,
            state,
            error_code,
            backup_id,
            backup_path,
            backup_digest,
            applied_digest,
            previous_existed,
            restore_state,
        ) = row.map_err(|_| WriteError::LogReadFailed)?;
        // Ordinals are the step order; a gap or a repeat means the row was changed.
        if ordinal != i64::try_from(index).map_err(|_| invalid())? {
            return Err(invalid());
        }
        let state = StepState::from_stored(&state).ok_or_else(invalid)?;
        for digest in [&expected_digest, &backup_digest, &applied_digest] {
            if digest.as_deref().is_some_and(|value| !is_digest(value)) {
                return Err(invalid());
            }
        }
        if error_code
            .as_deref()
            .is_some_and(|code| !is_error_code(code))
        {
            return Err(invalid());
        }
        let backup = match (backup_id, backup_path, backup_digest) {
            (Some(id), Some(path), Some(content_digest)) if is_random_id_or_operation(&id) => {
                Some(Backup {
                    id,
                    path,
                    content_digest,
                })
            }
            (None, None, None) => None,
            _ => return Err(invalid()),
        };
        // The three states carry distinct fields; a row that mixes them is not
        // shown as if it were a real outcome.
        if (state == StepState::Failed) != error_code.is_some()
            || (state == StepState::Applied) != applied_digest.is_some()
            || (backup.is_some() && state != StepState::Applied)
        {
            return Err(invalid());
        }
        if path_text(Path::new(&target_path)).is_err() {
            return Err(invalid());
        }
        // Whether the file existed before the step belongs to an applied step,
        // and only its two stated values count.
        let previous_existed = match (previous_existed, state) {
            (None, _) => None,
            (Some(0), StepState::Applied) => Some(false),
            (Some(1), StepState::Applied) => Some(true),
            (Some(_), _) => return Err(invalid()),
        };
        // Only an applied step can carry a rollback result, and only a known
        // value counts as one.
        let restore_state = match (restore_state, state) {
            (None, _) => None,
            (Some(value), StepState::Applied) => {
                Some(RestoreState::from_stored(&value).ok_or_else(invalid)?)
            }
            (Some(_), _) => return Err(invalid()),
        };
        steps.push(StepRecord {
            ordinal: u32::try_from(ordinal).map_err(|_| invalid())?,
            target_path,
            expected_digest,
            state,
            error_code,
            backup,
            applied_digest,
            previous_existed,
            restore_state,
        });
    }
    Ok(steps)
}

/// Backup identifiers are write identifiers, not stored rows; only their shape
/// is checked here.
fn is_random_id_or_operation(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
