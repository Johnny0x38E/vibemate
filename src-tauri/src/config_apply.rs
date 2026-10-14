//! Atomic, recoverable writes of Agent configuration files.
//!
//! A plan says what would change; this module is the only place that replaces a
//! file. It refuses to write when the file on disk no longer matches the plan's
//! change token, keeps a private copy of what it replaces, writes a temporary
//! file next to the target and renames it into place, and re-reads the result
//! instead of assuming success. Failures carry a stable code and a path only:
//! file contents never leave this module except through the backup directory,
//! which is private because a settings file can already hold a user's key.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Serialize;

use crate::config_plan::{
    MAX_TARGET_BYTES, PlanError, TargetDocument, TargetFile, digest_hex, read_target_document,
};
use crate::shared;

/// A file a plan wants to replace, with the token the plan was built from.
#[derive(Debug, Clone)]
pub struct WriteTarget {
    /// Absolute path of the file a plan would change.
    pub path: PathBuf,
    /// SHA-256 the plan saw, or `None` when the plan saw no file at all.
    pub expected_digest: Option<String>,
}

impl WriteTarget {
    /// Take the path and token straight from a plan, so an apply step cannot be
    /// built from a different file than the one the user reviewed.
    pub fn from_plan(target: &TargetFile) -> Self {
        Self {
            path: PathBuf::from(&target.path),
            expected_digest: target.content_digest.clone(),
        }
    }
}

/// Private directory that holds the content this module replaced.
#[derive(Debug, Clone)]
pub struct BackupStore {
    root: PathBuf,
}

/// One stored copy of a file that was replaced.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// Identifier of the write that stored this copy; also its directory name.
    pub id: String,
    /// Absolute path of the stored copy, so the user can locate the backup.
    pub path: String,
    /// SHA-256 of the replaced bytes, for a restore step to verify.
    pub content_digest: String,
}

impl BackupStore {
    /// Keep backups under `root`; the directory is created on the first backup.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Root directory of every stored copy.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Store the bytes that are about to be replaced.
    fn store(&self, id: &str, name: &str, bytes: &[u8]) -> Result<Backup, WriteError> {
        let directory = self.root.join(id);
        create_private_directory(&directory)?;
        let path = directory.join(name);
        overwrite_private_file(&path, bytes)?;
        Ok(Backup {
            id: id.to_owned(),
            path: path_text(&path)?,
            content_digest: digest_hex(bytes),
        })
    }

    /// Remove a copy whose write did not happen, keeping the store free of
    /// entries no operation refers to.
    fn discard(&self, backup: &Backup) {
        let _ = fs::remove_dir_all(self.root.join(&backup.id));
    }
}

/// What one applied file became on disk.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedFile {
    /// Identity of the target after the write, including its new digest.
    pub target: TargetFile,
    /// Copy of the replaced content; absent for a new file or an unchanged one.
    pub backup: Option<Backup>,
    /// Whether the file changed; `false` means the bytes already matched.
    pub changed: bool,
}

/// Why a configuration write did not replace the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteError {
    /// The plan token no longer matches: the file appeared, vanished or changed.
    TargetChanged,
    /// The target path is a symbolic link; replacing it would write elsewhere.
    SymlinkRefused,
    /// The target exists but is read-only for the user.
    TargetReadOnly,
    /// The target path cannot be shown or is not absolute.
    TargetPathInvalid,
    /// The target is not a readable regular file.
    TargetUnreadable,
    /// The target is larger than the preview limit.
    TargetTooLarge,
    /// The private backup could not be stored; the target was left alone.
    BackupUnavailable,
    /// The target could not be replaced; its previous content is intact.
    WriteFailed,
    /// The replacement may or may not have happened; reload before another try.
    OutcomeUnknown,
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TargetChanged => "The configuration file changed since the preview.",
            Self::SymlinkRefused => "The configuration path is a symbolic link.",
            Self::TargetReadOnly => "The configuration file is read-only.",
            Self::TargetPathInvalid => "The configuration path cannot be used.",
            Self::TargetUnreadable => "The configuration file could not be read.",
            Self::TargetTooLarge => "The configuration file is too large to replace.",
            Self::BackupUnavailable => "The backup could not be stored.",
            Self::WriteFailed => "The configuration file could not be written.",
            Self::OutcomeUnknown => "The configuration write result is unknown.",
        })
    }
}

impl std::error::Error for WriteError {}

/// Replace one configuration file with `content` after checking the plan token.
///
/// The file is written through a temporary file in the same directory and then
/// renamed, so a reader sees either the previous or the new content. The
/// previous bytes are stored in `backups` first; a write that cannot be backed
/// up does not happen. An unchanged target reports `changed: false` without
/// touching the disk.
///
/// # Errors
/// Returns `TargetChanged` when the file no longer matches the plan token,
/// `SymlinkRefused`, `TargetReadOnly`, `TargetPathInvalid`, `TargetUnreadable`
/// or `TargetTooLarge` for a target this module refuses to replace,
/// `BackupUnavailable` when the previous content could not be stored, and
/// `WriteFailed` or `OutcomeUnknown` when the replacement did not complete or
/// could not be verified. No error carries file content.
pub fn apply_file(
    target: &WriteTarget,
    content: &[u8],
    backups: &BackupStore,
) -> Result<AppliedFile, WriteError> {
    let current = read_target(&target.path)?;
    if current.target.symlink {
        return Err(WriteError::SymlinkRefused);
    }
    if current.target.content_digest != target.expected_digest {
        return Err(WriteError::TargetChanged);
    }
    let new_digest = digest_hex(content);
    if current.target.content_digest.as_deref() == Some(new_digest.as_str()) {
        return Ok(AppliedFile {
            target: current.target,
            backup: None,
            changed: false,
        });
    }
    let previous = match current.target.exists {
        true => {
            if is_read_only(&target.path)? {
                return Err(WriteError::TargetReadOnly);
            }
            Some(read_replaced_bytes(&target.path)?)
        }
        false => None,
    };
    let id = operation_id();
    let backup = match &previous {
        Some(bytes) => Some(backups.store(&id, &file_name(&target.path), bytes)?),
        None => None,
    };
    if let Err(error) = replace(&target.path, content, previous.is_some(), &id) {
        // Nothing was replaced, so the copy has no matching operation; a later
        // batch records backups and operations together.
        if let Some(backup) = &backup {
            backups.discard(backup);
        }
        return Err(error);
    }
    let after = read_target(&target.path).map_err(|_| WriteError::OutcomeUnknown)?;
    if after.target.content_digest.as_deref() != Some(new_digest.as_str()) {
        return Err(WriteError::OutcomeUnknown);
    }
    Ok(AppliedFile {
        target: after.target,
        backup,
        changed: true,
    })
}

/// Replace the target through a same-directory temporary file.
///
/// A failure removes the temporary file and leaves the target untouched.
fn replace(path: &Path, content: &[u8], keep_mode: bool, id: &str) -> Result<(), WriteError> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(WriteError::TargetPathInvalid)?;
    // A first-time user has no configuration directory yet; the preview already
    // reported the target as a new file at this path.
    fs::create_dir_all(directory).map_err(|_| WriteError::WriteFailed)?;
    let temporary = directory.join(format!(".{}.vibemate-{id}.tmp", file_name(path)));
    write_temporary(&temporary, content)?;
    if keep_mode && let Err(error) = copy_mode(path, &temporary) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(WriteError::WriteFailed);
    }
    sync_directory(directory)
}

/// Create the temporary file with private access and write the planned bytes.
fn write_temporary(path: &Path, content: &[u8]) -> Result<(), WriteError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // The file can hold a credential before it replaces the target.
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| WriteError::WriteFailed)?;
    if file
        .write_all(content)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(WriteError::WriteFailed);
    }
    Ok(())
}

/// Read a target's identity through the preview's reader, so both steps agree
/// on what an unchanged file means.
fn read_target(path: &Path) -> Result<TargetDocument, WriteError> {
    read_target_document(path).map_err(|error| match error {
        PlanError::TargetPathInvalid => WriteError::TargetPathInvalid,
        PlanError::TargetTooLarge => WriteError::TargetTooLarge,
        _ => WriteError::TargetUnreadable,
    })
}

/// Read the bytes that are about to be replaced, within the preview limit.
fn read_replaced_bytes(path: &Path) -> Result<Vec<u8>, WriteError> {
    let metadata = fs::metadata(path).map_err(|_| WriteError::TargetUnreadable)?;
    if metadata.len() > MAX_TARGET_BYTES {
        return Err(WriteError::TargetTooLarge);
    }
    fs::read(path).map_err(|_| WriteError::TargetUnreadable)
}

/// Whether the user marked the target read-only.
///
/// A read-only file inside a writable directory can still be replaced by a
/// rename, so the permission is checked explicitly instead of surfacing as a
/// surprising success.
fn is_read_only(path: &Path) -> Result<bool, WriteError> {
    fs::metadata(path)
        .map(|metadata| metadata.permissions().readonly())
        .map_err(|_| WriteError::TargetUnreadable)
}

/// Fresh identifier for one write: backup directory name and temporary suffix.
///
/// The value only has to make two writes of the same file distinct; the stored
/// operation record (P19.b) is what identifies a write across restarts.
fn operation_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let millis = shared::current_unix_millis().unwrap_or(0);
    format!("{millis}-{}-{counter}", std::process::id())
}

/// Name used inside a backup directory.
fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "target".to_owned())
}

/// Absolute, printable path text matching the preview's rules.
fn path_text(path: &Path) -> Result<String, WriteError> {
    let text = path.to_str().ok_or(WriteError::TargetPathInvalid)?;
    (path.is_absolute() && text.len() <= 4096 && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
        .ok_or(WriteError::TargetPathInvalid)
}

/// Create the backup directory chain with owner-only access.
#[cfg(unix)]
fn create_private_directory(directory: &Path) -> Result<(), WriteError> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder
        .create(directory)
        .map_err(|_| WriteError::BackupUnavailable)
}

/// Other platforms do not expose a comparable directory mode.
#[cfg(not(unix))]
fn create_private_directory(directory: &Path) -> Result<(), WriteError> {
    fs::create_dir_all(directory).map_err(|_| WriteError::BackupUnavailable)
}

/// Store bytes in a private file of the backup directory.
fn overwrite_private_file(path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| WriteError::BackupUnavailable)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| WriteError::BackupUnavailable)
}

/// Keep the user's permission bits on the replacement file.
///
/// Preserving them keeps a private file private; the create mode of 0600 covers
/// the case where no earlier file existed.
#[cfg(unix)]
fn copy_mode(from: &Path, to: &Path) -> Result<(), WriteError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(from)
        .map_err(|_| WriteError::WriteFailed)?
        .permissions()
        .mode()
        & 0o7777;
    fs::set_permissions(to, fs::Permissions::from_mode(mode)).map_err(|_| WriteError::WriteFailed)
}

/// Other platforms do not carry a comparable permission model.
#[cfg(not(unix))]
fn copy_mode(_: &Path, _: &Path) -> Result<(), WriteError> {
    Ok(())
}

/// Flush the directory entry after the rename, so a crash cannot lose the name.
///
/// A failure here means the new content is in place but its durability is
/// unknown, which is reported instead of being called a success.
#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), WriteError> {
    let handle = fs::File::open(directory).map_err(|_| WriteError::OutcomeUnknown)?;
    handle.sync_all().map_err(|_| WriteError::OutcomeUnknown)
}

/// Windows does not open a directory through `File`; the rename itself is still
/// atomic and the content is flushed before it happens.
#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<(), WriteError> {
    Ok(())
}

#[cfg(test)]
mod tests;
