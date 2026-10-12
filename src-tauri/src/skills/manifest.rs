//! Inspect a source skill directory: frontmatter plus a bounded file list.
//!
//! Nothing here executes a command or follows a symbolic link. The inspection
//! result is a plan; the import service copies exactly the planned files.

use super::types::SkillError;
use super::validation;
use saphyr::{LoadableYamlNode, Yaml};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Every skill directory must contain this file.
pub(super) const MANIFEST_FILE_NAME: &str = "SKILL.md";
/// Git metadata is never part of a skill and may be large.
const GIT_DIRECTORY: &str = ".git";
/// Frontmatter blocks stay small; a larger header is treated as invalid. The
/// document below the header may be long: only the YAML block is bounded.
const MAX_FRONTMATTER_BYTES: usize = 64 * 1024;
/// File count limit for one imported skill.
///
/// Published skills ship examples, references and test fixtures: the largest
/// known local skill has 315 files, so the limit leaves room above that while
/// still refusing a directory that is really an application checkout.
pub(super) const MAX_FILES: usize = 2048;
/// How many skills one chosen folder may contribute.
pub(super) const MAX_CANDIDATES: usize = 64;
/// Single file size limit.
pub(super) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Total size limit for one imported skill. These three numbers are repeated in
/// the `skill` table constraints and in the interface's response validation.
pub(super) const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// Directory depth limit below the skill directory.
const MAX_DEPTH: usize = 16;
/// Single path segment limit.
const MAX_COMPONENT_BYTES: usize = 128;
/// Whole relative path limit.
const MAX_RELATIVE_PATH_BYTES: usize = 512;

/// One planned file; the copy step reads it by relative path.
#[derive(Debug)]
pub(super) struct SourceFile {
    /// Path relative to the skill directory, joined with `/` on every platform.
    pub(super) relative: String,
    /// Size recorded during inspection, checked again while copying.
    pub(super) size: u64,
    /// Unix permission bits to preserve, when the platform has them.
    pub(super) mode: Option<u32>,
}

/// What the import needs to know before it copies anything.
#[derive(Debug)]
pub(super) struct SourcePlan {
    /// Validated skill name from the frontmatter.
    pub(super) name: String,
    /// Validated one-line description from the frontmatter.
    pub(super) description: String,
    /// Planned files, sorted by relative path.
    pub(super) files: Vec<SourceFile>,
    /// Sum of the planned file sizes.
    pub(super) total_bytes: u64,
}

/// Folders that one chosen directory contributes to an import.
///
/// The chosen directory wins when it is a skill itself, so importing a single
/// skill never pulls in its neighbours. Otherwise its immediate subfolders that
/// hold a `SKILL.md` are used, sorted by name for a predictable report. Hidden
/// folders are skipped: they are agent bookkeeping rather than shared skills.
///
/// A subfolder reached through a directory link is followed, matching the way
/// Agent installations discover skills, while the `SKILL.md` check still
/// requires a regular file.
///
/// # Errors
/// Reports an unreadable directory, a name that is not valid UTF-8, and a folder
/// that holds more candidate skills than one import may copy.
pub(super) fn discover(source: &Path) -> Result<Vec<PathBuf>, SkillError> {
    if is_skill(source) {
        return Ok(vec![source.to_path_buf()]);
    }
    let entries = fs::read_dir(source).map_err(|_| SkillError::SourceNotFound)?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| SkillError::SourceNotFound)?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or(SkillError::SourceUnsupportedEntry)?
            .to_string();
        if name.starts_with('.') {
            continue;
        }
        // A directory reached through a link is a skill folder like any other.
        let followed = fs::metadata(entry.path()).map_err(|_| SkillError::SourceNotFound)?;
        if !followed.is_dir() || !is_skill(&entry.path()) {
            continue;
        }
        if candidates.len() >= MAX_CANDIDATES {
            return Err(SkillError::SourceTooLarge);
        }
        candidates.push(entry.path());
    }
    candidates.sort();
    Ok(candidates)
}

/// Whether one directory is a skill: a regular `SKILL.md` at its root.
///
/// A linked `SKILL.md` is not accepted, so the imported content always comes
/// from the chosen tree.
fn is_skill(directory: &Path) -> bool {
    fs::symlink_metadata(directory.join(MANIFEST_FILE_NAME))
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

/// Inspect `source` and return the import plan.
///
/// # Errors
/// Reports a missing or invalid manifest, an unreadable directory, a symbolic
/// link or special file, and every size, depth or count limit.
pub(super) fn inspect(source: &Path) -> Result<SourcePlan, SkillError> {
    let manifest_path = source.join(MANIFEST_FILE_NAME);
    let manifest = fs::symlink_metadata(&manifest_path).map_err(|_| SkillError::ManifestMissing)?;
    if !manifest.is_file() {
        return Err(SkillError::ManifestMissing);
    }
    let block = frontmatter(&manifest_path)?;
    let (name, description) = parse_frontmatter(&block)?;

    let mut files = Vec::new();
    let mut total_bytes = 0;
    collect(source, "", 0, &mut files, &mut total_bytes)?;
    // A stable order makes the content digest depend on the file set alone.
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(SourcePlan {
        name,
        description,
        files,
        total_bytes,
    })
}

/// Read the YAML block between the opening and closing `---` lines.
///
/// Only the header is read from disk, so a long skill document does not have to
/// be held in memory, and the byte limit applies to the block that is parsed. A
/// file whose first line is not `---`, whose block is never closed, whose block
/// exceeds the limit, or that is not UTF-8 has no usable frontmatter.
fn frontmatter(path: &Path) -> Result<String, SkillError> {
    let file = fs::File::open(path).map_err(|_| SkillError::ManifestMissing)?;
    let mut reader = std::io::BufReader::new(file);
    let mut block = String::new();
    let mut line = String::new();
    let mut started = false;
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|_| SkillError::ManifestInvalid)?
            == 0
        {
            // The document ended before a closing marker: no usable frontmatter.
            return Err(SkillError::ManifestInvalid);
        }
        // A byte order mark is common in files edited on Windows; it is not content.
        let content = line
            .strip_prefix('\u{feff}')
            .unwrap_or(&line)
            .trim_end_matches(['\n', '\r'])
            .trim_end();
        if !started {
            if content != "---" {
                return Err(SkillError::ManifestInvalid);
            }
            started = true;
            continue;
        }
        if content == "---" || content == "..." {
            return Ok(block);
        }
        if block.len() + line.len() > MAX_FRONTMATTER_BYTES {
            return Err(SkillError::ManifestInvalid);
        }
        block.push_str(&line);
    }
}

/// Read `name` and `description` from the frontmatter mapping.
///
/// Extra keys such as `license`, `metadata` or `disable-model-invocation` are
/// ignored: they belong to the Agent that runs the skill, not to the library.
fn parse_frontmatter(block: &str) -> Result<(String, String), SkillError> {
    let documents = Yaml::load_from_str(block).map_err(|_| SkillError::ManifestInvalid)?;
    let document = documents.first().ok_or(SkillError::ManifestInvalid)?;
    if document.as_mapping().is_none() {
        return Err(SkillError::ManifestInvalid);
    }
    let name = scalar(document, "name").ok_or(SkillError::ManifestInvalid)?;
    let name = validation::skill_name(&name)?.to_string();
    let raw_description = scalar(document, "description").ok_or(SkillError::ManifestInvalid)?;
    // Folded or multi-line descriptions become one line, which is what the list
    // and the record can display without breaking the layout.
    let description = collapse_whitespace(&raw_description);
    let description = validation::description(&description)?.to_string();
    Ok((name, description))
}

/// Read one string value from a mapping; non-string values are not accepted.
fn scalar(document: &Yaml<'_>, key: &str) -> Option<String> {
    document.as_mapping_get(key)?.as_str().map(str::to_string)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collect every file below `directory`, rejecting links and special files.
fn collect(
    directory: &Path,
    prefix: &str,
    depth: usize,
    files: &mut Vec<SourceFile>,
    total_bytes: &mut u64,
) -> Result<(), SkillError> {
    if depth > MAX_DEPTH {
        return Err(SkillError::SourceTooLarge);
    }
    let entries = fs::read_dir(directory).map_err(|_| SkillError::SourceNotFound)?;
    for entry in entries {
        let entry = entry.map_err(|_| SkillError::SourceNotFound)?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or(SkillError::SourceUnsupportedEntry)?
            .to_string();
        if name == GIT_DIRECTORY {
            continue;
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if name.len() > MAX_COMPONENT_BYTES || relative.len() > MAX_RELATIVE_PATH_BYTES {
            return Err(SkillError::SourceTooLarge);
        }
        // `symlink_metadata` never follows a link, so a link is rejected instead
        // of being copied or dereferenced outside the chosen directory.
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| SkillError::SourceNotFound)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(SkillError::SourceUnsupportedEntry);
        }
        if file_type.is_dir() {
            collect(&entry.path(), &relative, depth + 1, files, total_bytes)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(SkillError::SourceUnsupportedEntry);
        }
        if metadata.len() > MAX_FILE_BYTES || files.len() >= MAX_FILES {
            return Err(SkillError::SourceTooLarge);
        }
        *total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or(SkillError::SourceTooLarge)?;
        if *total_bytes > MAX_TOTAL_BYTES {
            return Err(SkillError::SourceTooLarge);
        }
        files.push(SourceFile {
            relative,
            size: metadata.len(),
            mode: unix_mode(&metadata),
        });
    }
    Ok(())
}

/// Permission bits worth preserving for bundled scripts.
///
/// Only the ordinary read/write/execute bits are copied; setuid and setgid bits
/// are dropped so an imported skill cannot gain privileges.
#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode() & 0o777)
}

/// Windows and other platforms have no comparable permission bits to copy.
#[cfg(not(unix))]
fn unix_mode(_: &fs::Metadata) -> Option<u32> {
    None
}
