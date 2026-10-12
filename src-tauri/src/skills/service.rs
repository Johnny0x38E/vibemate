//! Import local skill directories into the managed library and record sources.
//!
//! The user's directories are only read. Content is copied into a temporary
//! sibling directory and renamed into place, so an interrupted import never
//! leaves a half-copied skill under a name that a record points at. No command
//! from a skill directory is ever executed.

use super::SkillLibrary;
use super::manifest::{self, SourcePlan};
use super::repository::{self, NewSkill};
use super::types::{
    ImportFolderReport, ImportFolderRequest, SetSkillSourceRequest, SkillError, SkillRecord,
    SkippedSkill,
};
use super::validation;
use crate::storage::Storage;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Domain prefix of the digest stream. Changing the layout requires a new value,
/// so digests from an older scheme never compare equal by accident.
const DIGEST_DOMAIN: &[u8] = b"vibemate-skill-v1\0";
/// Temporary directories are hidden and never match a 32-character ID.
const TEMPORARY_PREFIX: &str = ".import-";

/// Import every skill inside one chosen folder.
///
/// The folder itself is imported when it is a skill; otherwise each immediate
/// subfolder that is a skill is imported. A folder that cannot be imported never
/// stops the others: the report says what was added, and names the refused
/// folders with a safe reason code the interface translates.
///
/// # Errors
/// Rejects a path that cannot be a source, a source that holds no skill, and one
/// that holds more candidate skills than a single import copies. A failure never
/// changes any source directory.
pub fn import_folder(
    library: &SkillLibrary,
    storage: &Storage,
    request: ImportFolderRequest,
    now_ms: i64,
) -> Result<ImportFolderReport, SkillError> {
    if now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    let source = resolve_source(library, &request.path)?;
    let candidates = manifest::discover(&source)?;
    if candidates.is_empty() {
        return Err(SkillError::NoSkillsFound);
    }
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        match import_one(library, storage, &candidate, now_ms) {
            Ok(record) => imported.push(record),
            Err(code) => skipped.push(SkippedSkill {
                folder: validation::folder_label(&candidate),
                code,
            }),
        }
    }
    Ok(ImportFolderReport { imported, skipped })
}

/// Import one directory that was already discovered as a skill.
///
/// # Errors
/// Rejects an unreadable or changing source, a missing or invalid manifest, a
/// symbolic link or special file, every size limit, an existing name, and
/// storage failures. A failure never changes the source directory.
pub(super) fn import_one(
    library: &SkillLibrary,
    storage: &Storage,
    requested: &Path,
    now_ms: i64,
) -> Result<SkillRecord, SkillError> {
    if now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    // Canonicalizing resolves a linked source directory into the real one, so the
    // managed copy and the recorded origin path describe the same files.
    let source = fs::canonicalize(requested).map_err(|_| SkillError::SourceNotFound)?;
    if !fs::metadata(&source)
        .map_err(|_| SkillError::SourceNotFound)?
        .is_dir()
    {
        return Err(SkillError::SourceNotDirectory);
    }
    // Compare canonical paths: a platform temporary directory is often reached
    // through a link, and the library directory may not exist yet on first use.
    let library_root =
        fs::canonicalize(library.root()).unwrap_or_else(|_| library.root().to_path_buf());
    if source.starts_with(library_root) {
        return Err(SkillError::SourcePathInvalid);
    }
    let plan = manifest::inspect(&source)?;
    // Checked before copying so a duplicate name fails without writing files; the
    // insert repeats the check inside its transaction for the concurrent case.
    if repository::name_exists(storage, &plan.name)? {
        return Err(SkillError::NameTaken);
    }
    let origin_path = source
        .to_str()
        .ok_or(SkillError::SourcePathInvalid)?
        .to_string();
    let id = repository::random_id(storage)?;
    let root = library.root();
    fs::create_dir_all(root).map_err(|_| SkillError::WriteFailed)?;
    let temporary = root.join(format!("{TEMPORARY_PREFIX}{id}"));
    let target = root.join(&id);
    fs::create_dir(&temporary).map_err(|_| SkillError::WriteFailed)?;
    let digest = match copy(&source, &plan, &temporary) {
        Ok(digest) => digest,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };
    if fs::rename(&temporary, &target).is_err() {
        let _ = fs::remove_dir_all(&temporary);
        return Err(SkillError::WriteFailed);
    }
    let new = NewSkill {
        id: &id,
        name: &plan.name,
        description: &plan.description,
        origin_path: &origin_path,
        content_digest: &digest,
        file_count: plan.files.len() as i64,
        byte_size: plan.total_bytes as i64,
    };
    match repository::insert_skill(storage, &new, now_ms) {
        Ok(()) => repository::get_skill(storage, &id),
        Err((error, uncertain)) => {
            // Without a stored record the copied directory is unreachable, so it is
            // removed. If either step is uncertain, the caller must reload.
            let removed = fs::remove_dir_all(&target).is_ok();
            Err(if removed && !uncertain {
                error
            } else {
                SkillError::OutcomeUnknown
            })
        }
    }
}

/// Validate a chosen folder path and return the canonical directory.
fn resolve_source(library: &SkillLibrary, value: &str) -> Result<PathBuf, SkillError> {
    let requested = validation::source_path(value)?;
    let source = fs::canonicalize(&requested).map_err(|_| SkillError::SourceNotFound)?;
    if !fs::metadata(&source)
        .map_err(|_| SkillError::SourceNotFound)?
        .is_dir()
    {
        return Err(SkillError::SourceNotDirectory);
    }
    let library_root =
        fs::canonicalize(library.root()).unwrap_or_else(|_| library.root().to_path_buf());
    if source.starts_with(library_root) {
        return Err(SkillError::SourcePathInvalid);
    }
    Ok(source)
}

/// Record or clear the upstream source of an existing skill.
///
/// The upstream address is metadata only: no download or update check happens
/// here. Clearing it keeps the local origin path, which is the P25 source of
/// truth for an imported directory.
///
/// # Errors
/// Rejects an invalid address or revision, a missing skill, a stale revision and
/// storage failures.
pub fn set_skill_source(
    storage: &Storage,
    request: SetSkillSourceRequest,
    now_ms: i64,
) -> Result<SkillRecord, SkillError> {
    validation::identity(&request.id)?;
    if request.expected_revision < 1 || now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    let url = request
        .url
        .as_deref()
        .map(validation::source_url)
        .transpose()?;
    let revision = request
        .revision
        .as_deref()
        .map(validation::source_revision)
        .transpose()?;
    // A revision alone cannot be interpreted later, so it needs an address.
    if url.is_none() && revision.is_some() {
        return Err(SkillError::InvalidRequest);
    }
    let current = repository::get_skill(storage, &request.id)?;
    // Every record must keep one explanation of where the content came from.
    if url.is_none() && current.origin_path.is_none() {
        return Err(SkillError::InvalidRequest);
    }
    repository::update_source(
        storage,
        &request.id,
        request.expected_revision,
        url,
        revision,
        now_ms,
    )?;
    repository::get_skill(storage, &request.id)
}

/// Copy the planned files and return the digest of exactly those bytes.
///
/// # Errors
/// Reports an unreadable source file, a source that changed after inspection,
/// and write failures inside the temporary directory.
pub(super) fn copy(source: &Path, plan: &SourcePlan, target: &Path) -> Result<String, SkillError> {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    for file in &plan.files {
        let bytes =
            fs::read(source.join(&file.relative)).map_err(|_| SkillError::SourceNotFound)?;
        // The source may change between inspection and copy; a size difference
        // means the digest would describe neither version, so the import stops.
        if bytes.len() as u64 != file.size {
            return Err(SkillError::SourceChanged);
        }
        let destination = target.join(&file.relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| SkillError::WriteFailed)?;
        }
        fs::write(&destination, &bytes).map_err(|_| SkillError::WriteFailed)?;
        set_mode(&destination, file.mode)?;
        // Length prefixes keep the stream unambiguous for any file content.
        hasher.update((file.relative.len() as u64).to_be_bytes());
        hasher.update(file.relative.as_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Preserve ordinary permission bits so bundled scripts stay executable.
#[cfg(unix)]
fn set_mode(path: &Path, mode: Option<u32>) -> Result<(), SkillError> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = mode {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|_| SkillError::WriteFailed)?;
    }
    Ok(())
}

/// Other platforms copy content without a comparable permission model.
#[cfg(not(unix))]
fn set_mode(_: &Path, _: Option<u32>) -> Result<(), SkillError> {
    Ok(())
}
