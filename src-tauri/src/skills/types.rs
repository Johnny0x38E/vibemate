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
    /// The source is not a GitHub repository address this installer accepts.
    SourceRepositoryInvalid,
    /// The recorded revision contains unsupported characters.
    SourceRevisionInvalid,
    /// A Git revision contains characters this installer does not accept.
    GitRevisionInvalid,
    /// A Git install needs a pinned revision.
    SourceRevisionRequired,
    /// The reviewed npx installer version is not an exact semantic version.
    InstallerVersionInvalid,
    /// The Git executable or the reviewed npx installer is not available.
    InstallerUnavailable,
    /// The external installer exited unsuccessfully.
    InstallerFailed,
    /// The external installer produced output this module cannot interpret.
    InstallerOutputInvalid,
    /// The external installer did not finish within its time limit.
    InstallTimedOut,
    /// A private temporary workspace could not be created or read.
    InstallWorkspaceFailed,
    /// The selected skill name does not exist in the acquired source.
    SkillNotFoundInSource,
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
    /// The skill still has an Agent deployment; revoke it before deleting.
    SkillInUse,
    /// The Agent configuration root cannot be resolved or written.
    AgentLocationUnavailable,
    /// An unmanaged path already occupies the deploy target.
    TargetConflict,
    /// The target changed after the preview the user reviewed.
    TargetChanged,
    /// Symlink mode was requested but the platform cannot create one here.
    SymlinkUnavailable,
    /// The requested deploy mode is not allowed for this preview.
    DeployModeInvalid,
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

/// How a recorded source was fetched, and how a later update will fetch it.
///
/// `None` on a record means the address was entered by hand and no update
/// method is known. The interface must not present that as auto-updatable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchMethod {
    /// A GitHub repository fetched by Git at a pinned commit.
    Git,
    /// The reviewed `skills` npx installer at an exact package version.
    Npx,
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
    /// Fetch method for a later update check, or null when it is unknown.
    pub fetch_method: Option<FetchMethod>,
    /// Exact npx installer version; only valid with [`FetchMethod::Npx`].
    pub installer_version: Option<String>,
}

/// A GitHub repository reviewed before a Git install.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitInstallSource {
    /// GitHub repository URL or `owner/repo` shorthand.
    pub repository: String,
    /// Tag, branch or commit to fetch. The resolved commit is recorded.
    pub revision: String,
    /// One skill name, `*`, or null for every discovered skill.
    pub skill: Option<String>,
}

/// A source reviewed for the allowlisted `skills` npx installer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpxInstallSource {
    /// GitHub repository URL or `owner/repo` shorthand.
    pub source: String,
    /// Exact installer version; null selects the version reviewed by vibemate.
    pub installer_version: Option<String>,
    /// One skill name, `*`, or null for every discovered skill.
    pub skill: Option<String>,
}

/// The two acquisition paths P26 supports.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "method", content = "source", rename_all = "snake_case")]
pub enum SkillInstallSource {
    /// Fetch a pinned GitHub revision with the installed Git executable.
    Git(GitInstallSource),
    /// Run the reviewed installer inside an isolated home directory.
    Npx(NpxInstallSource),
}

/// What the interface shows before an install may run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallPreview {
    /// Acquisition path the reviewed command uses.
    pub method: FetchMethod,
    /// Canonical GitHub repository address.
    pub source_url: String,
    /// Requested Git revision, or null for the npx installer.
    pub revision: Option<String>,
    /// Executable or package that acquires the source.
    pub installer: String,
    /// Exact npx installer version, or null for Git.
    pub installer_version: Option<String>,
    /// Exact command shown to the user before execution. It is display text,
    /// never a shell program built from unchecked input.
    pub command: String,
}

/// What one remote install produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallReport {
    /// Acquisition path that produced the result.
    pub method: FetchMethod,
    /// Canonical GitHub repository address.
    pub source_url: String,
    /// Resolved commit for Git, or null for the npx installer.
    pub revision: Option<String>,
    /// Executable or package that acquired the source.
    pub installer: String,
    /// Exact npx installer version, or null for Git.
    pub installer_version: Option<String>,
    /// Command that ran, using the same reviewed form as the preview.
    pub command: String,
    /// Process exit code; a non-zero code becomes `installer_failed`.
    pub exit_status: i32,
    /// Skills copied into the central library.
    pub imported: Vec<SkillRecord>,
    /// Candidate folders that were refused, each with a safe reason.
    pub skipped: Vec<SkippedSkill>,
}

/// Revision-checked removal of one library entry and its managed copy.
///
/// The user's imported folder is never part of this request and is never
/// changed; only vibemate's own copy is removed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteSkillRequest {
    /// Stable identity of the library entry.
    pub id: String,
    /// Revision the user reviewed before confirming deletion.
    pub expected_revision: i64,
}

/// Confirmed removal of one skill; parked files may still need a retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedSkill {
    /// Identity of the removed library entry.
    pub id: String,
    /// Whether copied files remain parked until an explicit cleanup retry.
    pub cleanup_pending: bool,
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
    /// How the source was fetched, or null when only an address is known.
    pub fetch_method: Option<FetchMethod>,
    /// Exact npx installer version recorded by an npx install.
    pub installer_version: Option<String>,
    /// Command that was reviewed and run for this source.
    pub installer_command: Option<String>,
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

/// How a skill is linked into an Agent skills directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDeployMode {
    /// Directory symlink to the managed library copy.
    Symlink,
    /// Independent directory copy of the managed content.
    Copy,
}

/// Filesystem classification of the deploy target at preview time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDeployTargetState {
    /// No file or directory at the target path.
    Absent,
    /// Vibemate-owned symlink to the current managed copy.
    OwnedSymlink,
    /// Vibemate-owned directory copy of the managed content.
    OwnedCopy,
    /// An unmanaged path occupies the target; vibemate will not overwrite it.
    Foreign,
}

/// Preview of deploying one central skill into Pi's user skills directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiSkillDeployPreview {
    /// Central skill identity.
    pub skill_id: String,
    /// Skill name used as the target directory name.
    pub skill_name: String,
    /// Current library revision the preview was built from.
    pub skill_revision: i64,
    /// Content digest that would be recorded on apply.
    pub content_digest: String,
    /// Absolute Pi agent directory used for this preview.
    pub config_root: String,
    /// Absolute target path under `<config_root>/skills/<name>/`.
    pub target_path: String,
    /// Directory name under `skills/`, equal to `skill_name`.
    pub target_name: String,
    /// What currently occupies the target path.
    pub target_state: SkillDeployTargetState,
    /// Whether a directory symlink can be created under the skills parent.
    pub symlink_available: bool,
    /// Preferred mode when apply is allowed: symlink when available, else copy.
    pub suggested_mode: SkillDeployMode,
    /// False when the target is foreign or the agent root is unusable.
    pub can_apply: bool,
}

/// Tokens from a reviewed Pi skill deploy preview.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedPiSkillDeploy {
    /// Central skill identity the preview named.
    pub skill_id: String,
    /// Library revision the user reviewed.
    pub expected_skill_revision: i64,
    /// Absolute Pi agent directory from the preview.
    pub config_root: String,
    /// Deploy mode the user confirmed.
    pub mode: SkillDeployMode,
    /// Target classification the preview reported.
    pub target_state: SkillDeployTargetState,
}

/// Result of applying a reviewed Pi skill deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiSkillDeployed {
    /// Central skill identity.
    pub skill_id: String,
    /// Skill name at the target.
    pub skill_name: String,
    /// Library revision recorded with the deployment.
    pub skill_revision: i64,
    /// Mode that was written.
    pub mode: SkillDeployMode,
    /// Absolute target path that was created or replaced.
    pub target_path: String,
}

/// Request to revoke one vibemate-managed Pi skill deployment.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisablePiSkillDeployRequest {
    /// Central skill identity.
    pub skill_id: String,
    /// Absolute Pi agent directory that holds the deployment.
    pub config_root: String,
}

/// Outcome of disabling a Pi skill deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiSkillDeployDisabled {
    /// Central skill identity.
    pub skill_id: String,
    /// Absolute target path that was reviewed.
    pub target_path: String,
    /// Whether the managed filesystem target was removed.
    pub target_removed: bool,
    /// True when the path no longer matched ownership and was left alone.
    pub left_foreign: bool,
}

/// One recorded skill deployment for an Agent configuration root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDeploymentRecord {
    /// Central skill identity.
    pub skill_id: String,
    /// Skill name at deploy time (target directory name).
    pub skill_name: String,
    /// Agent identity wire value (`pi`).
    pub agent_id: String,
    /// Absolute Agent configuration root.
    pub config_root: String,
    /// Deploy mode last applied.
    pub mode: SkillDeployMode,
    /// Library revision recorded at apply time.
    pub skill_revision: i64,
    /// Content digest recorded at apply time.
    pub content_digest: String,
    /// Whether the live target still matches vibemate ownership.
    pub in_sync: bool,
    /// Deploy timestamp in Unix milliseconds.
    pub deployed_at_ms: i64,
}
