//! Read-only, root-scoped configuration history, including restore/compensation batches.
//!
//! A page is one SQLite read snapshot. Cursors seek by immutable entry time and
//! identity rather than an offset; separate pages remain a live view of the log.
//! This module reads journal metadata only. It neither opens configuration or
//! backup files nor grants permission to restore, resolve or delete anything.

use std::path::{Component, Path};

use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

use super::{
    OperationKind, OperationRecord, OperationState, RestoreState, RevisionMatch, StepState,
    WriteError,
    operations::{is_digest, read_operation},
    path_text,
};
use crate::{config_plan::digest_hex, shared, storage::Storage};

const MAX_PAGE_SIZE: u32 = 50;
const MAX_SCOPE_FILES: usize = 16;

/// Exact files owned by one adapter below one configuration root.
///
/// Construct this in trusted Rust adapter code, not from frontend-supplied paths.
/// Every step of a listed source operation must match one of these file paths.
/// Matching is textual, not a path prefix or filesystem identity comparison:
/// aliases and symbolic links are not resolved, and missing roots still work.
#[derive(Debug)]
pub struct HistoryScope {
    kind: OperationKind,
    targets: Vec<String>,
    digest: String,
}

impl HistoryScope {
    /// Bind an Agent operation kind to its root and owned, direct child files.
    ///
    /// Does not access the filesystem. File order does not affect the scope.
    /// Returns `InvalidRequest` for unusable paths, an empty/oversized file list,
    /// duplicate file names, or names containing directory components.
    pub fn new(kind: OperationKind, root: &Path, files: &[&str]) -> Result<Self, WriteError> {
        if path_text(root).is_err() || files.is_empty() || files.len() > MAX_SCOPE_FILES {
            return Err(WriteError::InvalidRequest);
        }
        let mut targets = Vec::with_capacity(files.len());
        for file in files {
            let mut components = Path::new(file).components();
            if !matches!(components.next(), Some(Component::Normal(_)))
                || components.next().is_some()
                || file.contains(['/', '\\', ':'])
            {
                return Err(WriteError::InvalidRequest);
            }
            targets.push(path_text(&root.join(file)).map_err(|_| WriteError::InvalidRequest)?);
        }
        targets.sort();
        if targets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WriteError::InvalidRequest);
        }
        let bytes = serde_json::to_vec(&(kind.as_str(), &targets))
            .map_err(|_| WriteError::InvalidRequest)?;
        Ok(Self {
            kind,
            targets,
            digest: digest_hex(&bytes),
        })
    }
}

/// Position after the last entry of a page, bound to its exact Agent/file scope.
///
/// This is a pagination position, not a signed authorization token. The query
/// always reapplies the server-selected scope, even for a constructed cursor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryCursor {
    scope_digest: String,
    created_at: i64,
    entry_key: String,
}

/// Why a group of files was changed; not a recovery instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryIntent {
    /// The original feature apply attempt, including legacy/failed attempts.
    Apply,
    /// A separately confirmed restore attempt.
    Restore,
    /// A failed apply's automatic compensation attempt.
    Compensation,
}

impl HistoryIntent {
    fn from_stored(value: &str) -> Result<Self, WriteError> {
        match value {
            "apply" => Ok(Self::Apply),
            "restore" => Ok(Self::Restore),
            "compensation" => Ok(Self::Compensation),
            _ => Err(WriteError::InvalidStoredOperation),
        }
    }
}

/// Recorded outcome of a single file mutation, without inferring current bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryRevisionState {
    /// The snapshot committed; the final result was not recorded.
    Prepared,
    /// The journal reports completion; see any later reconciliation observation.
    Completed,
    /// A definite failure was recorded.
    Failed,
    /// The final result could not be verified.
    Unknown,
}

impl HistoryRevisionState {
    fn from_stored(value: &str) -> Result<Self, WriteError> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err(WriteError::InvalidStoredOperation),
        }
    }
}

/// Original operation step metadata, not necessarily changed by this batch.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySourceStep {
    /// Position in the original operation.
    pub ordinal: u32,
    /// Exact recorded target owned by this scope.
    pub target_path: String,
    /// Original expected digest; null may mean absence, not an observation now.
    pub expected_digest: Option<String>,
    /// Last result recorded for the apply step; absent unless it applied.
    pub applied_digest: Option<String>,
    /// Original existence bookkeeping, possibly absent in old logs.
    pub previous_existed: Option<bool>,
    /// Whether the legacy step retained a copy reference, not its availability.
    pub backup_recorded: bool,
    /// Current journal state of the original apply step.
    pub state: StepState,
    /// Known write error recorded by a failed original step.
    pub error_code: Option<WriteError>,
    /// Latest legacy rollback bookkeeping, not this entry's mutation outcome.
    pub restore_state: Option<RestoreState>,
}

/// An explicit reconciliation audit, not proof of who changed the file.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResolution {
    /// Journal state before the user confirmed the observation.
    pub previous_state: HistoryRevisionState,
    /// Digest observed at reconciliation; null records observed absence.
    pub observed_digest: Option<String>,
    /// Only `previous` or `intended` can be explicitly settled.
    pub current_match: RevisionMatch,
    /// Unix milliseconds when the observation was recorded.
    pub resolved_at: i64,
}

/// One independent pre-mutation snapshot, with no private backup path or bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRevision {
    /// Identity of the retained pre-image, not an approval to use it.
    pub revision_id: String,
    /// Position in the source operation, shared with `source_steps`.
    pub ordinal: u32,
    /// Digest before mutation; null records absence, not an empty file.
    pub previous_digest: Option<String>,
    /// Intended result; null means removal, not a missing observation.
    pub intended_digest: Option<String>,
    /// Whether a private copy reference was recorded, NOT its availability.
    pub backup_recorded: bool,
    /// Recorded file result, independent of the original operation's state.
    pub state: HistoryRevisionState,
    /// Explicit observation that repaired this result, when one exists.
    pub resolution: Option<HistoryResolution>,
    /// Known safe write-layer error, when the file result requires one.
    pub error_code: Option<WriteError>,
    /// Unix milliseconds when this file's snapshot committed.
    pub created_at: i64,
    /// Unix milliseconds when its recorded result last changed.
    pub updated_at: i64,
}

/// One original apply attempt or one restore/compensation batch, kept whole.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Stable, namespaced identity: `operation:<id>` or `batch:<id>`.
    pub entry_key: String,
    /// Source operation identity; restore/compensation do not create old jobs.
    pub operation_id: String,
    /// Why this entry exists, not what should run after a restart.
    pub intent: HistoryIntent,
    /// Provider reference recorded on the source operation, possibly absent.
    pub owner_ref: Option<String>,
    /// Current state of the SOURCE operation, not a batch's aggregate result.
    pub source_state: OperationState,
    /// Immutable sort time: operation creation or the batch's first inserted row.
    pub created_at: i64,
    /// Latest source-operation or included revision update time.
    pub updated_at: i64,
    /// Original operation's target list; revisions name the actual batch subset.
    pub source_steps: Vec<HistorySourceStep>,
    /// Apply revisions for an operation, or all revisions of this extra batch.
    /// Empty for legacy operations and attempts that never prepared a mutation.
    pub revisions: Vec<HistoryRevision>,
}

/// Bounded read result. No total is inferred from a changing journal.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    /// Newest first; every multi-file batch stays inside one entry.
    pub entries: Vec<HistoryEntry>,
    /// Seek position for more entries, absent when this read reached the end.
    pub next_cursor: Option<HistoryCursor>,
}

/// Read one page of the complete operation and mutation-batch history in scope.
///
/// Each page uses a deferred read transaction, so another app instance cannot
/// make the ID selection and entry validation observe different database states.
/// Across pages, metadata may change and entries may disappear; this is not a
/// frozen export. Newer insertions do not shift the seek position. No target or
/// backup files are opened and no journal rows are modified.
///
/// # Errors
/// Returns `InvalidRequest` for a limit outside 1–50 or an invalid/wrong-scope
/// cursor, `InvalidStoredOperation` for inconsistent selected metadata, and safe
/// storage/log errors for database failures. Off-scope or mixed-root operations
/// are excluded as a whole before their revisions are read.
pub fn configuration_history(
    storage: &Storage,
    scope: &HistoryScope,
    after: Option<&HistoryCursor>,
    limit: u32,
) -> Result<HistoryPage, WriteError> {
    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(WriteError::InvalidRequest);
    }
    if let Some(cursor) = after
        && (cursor.scope_digest != scope.digest
            || cursor.created_at < 0
            || !valid_entry_key(&cursor.entry_key))
    {
        return Err(WriteError::InvalidRequest);
    }
    let mut connection = storage.lock().map_err(|_| WriteError::StorageUnavailable)?;
    let transaction = connection
        .transaction()
        .map_err(|_| WriteError::LogReadFailed)?;
    let mut candidates = read_candidates(&transaction, scope, after, limit + 1)?;
    let has_more = candidates.len() > limit as usize;
    if has_more {
        candidates.pop();
    }
    let mut entries = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        entries.push(read_entry(&transaction, scope, candidate)?);
    }
    let next_cursor = if has_more {
        entries.last().map(|entry| HistoryCursor {
            scope_digest: scope.digest.clone(),
            created_at: entry.created_at,
            entry_key: entry.entry_key.clone(),
        })
    } else {
        None
    };
    transaction
        .commit()
        .map_err(|_| WriteError::LogReadFailed)?;
    Ok(HistoryPage {
        entries,
        next_cursor,
    })
}

struct Candidate {
    entry_key: String,
    operation_id: String,
    created_at: i64,
}

fn read_candidates(
    connection: &Connection,
    scope: &HistoryScope,
    after: Option<&HistoryCursor>,
    limit: u32,
) -> Result<Vec<Candidate>, WriteError> {
    // Only placeholder numbers are formatted into SQL. Paths, kinds and cursor
    // values remain bound parameters; `%` and `_` in roots are literal text.
    let path_parameters = (2..scope.targets.len() + 2)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let time_parameter = scope.targets.len() + 2;
    let key_parameter = time_parameter + 1;
    let limit_parameter = time_parameter + 2;
    let sql = format!(
        "WITH scoped AS (
            SELECT o.id, o.created_at FROM config_operation o
            WHERE o.kind = ?1
              AND EXISTS (SELECT 1 FROM config_operation_step s WHERE s.operation_id = o.id)
              AND NOT EXISTS (SELECT 1 FROM config_operation_step s
                WHERE s.operation_id = o.id AND s.target_path NOT IN ({path_parameters}))
        ), entries AS (
            SELECT 'operation:' || id AS entry_key, id AS operation_id, created_at FROM scoped
            UNION ALL
            SELECT 'batch:' || r.batch_id, r.operation_id, r.created_at
            FROM config_file_revision r JOIN scoped s ON s.id = r.operation_id
            WHERE r.intent != 'apply' AND r.rowid = (
                SELECT min(first.rowid) FROM config_file_revision first
                WHERE first.batch_id = r.batch_id AND first.operation_id = r.operation_id
                    AND first.intent != 'apply')
        ) SELECT entry_key, operation_id, created_at FROM entries
        WHERE (?{time_parameter} IS NULL OR
            (created_at, entry_key) < (?{time_parameter}, ?{key_parameter}))
        ORDER BY created_at DESC, entry_key DESC LIMIT ?{limit_parameter}"
    );
    let mut parameters = vec![Value::Text(scope.kind.as_str().to_owned())];
    parameters.extend(scope.targets.iter().cloned().map(Value::Text));
    parameters.push(after.map_or(Value::Null, |cursor| Value::Integer(cursor.created_at)));
    parameters.push(after.map_or(Value::Null, |cursor| Value::Text(cursor.entry_key.clone())));
    parameters.push(Value::Integer(i64::from(limit)));
    let mut query = connection
        .prepare(&sql)
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map(params_from_iter(parameters), |row| {
            Ok(Candidate {
                entry_key: row.get(0)?,
                operation_id: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|_| WriteError::LogReadFailed)?;
    rows.collect::<Result<_, _>>()
        .map_err(|_| WriteError::InvalidStoredOperation)
}

fn read_entry(
    connection: &Connection,
    scope: &HistoryScope,
    candidate: Candidate,
) -> Result<HistoryEntry, WriteError> {
    if !valid_entry_key(&candidate.entry_key)
        || !shared::is_random_id(&candidate.operation_id)
        || candidate.created_at < 0
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    let source = read_operation(connection, &candidate.operation_id)?
        .ok_or(WriteError::InvalidStoredOperation)?;
    if source.kind != scope.kind
        || source.steps.len() > MAX_SCOPE_FILES
        || source
            .steps
            .iter()
            .any(|step| !scope.targets.contains(&step.target_path))
    {
        return Err(WriteError::InvalidStoredOperation);
    }
    let batch = candidate.entry_key.strip_prefix("batch:");
    let (intent, revisions) = read_revisions(connection, &source, batch)?;
    let updated_at = revisions
        .iter()
        .map(|row| row.updated_at)
        .fold(source.updated_at, i64::max);
    let mut source_steps = Vec::with_capacity(source.steps.len());
    for step in source.steps {
        source_steps.push(HistorySourceStep {
            ordinal: step.ordinal,
            target_path: step.target_path,
            expected_digest: step.expected_digest,
            applied_digest: step.applied_digest,
            previous_existed: step.previous_existed,
            backup_recorded: step.backup.is_some(),
            state: step.state,
            error_code: step.error_code.as_deref().map(parse_error).transpose()?,
            restore_state: step.restore_state,
        });
    }
    Ok(HistoryEntry {
        entry_key: candidate.entry_key,
        operation_id: source.id,
        intent,
        owner_ref: source.owner_ref,
        source_state: source.state,
        created_at: candidate.created_at,
        updated_at,
        source_steps,
        revisions,
    })
}

struct StoredRevision {
    id: String,
    batch_id: String,
    operation_id: String,
    ordinal: u32,
    intent: String,
    target_path: String,
    previous_digest: Option<String>,
    next_digest: Option<String>,
    backup_path: Option<String>,
    state: String,
    error_code: Option<String>,
    created_at: i64,
    updated_at: i64,
}

fn read_revisions(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<(HistoryIntent, Vec<HistoryRevision>), WriteError> {
    let predicate = if batch.is_some() {
        "batch_id = ?1"
    } else {
        "operation_id = ?1 AND intent = 'apply'"
    };
    let mut query = connection
        .prepare(&format!(
            "SELECT id, batch_id, operation_id, ordinal, intent, target_path,
         previous_digest, next_digest, backup_path, state, error_code, created_at, updated_at
         FROM config_file_revision WHERE {predicate} ORDER BY ordinal, id LIMIT ?2"
        ))
        .map_err(|_| WriteError::LogReadFailed)?;
    let rows = query
        .query_map(
            params![
                batch.unwrap_or(&source.id),
                i64::try_from(MAX_SCOPE_FILES + 1).map_err(|_| WriteError::InvalidRequest)?
            ],
            |row| {
                Ok(StoredRevision {
                    id: row.get(0)?,
                    batch_id: row.get(1)?,
                    operation_id: row.get(2)?,
                    ordinal: row.get(3)?,
                    intent: row.get(4)?,
                    target_path: row.get(5)?,
                    previous_digest: row.get(6)?,
                    next_digest: row.get(7)?,
                    backup_path: row.get(8)?,
                    state: row.get(9)?,
                    error_code: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            },
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let mut revisions = Vec::new();
    let mut intent = None;
    for result in rows {
        let row = result.map_err(|_| WriteError::InvalidStoredOperation)?;
        let row_intent = HistoryIntent::from_stored(&row.intent)?;
        let state = HistoryRevisionState::from_stored(&row.state)?;
        let error_code = row.error_code.as_deref().map(parse_error).transpose()?;
        let source_step = source.steps.iter().find(|step| step.ordinal == row.ordinal);
        if revisions.len() >= MAX_SCOPE_FILES
            || !valid_revision_id(&row.id)
            || !valid_revision_id(&row.batch_id)
            || row.operation_id != source.id
            || source_step.is_none_or(|step| step.target_path != row.target_path)
            || row.batch_id != batch.unwrap_or(&source.id)
            || (batch.is_none() && row_intent != HistoryIntent::Apply)
            || (batch.is_some() && row_intent == HistoryIntent::Apply)
            || intent.is_some_and(|previous| previous != row_intent)
            || revisions
                .iter()
                .any(|previous: &HistoryRevision| previous.ordinal == row.ordinal)
            || row.created_at < 0
            || row.updated_at < row.created_at
            || row
                .previous_digest
                .as_deref()
                .is_some_and(|value| !is_digest(value))
            || row
                .next_digest
                .as_deref()
                .is_some_and(|value| !is_digest(value))
            || row.previous_digest.is_some() != row.backup_path.is_some()
            || row
                .backup_path
                .as_deref()
                .is_some_and(|value| path_text(Path::new(value)).is_err())
            || !valid_result(state, error_code)
        {
            return Err(WriteError::InvalidStoredOperation);
        }
        intent = Some(row_intent);
        let resolution = read_resolution(connection, &row, state)?;
        revisions.push(HistoryRevision {
            revision_id: row.id,
            ordinal: row.ordinal,
            previous_digest: row.previous_digest,
            intended_digest: row.next_digest,
            backup_recorded: row.backup_path.is_some(),
            state,
            resolution,
            error_code,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    if batch.is_some() && revisions.is_empty() {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok((intent.unwrap_or(HistoryIntent::Apply), revisions))
}

fn read_resolution(
    connection: &Connection,
    revision: &StoredRevision,
    state: HistoryRevisionState,
) -> Result<Option<HistoryResolution>, WriteError> {
    let audit = connection
        .query_row(
            "SELECT previous_state, observed_digest, matched_state, resolved_at
         FROM config_revision_resolution WHERE revision_id = ?1",
            [&revision.id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| WriteError::LogReadFailed)?;
    let Some((previous, observed_digest, matched, resolved_at)) = audit else {
        return Ok(None);
    };
    let previous_state = HistoryRevisionState::from_stored(&previous)?;
    let current_match = match matched.as_str() {
        "previous"
            if state == HistoryRevisionState::Failed
                && revision.error_code.as_deref() == Some(WriteError::WriteFailed.code())
                && observed_digest == revision.previous_digest =>
        {
            RevisionMatch::Previous
        }
        "intended"
            if state == HistoryRevisionState::Completed
                && observed_digest == revision.next_digest =>
        {
            RevisionMatch::Intended
        }
        _ => return Err(WriteError::InvalidStoredOperation),
    };
    if previous_state == HistoryRevisionState::Failed || resolved_at < 0 {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok(Some(HistoryResolution {
        previous_state,
        observed_digest,
        current_match,
        resolved_at,
    }))
}

fn valid_result(state: HistoryRevisionState, error: Option<WriteError>) -> bool {
    match state {
        HistoryRevisionState::Prepared | HistoryRevisionState::Completed => error.is_none(),
        HistoryRevisionState::Failed => {
            error.is_some_and(|error| error != WriteError::OutcomeUnknown)
        }
        HistoryRevisionState::Unknown => error == Some(WriteError::OutcomeUnknown),
    }
}

fn parse_error(code: &str) -> Result<WriteError, WriteError> {
    [
        WriteError::InvalidRequest,
        WriteError::TargetChanged,
        WriteError::SymlinkRefused,
        WriteError::TargetReadOnly,
        WriteError::TargetPathInvalid,
        WriteError::TargetUnreadable,
        WriteError::TargetTooLarge,
        WriteError::BackupUnavailable,
        WriteError::WriteFailed,
        WriteError::OutcomeUnknown,
        WriteError::StorageUnavailable,
        WriteError::LogWriteFailed,
        WriteError::LogReadFailed,
        WriteError::InvalidStoredOperation,
        WriteError::PlanStale,
    ]
    .into_iter()
    .find(|error| error.code() == code)
    .ok_or(WriteError::InvalidStoredOperation)
}

fn valid_revision_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn valid_entry_key(key: &str) -> bool {
    if let Some(id) = key.strip_prefix("operation:") {
        shared::is_random_id(id)
    } else {
        key.strip_prefix("batch:").is_some_and(valid_revision_id)
    }
}
