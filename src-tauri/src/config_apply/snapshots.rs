//! Read and verify the private pre-mutation copies referenced by revision rows.
//!
//! A revision identifies its snapshot by ID and digest; the stored path is a
//! reference, not permission to read any path planted in the database. The
//! expected location is always `<backup-root>/<revision-id>/<target-file-name>`,
//! so this module checks both directory components, refuses links and
//! non-files, applies the same size bound as a configuration target, and
//! verifies the bytes against the recorded digest. It never writes.

use std::{fs, path::Path};

use super::{BackupStore, MAX_TARGET_BYTES, WriteError, digest_hex, file_name};

/// Read one retained snapshot and prove it still holds the recorded bytes.
///
/// `recorded_path` comes from the journal and must equal the path this module
/// derives; a mismatch is treated as an unavailable copy rather than followed.
/// Absence before the mutation has no snapshot and must be handled by the
/// caller, not passed here.
///
/// # Errors
/// Returns [`WriteError::BackupUnavailable`] for a missing or non-file path,
/// a symbolic link in the expected chain, a size over the target limit, a
/// digest mismatch, an unreadable copy, or a recorded path that does not match
/// the private-store layout. No error carries file content.
pub(super) fn read_snapshot(
    backups: &BackupStore,
    revision_id: &str,
    target_path: &str,
    recorded_path: &str,
    expected_digest: &str,
) -> Result<Vec<u8>, WriteError> {
    let expected = backups
        .root()
        .join(revision_id)
        .join(file_name(Path::new(target_path)));
    if Path::new(recorded_path) != expected
        || !is_regular_directory(backups.root())
        || !is_regular_directory(expected.parent().expect("snapshot has a parent"))
    {
        return Err(WriteError::BackupUnavailable);
    }
    let metadata = fs::symlink_metadata(&expected).map_err(|_| WriteError::BackupUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_TARGET_BYTES
    {
        return Err(WriteError::BackupUnavailable);
    }
    let bytes = fs::read(&expected).map_err(|_| WriteError::BackupUnavailable)?;
    if digest_hex(&bytes) != expected_digest {
        return Err(WriteError::BackupUnavailable);
    }
    Ok(bytes)
}

/// Whether a path is a real directory rather than a link or another file type.
fn is_regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !metadata.is_symlink())
}
