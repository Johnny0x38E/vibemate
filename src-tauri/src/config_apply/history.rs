//! Read-only, root-scoped configuration history, including restore/compensation batches.
//!
//! A page is one SQLite read snapshot. Cursors seek by immutable entry time and
//! identity rather than an offset; separate pages remain a live view of the log.
//! Journal pages may open private backup files to report availability, and may
//! open configuration targets when identifying the current version. They never
//! return backup paths or file bytes, and never grant restore, resolve or delete
//! permission.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

use super::{
    BackupStore, OperationKind, OperationRecord, OperationState, RestoreState, RevisionMatch,
    StepState, WriteError,
    inspection::{RevisionBackupState, recorded_backup_state},
    operations::{is_digest, read_operation},
    path_text,
};
use crate::{config_plan::digest_hex, shared, storage::Storage};

const MAX_PAGE_SIZE: u32 = 50;
pub(super) const MAX_SCOPE_FILES: usize = 16;

/// Exact files owned by one adapter below one configuration root.
///
/// Construct this in trusted Rust adapter code, not from frontend-supplied paths.
/// Every step of a listed source operation must match one of these file paths.
/// Matching is textual, not a path prefix or filesystem identity comparison:
/// aliases and symbolic links are not resolved, and missing roots still work.
#[derive(Debug)]
pub struct HistoryScope {
    kind: OperationKind,
    root: String,
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
        let root = path_text(root)?;
        if files.is_empty() || files.len() > MAX_SCOPE_FILES {
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
            targets.push(
                path_text(&Path::new(&root).join(file)).map_err(|_| WriteError::InvalidRequest)?,
            );
        }
        targets.sort();
        if targets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WriteError::InvalidRequest);
        }
        let bytes = serde_json::to_vec(&(kind.as_str(), &targets))
            .map_err(|_| WriteError::InvalidRequest)?;
        Ok(Self {
            kind,
            root,
            targets,
            digest: digest_hex(&bytes),
        })
    }

    /// Exact configuration root this scope was built for, as printable text.
    ///
    /// Adapter-owned records such as Grok alias bindings are keyed by this
    /// same root text, so history enrichment can look them up without
    /// reconstructing a path from a target file.
    pub(crate) fn root(&self) -> &str {
        &self.root
    }

    /// Operation kind this scope was built for.
    pub(super) fn operation_kind(&self) -> OperationKind {
        self.kind
    }

    /// Whether `target` is exactly one of the scope's owned files.
    ///
    /// Textual, not a path-prefix or filesystem-identity comparison, matching
    /// how the history reader admits an operation.
    pub(super) fn owns_target(&self, target: &str) -> bool {
        self.targets.iter().any(|owned| owned == target)
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
    /// A separately confirmed legacy restore attempt.
    Restore,
    /// A failed apply's automatic compensation attempt.
    Compensation,
    /// A direct restore of any retained version (H2), stored as a restore
    /// revision with `direct = 1`.
    VersionRestore,
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
    /// Real private-store check for the legacy step copy, when one was recorded.
    pub backup_state: RevisionBackupState,
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
    /// Real private-store check; a recorded reference alone is never "available".
    pub backup_state: RevisionBackupState,
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

/// How the live configuration relates to retained history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryCurrentStatus {
    /// Every scoped file matches one retained version completely.
    Matched,
    /// Files are readable but no complete retained version matches.
    Unrecorded,
    /// Some files match a candidate while others do not; never pin a partial set.
    Partial,
    /// At least one scoped file could not be read safely.
    Unreadable,
}

/// One live scoped file observation; digests only, never bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCurrentFile {
    /// Exact owned target path from the trusted scope.
    pub target_path: String,
    /// Live digest, or null when the file is absent (and still readable as such).
    pub current_digest: Option<String>,
    /// False for symlinks, oversized files, or other unsafe reads.
    pub readable: bool,
}

/// Fixed current-config summary, independent of history pagination.
///
/// Always returned with every page so the interface can pin it above the list.
/// Matching compares live digests to retained previous/intended digests; it does
/// not treat the newest apply/restore as current by itself.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCurrentConfig {
    /// Overall relationship of the live files to retained history.
    pub status: HistoryCurrentStatus,
    /// Single history entry that fully matches, only when `status` is `matched`.
    pub matched_entry_key: Option<String>,
    /// Per-file live observation for every scoped target, in scope order.
    pub files: Vec<HistoryCurrentFile>,
    /// Matched Provider's user-custom display name, when that row still exists.
    ///
    /// Populated only by the Agent history surface after a full match. The core
    /// journal matcher leaves this null. Never a joined "provider | model" string.
    pub provider_label: Option<String>,
    /// Model id read from the matched live config files, when known.
    ///
    /// Populated only by the Agent history surface after a full match, for
    /// adapters that read their model identity from the file. Null for
    /// unmatched, partial, or unreadable pins, when the live files do not yield
    /// a valid id, and for Grok Build, which reports its last alias write in a
    /// separate field instead of reading `[models].default`.
    pub model_id: Option<String>,
    /// Model alias when the saved model still has one; otherwise the model id.
    ///
    /// This is the right-hand value for list cards (`ownerDisplayName |
    /// modelAliasOrId`). Null whenever `model_id` is null. Never pre-joined with
    /// [`Self::provider_label`].
    pub model_label: Option<String>,
}

/// Bounded read result. No total is inferred from a changing journal.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    /// Live configuration pin; identical for every page of the same scope.
    pub current: HistoryCurrentConfig,
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
/// frozen export. Newer insertions do not shift the seek position. Private
/// backup files are opened only to report [`RevisionBackupState`]; their paths
/// and bytes never enter the response. No journal rows are modified.
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
    backups: &BackupStore,
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
        entries.push(read_entry(&transaction, scope, candidate, backups)?);
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
    let current = identify_current(&transaction, scope)?;
    transaction
        .commit()
        .map_err(|_| WriteError::LogReadFailed)?;
    Ok(HistoryPage {
        current,
        entries,
        next_cursor,
    })
}

/// Upper bound when scanning the journal to identify the current version.
const MAX_CURRENT_SCAN: u32 = 10_000;

/// Full previous and intended digest maps for one history entry, when complete.
type EntryVersionDigests = (
    Option<BTreeMap<String, Option<String>>>,
    Option<BTreeMap<String, Option<String>>>,
    HistoryIntent,
);

/// Observe live digests and find at most one fully matching retained version.
fn identify_current(
    connection: &Connection,
    scope: &HistoryScope,
) -> Result<HistoryCurrentConfig, WriteError> {
    let files = observe_current_files(scope);
    if files.iter().any(|file| !file.readable) {
        return Ok(HistoryCurrentConfig {
            status: HistoryCurrentStatus::Unreadable,
            matched_entry_key: None,
            files,
            provider_label: None,
            model_id: None,
            model_label: None,
        });
    }
    let live: BTreeMap<&str, Option<&str>> = files
        .iter()
        .map(|file| (file.target_path.as_str(), file.current_digest.as_deref()))
        .collect();
    let candidates = read_candidates(connection, scope, None, MAX_CURRENT_SCAN)?;
    let mut previous_matches: Vec<(i64, String, HistoryIntent)> = Vec::new();
    let mut intended_matches: Vec<(i64, String, HistoryIntent)> = Vec::new();
    let mut saw_partial = false;
    for candidate in candidates {
        // A damaged older row must not hide the current page. Pagination still
        // fails closed when that row is selected; matching simply ignores it.
        let Ok((previous, intended, intent)) = entry_version_digests(connection, scope, &candidate)
        else {
            continue;
        };
        let previous_full = previous.as_ref().is_some_and(|map| maps_equal(map, &live));
        let intended_full = intended.as_ref().is_some_and(|map| maps_equal(map, &live));
        if previous_full {
            previous_matches.push((candidate.created_at, candidate.entry_key.clone(), intent));
        } else if intended_full {
            intended_matches.push((candidate.created_at, candidate.entry_key.clone(), intent));
        } else if is_partial(&previous, &intended, &live) {
            saw_partial = true;
        }
    }
    let matched_entry_key = pick_matched_entry(&previous_matches, &intended_matches);
    let status = if matched_entry_key.is_some() {
        HistoryCurrentStatus::Matched
    } else if saw_partial {
        HistoryCurrentStatus::Partial
    } else {
        HistoryCurrentStatus::Unrecorded
    };
    Ok(HistoryCurrentConfig {
        status,
        matched_entry_key,
        files,
        provider_label: None,
        model_id: None,
        model_label: None,
    })
}

fn observe_current_files(scope: &HistoryScope) -> Vec<HistoryCurrentFile> {
    scope
        .targets
        .iter()
        .map(|path| match super::read_target(Path::new(path)) {
            Ok(target) if !target.symlink => HistoryCurrentFile {
                target_path: path.clone(),
                current_digest: target.content_digest,
                readable: true,
            },
            _ => HistoryCurrentFile {
                target_path: path.clone(),
                current_digest: None,
                readable: false,
            },
        })
        .collect()
}

fn maps_equal(
    candidate: &BTreeMap<String, Option<String>>,
    live: &BTreeMap<&str, Option<&str>>,
) -> bool {
    if candidate.len() != live.len() {
        return false;
    }
    live.iter().all(|(path, digest)| {
        candidate
            .get(*path)
            .is_some_and(|value| value.as_deref() == *digest)
    })
}

fn is_partial(
    previous: &Option<BTreeMap<String, Option<String>>>,
    intended: &Option<BTreeMap<String, Option<String>>>,
    live: &BTreeMap<&str, Option<&str>>,
) -> bool {
    for map in [previous, intended].into_iter().flatten() {
        let mut matched = 0usize;
        let mut differed = 0usize;
        for (path, digest) in live {
            match map.get(*path) {
                Some(value) if value.as_deref() == *digest => matched += 1,
                Some(_) => differed += 1,
                None => {}
            }
        }
        if matched > 0 && (differed > 0 || matched < live.len()) {
            return true;
        }
    }
    false
}

fn pick_matched_entry(
    previous: &[(i64, String, HistoryIntent)],
    intended: &[(i64, String, HistoryIntent)],
) -> Option<String> {
    // Prefer a restorable snapshot match (previous). Among those, prefer the
    // original apply identity, then the oldest entry so restoring B pins B.
    if let Some((_, key, _)) = select_match(previous, true) {
        return Some(key);
    }
    // Otherwise the latest write whose intended digests still hold.
    select_match(intended, false).map(|(_, key, _)| key)
}

fn select_match(
    matches: &[(i64, String, HistoryIntent)],
    oldest: bool,
) -> Option<(i64, String, HistoryIntent)> {
    if matches.is_empty() {
        return None;
    }
    let apply_only: Vec<_> = matches
        .iter()
        .filter(|(_, key, intent)| *intent == HistoryIntent::Apply && key.starts_with("operation:"))
        .cloned()
        .collect();
    let pool = if apply_only.is_empty() {
        matches.to_vec()
    } else {
        apply_only
    };
    pool.into_iter().reduce(|left, right| {
        let take_left = if oldest {
            (left.0, &left.1) <= (right.0, &right.1)
        } else {
            (left.0, &left.1) >= (right.0, &right.1)
        };
        if take_left { left } else { right }
    })
}

/// Full previous/intended digest maps when the entry covers every scoped file.
fn entry_version_digests(
    connection: &Connection,
    scope: &HistoryScope,
    candidate: &Candidate,
) -> Result<EntryVersionDigests, WriteError> {
    if !valid_entry_key(&candidate.entry_key) || !shared::is_random_id(&candidate.operation_id) {
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
    let (intent, stored) = read_revisions(connection, &source, batch)?;
    let previous_from_revisions = complete_map(
        scope,
        stored.iter().map(|row| {
            (
                row.target_path.clone(),
                row.revision.previous_digest.clone(),
            )
        }),
    );
    let intended_from_revisions = complete_map(
        scope,
        stored.iter().map(|row| {
            (
                row.target_path.clone(),
                row.revision.intended_digest.clone(),
            )
        }),
    );
    if batch.is_some() {
        return Ok((previous_from_revisions, intended_from_revisions, intent));
    }
    // Legacy apply rows without revisions still expose expected/applied digests.
    let previous_from_steps = complete_map(
        scope,
        source
            .steps
            .iter()
            .map(|step| (step.target_path.clone(), step.expected_digest.clone())),
    );
    let intended_from_steps =
        if source.steps.iter().all(|step| {
            step.state == StepState::Applied && scope.targets.contains(&step.target_path)
        }) {
            complete_map(
                scope,
                source
                    .steps
                    .iter()
                    .map(|step| (step.target_path.clone(), step.applied_digest.clone())),
            )
        } else {
            None
        };
    Ok((
        previous_from_revisions.or(previous_from_steps),
        intended_from_revisions.or(intended_from_steps),
        intent,
    ))
}

fn complete_map(
    scope: &HistoryScope,
    pairs: impl IntoIterator<Item = (String, Option<String>)>,
) -> Option<BTreeMap<String, Option<String>>> {
    let mut map = BTreeMap::new();
    for (path, digest) in pairs {
        if !scope.targets.contains(&path) || map.contains_key(&path) {
            return None;
        }
        map.insert(path, digest);
    }
    if map.len() != scope.targets.len() {
        return None;
    }
    Some(map)
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
    backups: &BackupStore,
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
    let (intent, stored) = read_revisions(connection, &source, batch)?;
    let updated_at = stored
        .iter()
        .map(|row| row.revision.updated_at)
        .fold(source.updated_at, i64::max);
    let mut source_steps = Vec::with_capacity(source.steps.len());
    for step in source.steps {
        let backup_recorded = step.backup.is_some();
        let backup_state = match &step.backup {
            Some(backup) => recorded_backup_state(
                backups,
                &backup.id,
                &step.target_path,
                Some(backup.path.as_str()),
                Some(backup.content_digest.as_str()),
            ),
            None => RevisionBackupState::NotNeeded,
        };
        source_steps.push(HistorySourceStep {
            ordinal: step.ordinal,
            target_path: step.target_path,
            expected_digest: step.expected_digest,
            applied_digest: step.applied_digest,
            previous_existed: step.previous_existed,
            backup_recorded,
            backup_state,
            state: step.state,
            error_code: step.error_code.as_deref().map(parse_error).transpose()?,
            restore_state: step.restore_state,
        });
    }
    let revisions = stored
        .into_iter()
        .map(|row| {
            let backup_state = recorded_backup_state(
                backups,
                &row.revision.revision_id,
                &row.target_path,
                row.backup_path.as_deref(),
                row.revision.previous_digest.as_deref(),
            );
            HistoryRevision {
                backup_state,
                ..row.revision
            }
        })
        .collect();
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

/// Validated journal row plus the private fields version restore needs.
///
/// `target_path` and `backup_path` never enter an IPC response; the history
/// reader strips them when it builds [`HistoryRevision`].
pub(super) struct StoredRevision {
    /// Metadata safe to expose.
    pub revision: HistoryRevision,
    /// Exact file this revision describes.
    pub target_path: String,
    /// Private snapshot path recorded with the revision, when one exists.
    pub backup_path: Option<String>,
}

struct RevisionRow {
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
    direct: i64,
}

pub(super) fn read_revisions(
    connection: &Connection,
    source: &OperationRecord,
    batch: Option<&str>,
) -> Result<(HistoryIntent, Vec<StoredRevision>), WriteError> {
    let predicate = if batch.is_some() {
        "batch_id = ?1"
    } else {
        "operation_id = ?1 AND intent = 'apply'"
    };
    let mut query = connection
        .prepare(&format!(
            "SELECT id, batch_id, operation_id, ordinal, intent, target_path,
         previous_digest, next_digest, backup_path, state, error_code, created_at, updated_at,
         direct
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
                Ok(RevisionRow {
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
                    direct: row.get(13)?,
                })
            },
        )
        .map_err(|_| WriteError::LogReadFailed)?;
    let mut revisions: Vec<StoredRevision> = Vec::new();
    let mut intent = None;
    for result in rows {
        let row = result.map_err(|_| WriteError::InvalidStoredOperation)?;
        let stored_intent = HistoryIntent::from_stored(&row.intent)?;
        // `direct` marks a version restore layered on the restore storage
        // intent. Any other combination is a malformed row.
        if !matches!(row.direct, 0 | 1)
            || (row.direct == 1 && stored_intent != HistoryIntent::Restore)
        {
            return Err(WriteError::InvalidStoredOperation);
        }
        let row_intent = if row.direct == 1 {
            HistoryIntent::VersionRestore
        } else {
            stored_intent
        };
        let state = HistoryRevisionState::from_stored(&row.state)?;
        let error_code = row.error_code.as_deref().map(parse_error).transpose()?;
        let source_step = source
            .steps
            .iter()
            .find(|step| step.ordinal == row.ordinal)
            .ok_or(WriteError::InvalidStoredOperation)?;
        if revisions.len() >= MAX_SCOPE_FILES
            || !valid_revision_id(&row.id)
            || !valid_revision_id(&row.batch_id)
            || row.operation_id != source.id
            || source_step.target_path != row.target_path
            || row.batch_id != batch.unwrap_or(&source.id)
            || (batch.is_none() && row_intent != HistoryIntent::Apply)
            || (batch.is_some() && row_intent == HistoryIntent::Apply)
            || intent.is_some_and(|previous| previous != row_intent)
            || revisions
                .iter()
                .any(|previous| previous.revision.ordinal == row.ordinal)
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
        let backup_path = row.backup_path.clone();
        revisions.push(StoredRevision {
            revision: HistoryRevision {
                revision_id: row.id,
                ordinal: row.ordinal,
                previous_digest: row.previous_digest,
                intended_digest: row.next_digest,
                backup_recorded: row.backup_path.is_some(),
                // Filled by read_entry after the private-store check.
                backup_state: RevisionBackupState::NotNeeded,
                state,
                resolution,
                error_code,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            target_path: row.target_path,
            backup_path,
        });
    }
    if batch.is_some() && revisions.is_empty() {
        return Err(WriteError::InvalidStoredOperation);
    }
    Ok((intent.unwrap_or(HistoryIntent::Apply), revisions))
}

fn read_resolution(
    connection: &Connection,
    revision: &RevisionRow,
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
        WriteError::HistoryActive,
        WriteError::HistoryUnresolved,
        WriteError::HistoryReferenced,
    ]
    .into_iter()
    .find(|error| error.code() == code)
    .ok_or(WriteError::InvalidStoredOperation)
}

pub(super) fn valid_revision_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn valid_entry_key(key: &str) -> bool {
    if let Some(id) = key.strip_prefix("operation:") {
        shared::is_random_id(id)
    } else {
        key.strip_prefix("batch:").is_some_and(valid_revision_id)
    }
}
