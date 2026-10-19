//! Agent-scoped review and explicit reconciliation of interrupted mutations.
//!
//! IPC supplies identities and a digest only. The private backup root comes from
//! application storage. Reconciliation records observations, never restores a
//! configuration or claims that a running Agent has loaded it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ClaudeCodeRestoreError;
use crate::{
    config_apply::{
        self, BackupStore, OperationKind, OperationState, RevisionInspection,
        RevisionResolutionPlan,
    },
    config_plan::digest_hex,
    shared,
    storage::Storage,
};

/// Only these adapters have a verified configuration write/history path.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigurationAgent {
    /// Claude Code's recorded user settings writes.
    ClaudeCode,
    /// Pi's recorded models/settings writes.
    Pi,
    /// Grok Build's recorded user configuration writes.
    GrokBuild,
}

impl ConfigurationAgent {
    fn kind(self) -> OperationKind {
        match self {
            Self::ClaudeCode => OperationKind::ClaudeCodeSettings,
            Self::Pi => OperationKind::PiSettings,
            Self::GrokBuild => OperationKind::GrokSettings,
        }
    }
}

/// Strict request for one Agent's recorded operation; accepts no filesystem path.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewResolutionRequest {
    /// Adapter whose history the user is viewing.
    pub agent: ConfigurationAgent,
    /// Recorded operation to inspect.
    pub operation_id: String,
}

/// Approval of the entire server-built review, never frontend-supplied observations.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveConfigurationRequest {
    /// Adapter whose history the user is viewing.
    pub agent: ConfigurationAgent,
    /// Recorded operation the review described.
    pub operation_id: String,
    /// SHA-256 of the complete backend plan, including source metadata and candidates.
    pub reviewed_digest: String,
}

/// Safe observation review; contains paths and digests, not configuration bytes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationResolutionReview {
    /// Operation whose observations must all be approved together.
    pub operation_id: String,
    /// Opaque approval token for the complete backend plan.
    pub reviewed_digest: String,
    /// Whether the backend currently permits recording these observations.
    pub resolvable: bool,
    /// All interrupted mutations, including blocked/superseded observations.
    pub revisions: Vec<RevisionInspection>,
}

/// A settled journal result, not a file-write or runtime-success claim.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationResolutionOutcome {
    /// Operation explicitly reconciled by the user.
    pub operation_id: String,
    /// Legacy operation state after atomically repairing its steps.
    pub state: OperationState,
}

/// Review only this Agent's interrupted mutations, without changing any files/logs.
///
/// Checks operation ownership before reading targets. The caller must supply the
/// trusted application backup root, never a path received from the frontend.
///
/// # Errors
/// Returns `InvalidRequest` for malformed identities, `OperationNotFound` for a
/// missing/other-Agent operation, and safe storage/operation failures on read errors.
pub fn review_configuration_resolution(
    storage: &Storage,
    request: &ReviewResolutionRequest,
    backups_root: &Path,
) -> Result<ConfigurationResolutionReview, ClaudeCodeRestoreError> {
    let plan = scoped_plan(storage, request.agent, &request.operation_id, backups_root)?;
    Ok(ConfigurationResolutionReview {
        operation_id: plan.operation_id.clone(),
        reviewed_digest: approval_digest(&plan)?,
        resolvable: plan.resolvable,
        revisions: plan.revisions,
    })
}

/// Record the explicitly approved observations and repair journal results.
///
/// Rebuilds the entire review and compares its digest, then delegates to the
/// shared cross-instance-locked transaction, which revalidates once more. Any
/// file, backup, step or candidate change invalidates approval. No configuration,
/// credential or ownership binding is changed; restoration is a separate action.
///
/// # Errors
/// As [`review_configuration_resolution`], plus `PlanStale` for changed/repeated
/// approval and `OperationFailed` for observations that cannot be safely settled.
pub fn resolve_configuration_operation(
    storage: &Storage,
    request: &ResolveConfigurationRequest,
    backups_root: &Path,
) -> Result<ConfigurationResolutionOutcome, ClaudeCodeRestoreError> {
    if !super::claude_code::is_digest(&request.reviewed_digest) {
        return Err(ClaudeCodeRestoreError::InvalidRequest);
    }
    let plan = scoped_plan(storage, request.agent, &request.operation_id, backups_root)?;
    if approval_digest(&plan)? != request.reviewed_digest || plan.revisions.is_empty() {
        return Err(ClaudeCodeRestoreError::PlanStale);
    }
    let state = config_apply::resolve_interrupted_revisions(
        storage,
        &request.operation_id,
        &plan,
        &BackupStore::new(backups_root.to_path_buf()),
    )?;
    Ok(ConfigurationResolutionOutcome {
        operation_id: request.operation_id.clone(),
        state,
    })
}

fn scoped_plan(
    storage: &Storage,
    agent: ConfigurationAgent,
    id: &str,
    backups_root: &Path,
) -> Result<RevisionResolutionPlan, ClaudeCodeRestoreError> {
    if !shared::is_random_id(id) {
        return Err(ClaudeCodeRestoreError::InvalidRequest);
    }
    match config_apply::operation(storage, id)? {
        Some(record) if record.kind == agent.kind() => {}
        _ => return Err(ClaudeCodeRestoreError::OperationNotFound),
    }
    config_apply::plan_revision_resolution(
        storage,
        id,
        &BackupStore::new(backups_root.to_path_buf()),
    )?
    .ok_or(ClaudeCodeRestoreError::OperationNotFound)
}

fn approval_digest(plan: &RevisionResolutionPlan) -> Result<String, ClaudeCodeRestoreError> {
    let bytes = serde_json::to_vec(plan).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    Ok(digest_hex(&bytes))
}
