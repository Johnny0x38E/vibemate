//! Input and stored-value validation for the central skill library.
//!
//! Every value that reaches SQLite or a filesystem path passes through here, and
//! persisted rows are validated again when they are read, so a manually edited
//! database cannot make the library write outside its own directory.

use super::types::SkillError;
use std::path::{Path, PathBuf};

/// Longest accepted skill name, matching the `skill` table constraint.
pub(super) const MAX_NAME_LENGTH: usize = 64;
/// Longest accepted description, matching the `skill` table constraint.
pub(super) const MAX_DESCRIPTION_LENGTH: usize = 1024;
/// Longest accepted source path. Platform path limits stay below this bound.
pub(super) const MAX_PATH_LENGTH: usize = 4096;
/// Longest accepted source URL, matching the `skill` table constraint.
pub(super) const MAX_URL_LENGTH: usize = 512;
/// Longest accepted folder label in an import report.
pub(super) const MAX_FOLDER_LENGTH: usize = 128;
/// Longest accepted revision string, matching the `skill` table constraint.
pub(super) const MAX_REVISION_LENGTH: usize = 128;

/// Require a 32-character lowercase hexadecimal identity.
pub(super) fn identity(id: &str) -> Result<(), SkillError> {
    if id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(SkillError::InvalidRequest)
    }
}

/// Validate a skill name and return it unchanged.
///
/// Names follow the documented Agent convention: letters, digits, dot, dash and
/// underscore, starting with a letter or digit so no entry looks like a hidden
/// file and no name can be confused with a path.
pub(super) fn skill_name(name: &str) -> Result<&str, SkillError> {
    let length = name.chars().count();
    let first_is_alphanumeric = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric());
    let allowed = name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character));
    if (1..=MAX_NAME_LENGTH).contains(&length) && first_is_alphanumeric && allowed {
        Ok(name)
    } else {
        Err(SkillError::ManifestInvalid)
    }
}

/// Validate a one-line description and return the trimmed value.
pub(super) fn description(text: &str) -> Result<&str, SkillError> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_DESCRIPTION_LENGTH
        || trimmed.chars().any(char::is_control)
    {
        return Err(SkillError::ManifestInvalid);
    }
    Ok(trimmed)
}

/// Validate an absolute source directory path.
///
/// The path is only checked as text here; existence, directory type and the
/// library-boundary check happen after canonicalization in the import service.
pub(super) fn source_path(value: &str) -> Result<PathBuf, SkillError> {
    let invalid =
        value.is_empty() || value.len() > MAX_PATH_LENGTH || value.chars().any(char::is_control);
    let path = PathBuf::from(value);
    if invalid || !path.is_absolute() || path.components().count() < 2 {
        return Err(SkillError::SourcePathInvalid);
    }
    Ok(path)
}

/// Validate a stored origin path without requiring the directory to exist.
pub(super) fn stored_path(value: &str) -> Result<&str, SkillError> {
    let valid = (1..=MAX_PATH_LENGTH).contains(&value.len())
        && !value.chars().any(char::is_control)
        && Path::new(value).is_absolute();
    if valid {
        Ok(value)
    } else {
        Err(SkillError::InvalidStoredSkill)
    }
}

/// Validate an HTTP(S) source URL without opening a connection.
///
/// Credentials in the URL are rejected so a token cannot be stored as metadata.
pub(super) fn source_url(value: &str) -> Result<&str, SkillError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_URL_LENGTH {
        return Err(SkillError::SourceUrlInvalid);
    }
    let parsed = url::Url::parse(trimmed).map_err(|_| SkillError::SourceUrlInvalid)?;
    let accepted_scheme = matches!(parsed.scheme(), "http" | "https");
    let has_host = parsed.host_str().is_some_and(|host| !host.is_empty());
    if !accepted_scheme || !has_host || !parsed.username().is_empty() || parsed.password().is_some()
    {
        return Err(SkillError::SourceUrlInvalid);
    }
    Ok(trimmed)
}

/// Validate a stored source URL, mapping failures to the stored-data error.
pub(super) fn stored_url(value: &str) -> Result<&str, SkillError> {
    source_url(value).map_err(|_| SkillError::InvalidStoredSkill)
}

/// Validate a revision such as a tag, branch name or commit hash.
///
/// Printable non-space ASCII only: revisions are used in messages and in a
/// future fetch plan, never as a path or command argument without review.
pub(super) fn source_revision(value: &str) -> Result<&str, SkillError> {
    let trimmed = value.trim();
    let valid = (1..=MAX_REVISION_LENGTH).contains(&trimmed.chars().count())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_graphic());
    if valid {
        Ok(trimmed)
    } else {
        Err(SkillError::SourceRevisionInvalid)
    }
}

/// Validate a stored revision, mapping failures to the stored-data error.
pub(super) fn stored_revision(value: &str) -> Result<&str, SkillError> {
    source_revision(value).map_err(|_| SkillError::InvalidStoredSkill)
}

/// Validate and split a list cursor: `name` line feed `id`.
///
/// The list is ordered by name and the cursor carries both parts so pagination
/// stays correct when several skills share a prefix or arrive at the same time.
pub(super) fn cursor(value: &str) -> Result<(&str, &str), SkillError> {
    if value.len() > MAX_NAME_LENGTH + 33 {
        return Err(SkillError::InvalidRequest);
    }
    let (name, id) = value.split_once('\n').ok_or(SkillError::InvalidRequest)?;
    skill_name(name).map_err(|_| SkillError::InvalidRequest)?;
    identity(id)?;
    Ok((name, id))
}

/// Build the cursor for the last row of a page.
pub(super) fn cursor_for(name: &str, id: &str) -> String {
    format!("{name}\n{id}")
}

/// Turn a folder name into a display label for an import report.
///
/// The label is only shown in the interface: control characters and path
/// separators are removed so a report line can never look like a full path.
pub(super) fn folder_label(path: &Path) -> String {
    let raw = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let cleaned: String = raw
        .chars()
        .filter(|character| !character.is_control() && *character != '/' && *character != '\\')
        .take(MAX_FOLDER_LENGTH)
        .collect();
    if cleaned.trim().is_empty() {
        // A name made only of separators or control characters has no safe label.
        "…".to_string()
    } else {
        cleaned
    }
}
