//! Skill import requests, safe records and stable error codes.

use serde::{Deserialize, Serialize};

/// Safe failures without paths, URLs, file content or platform diagnostics.
///
/// The interface translates these codes; no backend message reaches the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillError {
    /// Private storage is unavailable.
    StorageUnavailable,
    /// Reading persisted metadata failed.
    ReadFailed,
    /// Reading the source or writing the managed copy failed.
    WriteFailed,
    /// The request shape or a field is invalid.
    InvalidRequest,
    /// The source path is relative, empty, too long or inside the library.
    SourcePathInvalid,
    /// The source directory does not exist or cannot be listed.
    SourceNotFound,
    /// The source path exists but is not a directory.
    SourceNotDirectory,
    /// The source contains a symbolic link or a special file.
    SourceUnsupportedEntry,
    /// The source tree exceeds the file count, size or depth limits.
    SourceTooLarge,
    /// The source changed while it was copied; nothing was imported.
    SourceChanged,
    /// The source directory has no `SKILL.md` file.
    ManifestMissing,
    /// `SKILL.md` has no usable YAML frontmatter with a name and description.
    ManifestInvalid,
    /// The chosen folder holds no skill folder and is not a skill itself.
    NoSkillsFound,
    /// Another library entry already uses this skill name.
    NameTaken,
    /// The recorded source URL is not an acceptable HTTP(S) address.
    SourceUrlInvalid,
    /// The recorded revision contains unsupported characters.
    SourceRevisionInvalid,
    /// The skill no longer exists.
    NotFound,
    /// Another writer changed this skill.
    RevisionConflict,
    /// Persisted metadata failed validation and was left unchanged.
    InvalidStoredSkill,
    /// A partial result may exist; reload before another change.
    OutcomeUnknown,
    /// A worker task failed, possibly after a change; reload first.
    OperationFailed,
}

/// Import the skills inside one chosen folder.
///
/// The folder itself is imported when it holds a `SKILL.md`; otherwise every
/// immediate subfolder that holds one is imported.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportFolderRequest {
    /// Absolute path of the folder the user chose in the system picker.
    pub path: String,
}

/// Outcome of the system folder picker; closing it is not an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickedFolder {
    /// The user closed the dialog without choosing a folder.
    Cancelled,
    /// The chosen folder, as an absolute path.
    Selected {
        /// Path reported by the operating system dialog.
        path: String,
    },
}

/// One candidate folder that could not be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSkill {
    /// Folder name only, never the full path.
    pub folder: String,
    /// Safe reason code the interface translates.
    pub code: SkillError,
}

/// What one chosen folder produced: the added skills and the refused folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFolderReport {
    /// Skills that were copied into the library.
    pub imported: Vec<SkillRecord>,
    /// Candidate folders that were refused, each with a safe reason.
    pub skipped: Vec<SkippedSkill>,
}

/// Record or clear the upstream source of an existing skill.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetSkillSourceRequest {
    /// Stable identity of the library entry.
    pub id: String,
    /// Revision the editor loaded, so two writers cannot overwrite each other.
    pub expected_revision: i64,
    /// Repository or documentation URL, or null to clear the upstream source.
    pub url: Option<String>,
    /// Pinned revision such as a tag or commit, or null when unknown.
    pub revision: Option<String>,
}

/// Safe skill metadata; file contents never cross IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRecord {
    /// Stable identity, also the managed directory name.
    pub id: String,
    /// Skill name from the frontmatter.
    pub name: String,
    /// One-line description from the frontmatter.
    pub description: String,
    /// Directory the content was imported from, when it was a local import.
    pub origin_path: Option<String>,
    /// User-recorded upstream source; null means the source is unknown.
    pub source_url: Option<String>,
    /// User-recorded revision of `source_url`.
    pub source_revision: Option<String>,
    /// SHA-256 of the imported content, used as the change baseline.
    pub content_digest: String,
    /// Number of files in the managed copy.
    pub file_count: i64,
    /// Total bytes in the managed copy.
    pub byte_size: i64,
    /// Metadata revision for optimistic concurrency.
    pub revision: i64,
    /// Import timestamp in Unix milliseconds.
    pub imported_at_ms: i64,
    /// Last metadata change in Unix milliseconds.
    pub updated_at_ms: i64,
}

/// One bounded page of skill records, ordered by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPage {
    /// Records in this page.
    pub items: Vec<SkillRecord>,
    /// Cursor for the next page, or null at the end of the list.
    pub next_cursor: Option<String>,
    /// How many skills the library holds in total, so a count stays correct
    /// without reading every page.
    pub total: i64,
}
