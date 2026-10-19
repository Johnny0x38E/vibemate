//! Strict IPC boundary for direct restore of any retained version (H2).
//!
//! The frontend names one Agent and one history entry; Rust selects the active
//! configuration root and owned files from the trusted desktop environment. No
//! path, snapshot location or backup path is accepted from the caller. The
//! domain revalidates the entry, snapshots and reviewed digests; this module
//! only bounds the payload and maps safe errors.

use serde::Deserialize;

use super::{
    ClaudeCodeRestoreError, DiscoveryEnvironment, history::history_scope,
    recovery::ConfigurationAgent,
};
use crate::{
    config_apply::{
        self, BackupStore, ReviewedVersionFile, VersionRestoreOutcome, VersionRestorePlan,
    },
    storage::Storage,
};

/// Most files one supported Agent scope can own.
const MAX_REVIEWED_FILES: usize = 16;

/// Request one entry's version-restore preview.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionRestoreRequest {
    /// Adapter whose history the user is viewing.
    pub agent: ConfigurationAgent,
    /// `operation:<id>` or `batch:<id>` from the history page.
    pub entry_key: String,
}

/// Confirm a reviewed preview for one history entry.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyVersionRestoreRequest {
    /// Adapter whose history the user is viewing.
    pub agent: ConfigurationAgent,
    /// Entry the preview described.
    pub entry_key: String,
    /// Exactly the files the preview said it would change.
    pub reviewed: Vec<ReviewedVersionFile>,
}

/// Build a version-restore preview for one history entry.
///
/// # Errors
/// Returns safe request, scope, stale-log or storage errors. The plan itself
/// carries per-file blocked reasons and no file content.
pub fn plan_version_restore(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &VersionRestoreRequest,
    backups: &BackupStore,
) -> Result<VersionRestorePlan, ClaudeCodeRestoreError> {
    let scope = history_scope(request.agent, environment)?;
    Ok(config_apply::plan_version_restore(
        storage,
        &scope,
        &request.entry_key,
        backups,
    )?)
}

/// Apply a reviewed version restore or refuse when its tokens are stale.
///
/// # Errors
/// Returns `InvalidRequest` for an empty or oversized review list, the scope and
/// plan errors of [`plan_version_restore`], and `PlanStale` when the reviewed
/// files no longer match the rebuilt plan.
pub fn apply_version_restore(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &ApplyVersionRestoreRequest,
    backups: &BackupStore,
) -> Result<VersionRestoreOutcome, ClaudeCodeRestoreError> {
    if request.reviewed.is_empty()
        || request.reviewed.len() > MAX_REVIEWED_FILES
        || request.reviewed.iter().any(|file| {
            !valid_revision_id(&file.revision_id)
                || file
                    .current_digest
                    .as_deref()
                    .is_some_and(|digest| !is_digest(digest))
        })
    {
        return Err(ClaudeCodeRestoreError::InvalidRequest);
    }
    let scope = history_scope(request.agent, environment)?;
    Ok(config_apply::apply_version_restore(
        storage,
        &scope,
        &request.entry_key,
        &request.reviewed,
        backups,
    )?)
}

/// Whether a value has the shape of a stored revision identity.
fn valid_revision_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Whether a value is a lowercase SHA-256 token.
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
