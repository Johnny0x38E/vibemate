//! Skill metadata SQL: reads, the import insert and the source update.
//!
//! This module never touches the filesystem: the managed copy is written by the
//! import service, and a failed insert is compensated there.

use super::manifest;
use super::types::{DeleteSkillRequest, FetchMethod, SkillError, SkillPage, SkillRecord};
use super::validation::{
    cursor, cursor_for, description, identity, skill_name, stored_installer_command,
    stored_installer_version, stored_path, stored_revision, stored_url,
};
use crate::storage::Storage;
use rusqlite::{Connection, OptionalExtension, params};

/// Shared with the MCP module: the single connection is the app's write lock.
pub(super) fn lock(storage: &Storage) -> Result<std::sync::MutexGuard<'_, Connection>, SkillError> {
    storage.lock().map_err(|_| SkillError::StorageUnavailable)
}

/// Row values computed by the import service for a new skill.
pub(super) struct NewSkill<'a> {
    /// Stable identity, also the managed directory name.
    pub(super) id: &'a str,
    /// Validated name from the frontmatter.
    pub(super) name: &'a str,
    /// Validated description from the frontmatter.
    pub(super) description: &'a str,
    /// Canonical directory a local import copied from; null for a remote install.
    pub(super) origin_path: Option<&'a str>,
    /// Canonical upstream address recorded by an install.
    pub(super) source_url: Option<&'a str>,
    /// Pinned Git commit, or null for a hand-entered source or npx install.
    pub(super) source_revision: Option<&'a str>,
    /// How the source was fetched, or null for a local import/hand-entered source.
    pub(super) fetch_method: Option<FetchMethod>,
    /// Exact npx installer version, or null for Git/local sources.
    pub(super) installer_version: Option<&'a str>,
    /// Reviewed command that acquired the source.
    pub(super) installer_command: Option<&'a str>,
    /// Digest of the copied content.
    pub(super) content_digest: &'a str,
    /// Number of copied files.
    pub(super) file_count: i64,
    /// Total copied bytes.
    pub(super) byte_size: i64,
}

/// Text stored in `skill.fetch_method`.
fn fetch_method_value(method: FetchMethod) -> &'static str {
    match method {
        FetchMethod::Git => "git",
        FetchMethod::Npx => "npx",
    }
}

/// Parse a stored fetch method; an unknown value is malformed metadata.
fn fetch_method_from_stored(value: &str) -> Option<FetchMethod> {
    match value {
        "git" => Some(FetchMethod::Git),
        "npx" => Some(FetchMethod::Npx),
        _ => None,
    }
}

/// One stored row before validation.
struct StoredRow {
    name: String,
    description: String,
    origin_path: Option<String>,
    source_url: Option<String>,
    source_revision: Option<String>,
    fetch_method: Option<String>,
    installer_version: Option<String>,
    installer_command: Option<String>,
    content_digest: String,
    file_count: i64,
    byte_size: i64,
    revision: i64,
    imported_at: i64,
    updated_at: i64,
}

/// Create a fresh identity for a managed directory name.
pub(super) fn random_id(storage: &Storage) -> Result<String, SkillError> {
    let connection = lock(storage)?;
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| SkillError::WriteFailed)
}

/// Check whether the library already holds this name, before copying anything.
pub(super) fn name_exists(storage: &Storage, name: &str) -> Result<bool, SkillError> {
    let connection = lock(storage)?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skill WHERE name=?1)",
            [name],
            |row| row.get(0),
        )
        .map_err(|_| SkillError::ReadFailed)
}

/// Insert one imported skill and its digest baseline.
///
/// The name clash is checked inside the transaction, so two imports of the same
/// name cannot both succeed. The `bool` reports whether a failure may have left
/// a committed row, which requires the caller to reload before another change.
pub(super) fn insert_skill(
    storage: &Storage,
    new: &NewSkill<'_>,
    now: i64,
) -> Result<(), (SkillError, bool)> {
    let mut connection = lock(storage).map_err(|error| (error, false))?;
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| (SkillError::WriteFailed, false))?;
    let clash: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skill WHERE name=?1)",
            [new.name],
            |row| row.get(0),
        )
        .map_err(|_| (SkillError::WriteFailed, false))?;
    if clash {
        return Err((SkillError::NameTaken, false));
    }
    tx.execute(
        "INSERT INTO skill
         (id, name, description, origin_path, source_url, source_revision,
          fetch_method, installer_version, installer_command,
          content_digest, file_count, byte_size, revision, imported_at, updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,1,?13,?13)",
        params![
            new.id,
            new.name,
            new.description,
            new.origin_path,
            new.source_url,
            new.source_revision,
            new.fetch_method.map(fetch_method_value),
            new.installer_version,
            new.installer_command,
            new.content_digest,
            new.file_count,
            new.byte_size,
            now
        ],
    )
    .map_err(|_| (SkillError::WriteFailed, false))?;
    tx.commit().map_err(|_| (SkillError::OutcomeUnknown, true))
}

/// Source fields one reviewed edit replaces together.
pub(super) struct SourceUpdate<'a> {
    pub(super) url: Option<&'a str>,
    pub(super) revision: Option<&'a str>,
    pub(super) fetch_method: Option<FetchMethod>,
    pub(super) installer_version: Option<&'a str>,
    pub(super) installer_command: Option<&'a str>,
}

/// Replace the recorded upstream source at the expected revision.
///
/// # Errors
/// Reports a missing row and a stale revision separately so the interface can
/// tell "reload the list" from "this skill was removed".
pub(super) fn update_source(
    storage: &Storage,
    id: &str,
    expected_revision: i64,
    update: &SourceUpdate<'_>,
    now: i64,
) -> Result<(), SkillError> {
    let connection = lock(storage)?;
    let changed = connection
        .execute(
            "UPDATE skill
             SET source_url=?1, source_revision=?2, fetch_method=?3,
                 installer_version=?4, installer_command=?5,
                 revision=revision+1, updated_at=max(updated_at,?6)
             WHERE id=?7 AND revision=?8",
            params![
                update.url,
                update.revision,
                update.fetch_method.map(fetch_method_value),
                update.installer_version,
                update.installer_command,
                now,
                id,
                expected_revision
            ],
        )
        .map_err(|_| SkillError::WriteFailed)?;
    if changed == 1 {
        return Ok(());
    }
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skill WHERE id=?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| SkillError::ReadFailed)?;
    Err(if exists {
        SkillError::RevisionConflict
    } else {
        SkillError::NotFound
    })
}

/// Remove one skill's row at the reviewed revision, inside one transaction.
///
/// The row is read again before the delete, so a record that changed since the
/// review is refused as `RevisionConflict` and one that is already gone as
/// `NotFound`. A failed commit may or may not have removed the row, so it is
/// reported as `OutcomeUnknown` and the caller must reload.
pub(super) fn commit_skill_delete(
    storage: &Storage,
    request: &DeleteSkillRequest,
) -> Result<(), SkillError> {
    let mut connection = lock(storage)?;
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| SkillError::WriteFailed)?;
    let record = read_record(&tx, &request.id)?;
    if record.revision != request.expected_revision {
        return Err(SkillError::RevisionConflict);
    }
    // The second revision check keeps the delete bound to what was reviewed even
    // if the read above and this statement ever run under separate writers.
    tx.execute(
        "DELETE FROM skill WHERE id=?1 AND revision=?2",
        params![request.id, request.expected_revision],
    )
    .map_err(|_| SkillError::WriteFailed)?;
    tx.commit().map_err(|_| SkillError::OutcomeUnknown)
}

/// Whether the library still holds a record for this identity.
///
/// Used by the parked-copy cleanup to tell a finished delete from an interrupted
/// one whose record is still there.
pub(super) fn skill_exists(storage: &Storage, id: &str) -> Result<bool, SkillError> {
    let connection = lock(storage)?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skill WHERE id=?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| SkillError::ReadFailed)
}

/// Read one skill's safe metadata.
pub(super) fn read_record(connection: &Connection, id: &str) -> Result<SkillRecord, SkillError> {
    let row = read_row(connection, id)?.ok_or(SkillError::NotFound)?;
    validate_row(row, id)
}

/// Read one skill's safe metadata without reading any file content.
///
/// # Errors
/// Rejects an invalid identity, a missing row and invalid stored metadata.
pub fn get_skill(storage: &Storage, id: &str) -> Result<SkillRecord, SkillError> {
    identity(id)?;
    let connection = lock(storage)?;
    read_record(&connection, id)
}

/// Read one bounded page (1–100 rows) ordered by name.
///
/// # Errors
/// Rejects invalid cursors or limits and reports storage and read failures.
pub fn list_skills(
    storage: &Storage,
    after: Option<&str>,
    limit: u32,
) -> Result<SkillPage, SkillError> {
    if !(1..=100).contains(&limit) {
        return Err(SkillError::InvalidRequest);
    }
    let cursor = after.map(cursor).transpose()?;
    let (after_name, after_id) = match cursor {
        Some((name, id)) => (Some(name), Some(id)),
        None => (None, None),
    };
    let connection = lock(storage)?;
    let total: i64 = connection
        .query_row("SELECT COUNT(*) FROM skill", [], |row| row.get(0))
        .map_err(|_| SkillError::ReadFailed)?;
    let mut statement = connection
        .prepare(
            "SELECT id FROM skill
             WHERE (?1 IS NULL OR (name, id) > (?1, ?2))
             ORDER BY name, id LIMIT ?3",
        )
        .map_err(|_| SkillError::ReadFailed)?;
    let ids: Vec<String> = statement
        .query_map(params![after_name, after_id, limit + 1], |row| row.get(0))
        .map_err(|_| SkillError::ReadFailed)?
        .collect::<Result<_, _>>()
        .map_err(|_| SkillError::ReadFailed)?;
    // One extra row only tells whether another page exists; the cursor is the
    // last row that is actually returned, so no row can be skipped.
    let has_more = ids.len() > limit as usize;
    let items = ids
        .iter()
        .take(limit as usize)
        .map(|id| read_record(&connection, id))
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        items.last().map(|row| cursor_for(&row.name, &row.id))
    } else {
        None
    };
    Ok(SkillPage {
        items,
        next_cursor,
        total,
    })
}

fn read_row(connection: &Connection, id: &str) -> Result<Option<StoredRow>, SkillError> {
    connection
        .query_row(
            "SELECT name, description, origin_path, source_url, source_revision,
                    fetch_method, installer_version, installer_command,
                    content_digest, file_count, byte_size, revision, imported_at, updated_at
             FROM skill WHERE id=?1",
            [id],
            |row| {
                Ok(StoredRow {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    origin_path: row.get(2)?,
                    source_url: row.get(3)?,
                    source_revision: row.get(4)?,
                    fetch_method: row.get(5)?,
                    installer_version: row.get(6)?,
                    installer_command: row.get(7)?,
                    content_digest: row.get(8)?,
                    file_count: row.get(9)?,
                    byte_size: row.get(10)?,
                    revision: row.get(11)?,
                    imported_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            },
        )
        .optional()
        .map_err(|_| SkillError::ReadFailed)
}

/// Validate a stored row with the same rules as fresh input.
fn validate_row(row: StoredRow, id: &str) -> Result<SkillRecord, SkillError> {
    skill_name(&row.name).map_err(|_| SkillError::InvalidStoredSkill)?;
    description(&row.description).map_err(|_| SkillError::InvalidStoredSkill)?;
    if let Some(path) = &row.origin_path {
        stored_path(path)?;
    }
    if let Some(url) = &row.source_url {
        stored_url(url)?;
    }
    if let Some(revision) = &row.source_revision {
        stored_revision(revision)?;
    }
    let digest_is_sha256 = row.content_digest.len() == 64
        && row
            .content_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let bounds_hold = (1..=manifest::MAX_FILES as i64).contains(&row.file_count)
        && (1..=manifest::MAX_TOTAL_BYTES as i64).contains(&row.byte_size);
    let timestamps_hold =
        row.imported_at >= 0 && row.updated_at >= row.imported_at && row.revision >= 1;
    // The table enforces both invariants; a manually edited file may not.
    let source_exists = row.origin_path.is_some() || row.source_url.is_some();
    let revision_belongs_to_url = row.source_url.is_some() || row.source_revision.is_none();
    let fetch_method = row
        .fetch_method
        .as_deref()
        .map(|value| fetch_method_from_stored(value).ok_or(SkillError::InvalidStoredSkill))
        .transpose()?;
    // A method and its command only make sense with an upstream address. A
    // hand-entered address has no method until the user records one explicitly.
    let fetch_fields_consistent = match fetch_method {
        None => row.installer_version.is_none() && row.installer_command.is_none(),
        Some(FetchMethod::Git) => {
            row.source_url.is_some()
                && row.source_revision.is_some()
                && row.installer_version.is_none()
                && row.installer_command.is_some()
        }
        Some(FetchMethod::Npx) => {
            row.source_url.is_some()
                && row.source_revision.is_none()
                && row.installer_version.is_some()
                && row.installer_command.is_some()
        }
    };
    if let Some(version) = &row.installer_version {
        stored_installer_version(version)?;
    }
    if let Some(command) = &row.installer_command {
        stored_installer_command(command)?;
    }
    if !digest_is_sha256
        || !bounds_hold
        || !timestamps_hold
        || !source_exists
        || !revision_belongs_to_url
        || !fetch_fields_consistent
    {
        return Err(SkillError::InvalidStoredSkill);
    }
    Ok(SkillRecord {
        id: id.to_string(),
        name: row.name,
        description: row.description,
        origin_path: row.origin_path,
        source_url: row.source_url,
        source_revision: row.source_revision,
        fetch_method,
        installer_version: row.installer_version,
        installer_command: row.installer_command,
        content_digest: row.content_digest,
        file_count: row.file_count,
        byte_size: row.byte_size,
        revision: row.revision,
        imported_at_ms: row.imported_at,
        updated_at_ms: row.updated_at,
    })
}
