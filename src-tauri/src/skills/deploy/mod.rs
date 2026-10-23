//! Deploy a central skill into Pi's user-level `skills/` directory.
//!
//! Symlink is preferred when the platform can create a directory link under the
//! target parent; otherwise the caller must choose copy-sync explicitly. User
//! content at the same name is never overwritten. Ownership lives in
//! `skill_deployment`; disable only removes a still-managed target.

mod repository;

#[cfg(test)]
mod tests;

use super::SkillLibrary;
use super::manifest;
use super::service::{copy, digest_tree};
use super::types::{
    DisablePiSkillDeployRequest, PiSkillDeployDisabled, PiSkillDeployPreview, PiSkillDeployed,
    ReviewedPiSkillDeploy, SkillDeployMode, SkillDeployTargetState, SkillDeploymentRecord,
    SkillError,
};
use super::validation;
use crate::agents::{DirectorySource, DiscoveryEnvironment, pi_config_root};
use crate::storage::Storage;
use std::fs;
use std::path::{Path, PathBuf};

/// Agent wire identity stored with every Pi deployment row.
const PI_AGENT_ID: &str = "pi";
/// Skills live directly under the Pi agent directory.
const SKILLS_DIR: &str = "skills";
/// Temporary sibling used while copy-sync replaces an owned target.
const COPY_TEMP_PREFIX: &str = ".vibemate-deploy-";
/// Probe link name used only during symlink capability checks.
const SYMLINK_PROBE: &str = ".vibemate-symlink-probe";

/// Preview deploying one library skill into the resolved Pi agent directory.
///
/// # Errors
/// Rejects an unknown skill, an unresolvable Pi root, and storage failures.
pub fn preview_pi_skill_deploy(
    library: &SkillLibrary,
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    skill_id: &str,
) -> Result<PiSkillDeployPreview, SkillError> {
    validation::identity(skill_id)?;
    let record = super::repository::get_skill(storage, skill_id)?;
    let (root, source) = pi_config_root(environment);
    let root = match (root, source) {
        (Some(path), DirectorySource::Default | DirectorySource::Environment) => path,
        _ => return Err(SkillError::AgentLocationUnavailable),
    };
    let config_root = display_root(&root)?;
    let target = root.join(SKILLS_DIR).join(&record.name);
    let target_path = display_root(&target)?;
    let managed = library.root().join(&record.id);
    let owned = repository::get_deployment(storage, skill_id, PI_AGENT_ID, &config_root)?;
    let target_state = classify_target(&target, &managed, owned.as_ref())?;
    let symlink_available = probe_symlink_available(&root.join(SKILLS_DIR), &managed)?;
    let suggested_mode = if symlink_available {
        SkillDeployMode::Symlink
    } else {
        SkillDeployMode::Copy
    };
    let can_apply = target_state != SkillDeployTargetState::Foreign;
    Ok(PiSkillDeployPreview {
        skill_id: record.id,
        skill_name: record.name.clone(),
        skill_revision: record.revision,
        content_digest: record.content_digest,
        config_root,
        target_path,
        target_name: record.name,
        target_state,
        symlink_available,
        suggested_mode,
        can_apply,
    })
}

/// Apply a reviewed Pi skill deployment.
///
/// # Errors
/// Rejects stale reviews, foreign targets, unavailable symlink mode, and write
/// failures. A filesystem write followed by a failed record reports
/// `outcome_unknown` when the target cannot be rolled back.
pub fn apply_pi_skill_deploy(
    library: &SkillLibrary,
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    reviewed: ReviewedPiSkillDeploy,
    now_ms: i64,
) -> Result<PiSkillDeployed, SkillError> {
    if now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    validation::identity(&reviewed.skill_id)?;
    if reviewed.expected_skill_revision < 1 {
        return Err(SkillError::InvalidRequest);
    }
    if matches!(reviewed.target_state, SkillDeployTargetState::Foreign) {
        return Err(SkillError::DeployModeInvalid);
    }
    let record = super::repository::get_skill(storage, &reviewed.skill_id)?;
    if record.revision != reviewed.expected_skill_revision {
        return Err(SkillError::RevisionConflict);
    }
    let (root, source) = pi_config_root(environment);
    let root = match (root, source) {
        (Some(path), DirectorySource::Default | DirectorySource::Environment) => path,
        _ => return Err(SkillError::AgentLocationUnavailable),
    };
    let config_root = display_root(&root)?;
    if config_root != reviewed.config_root {
        return Err(SkillError::TargetChanged);
    }
    let skills_parent = root.join(SKILLS_DIR);
    let target = skills_parent.join(&record.name);
    let managed = library.root().join(&record.id);
    if !managed.is_dir() {
        return Err(SkillError::NotFound);
    }
    let owned = repository::get_deployment(storage, &reviewed.skill_id, PI_AGENT_ID, &config_root)?;
    let current = classify_target(&target, &managed, owned.as_ref())?;
    if current != reviewed.target_state {
        return Err(SkillError::TargetChanged);
    }
    if current == SkillDeployTargetState::Foreign {
        return Err(SkillError::TargetConflict);
    }
    let symlink_available = probe_symlink_available(&skills_parent, &managed)?;
    match reviewed.mode {
        SkillDeployMode::Symlink if !symlink_available => {
            return Err(SkillError::SymlinkUnavailable);
        }
        SkillDeployMode::Copy
            if symlink_available && reviewed.target_state == SkillDeployTargetState::Absent =>
        {
            // Copy is always allowed; the interface must choose it when symlink
            // is unavailable. When symlink is available, copy is still an
            // explicit user choice and remains valid.
        }
        _ => {}
    }

    fs::create_dir_all(&skills_parent).map_err(|_| SkillError::WriteFailed)?;
    // Remove a previously owned target before writing the new form.
    if matches!(
        current,
        SkillDeployTargetState::OwnedSymlink | SkillDeployTargetState::OwnedCopy
    ) {
        remove_path(&target)?;
    }

    match reviewed.mode {
        SkillDeployMode::Symlink => {
            create_dir_symlink(&managed, &target)?;
        }
        SkillDeployMode::Copy => {
            let plan = manifest::inspect(&managed)?;
            let temporary = skills_parent.join(format!("{COPY_TEMP_PREFIX}{}", record.id));
            let _ = fs::remove_dir_all(&temporary);
            fs::create_dir(&temporary).map_err(|_| SkillError::WriteFailed)?;
            if let Err(error) = copy(&managed, &plan, &temporary) {
                let _ = fs::remove_dir_all(&temporary);
                return Err(error);
            }
            if fs::rename(&temporary, &target).is_err() {
                let _ = fs::remove_dir_all(&temporary);
                return Err(SkillError::WriteFailed);
            }
        }
    }

    let target_path = display_root(&target)?;
    let upsert = repository::DeploymentUpsert {
        skill_id: &record.id,
        agent_id: PI_AGENT_ID,
        config_root: &config_root,
        target_name: &record.name,
        mode: reviewed.mode,
        skill_revision: record.revision,
        content_digest: &record.content_digest,
        deployed_at: now_ms,
    };
    if let Err(error) = repository::upsert_deployment(storage, &upsert) {
        // Roll back the filesystem write so a failed record does not leave an
        // untracked managed target. Failure to roll back requires a reload.
        if remove_path(&target).is_err() {
            return Err(SkillError::OutcomeUnknown);
        }
        return Err(error);
    }
    Ok(PiSkillDeployed {
        skill_id: record.id,
        skill_name: record.name,
        skill_revision: record.revision,
        mode: reviewed.mode,
        target_path,
    })
}

/// Revoke one vibemate-managed Pi skill deployment.
///
/// When the live path no longer matches ownership, the filesystem is left alone
/// and the record is still cleared so a later deploy can conflict-check honestly.
///
/// # Errors
/// Rejects an unknown deployment, an unresolvable root, and storage failures.
pub fn disable_pi_skill_deploy(
    library: &SkillLibrary,
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: DisablePiSkillDeployRequest,
) -> Result<PiSkillDeployDisabled, SkillError> {
    validation::identity(&request.skill_id)?;
    let (root, source) = pi_config_root(environment);
    let root = match (root, source) {
        (Some(path), DirectorySource::Default | DirectorySource::Environment) => path,
        _ => return Err(SkillError::AgentLocationUnavailable),
    };
    let config_root = display_root(&root)?;
    if config_root != request.config_root {
        return Err(SkillError::TargetChanged);
    }
    let deployment =
        repository::get_deployment(storage, &request.skill_id, PI_AGENT_ID, &config_root)?
            .ok_or(SkillError::NotFound)?;
    let record = super::repository::get_skill(storage, &request.skill_id)?;
    let target = root.join(SKILLS_DIR).join(&deployment.target_name);
    let target_path = display_root(&target)?;
    let managed = library.root().join(&record.id);
    let owned = Some(deployment);
    let state = classify_target(&target, &managed, owned.as_ref())?;
    let (target_removed, left_foreign) = match state {
        SkillDeployTargetState::OwnedSymlink | SkillDeployTargetState::OwnedCopy => {
            remove_path(&target)?;
            (true, false)
        }
        SkillDeployTargetState::Absent => (false, false),
        SkillDeployTargetState::Foreign => (false, true),
    };
    repository::delete_deployment(storage, &request.skill_id, PI_AGENT_ID, &config_root)?;
    Ok(PiSkillDeployDisabled {
        skill_id: request.skill_id,
        target_path,
        target_removed,
        left_foreign,
    })
}

/// List deployments recorded for the resolved Pi agent directory.
///
/// # Errors
/// Rejects an unresolvable Pi root and storage failures.
pub fn list_pi_skill_deployments(
    library: &SkillLibrary,
    storage: &Storage,
    environment: &DiscoveryEnvironment,
) -> Result<Vec<SkillDeploymentRecord>, SkillError> {
    let (root, source) = pi_config_root(environment);
    let root = match (root, source) {
        (Some(path), DirectorySource::Default | DirectorySource::Environment) => path,
        _ => return Err(SkillError::AgentLocationUnavailable),
    };
    let config_root = display_root(&root)?;
    let rows = repository::list_deployments_for_root(storage, PI_AGENT_ID, &config_root)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let managed = library.root().join(&row.skill_id);
        let target = root.join(SKILLS_DIR).join(&row.target_name);
        let owned = Some(row.clone());
        let state = classify_target(&target, &managed, owned.as_ref())?;
        let in_sync = matches!(
            state,
            SkillDeployTargetState::OwnedSymlink | SkillDeployTargetState::OwnedCopy
        );
        let skill_name = super::repository::get_skill(storage, &row.skill_id)
            .map(|skill| skill.name)
            .unwrap_or_else(|_| row.target_name.clone());
        out.push(SkillDeploymentRecord {
            skill_id: row.skill_id,
            skill_name,
            agent_id: row.agent_id,
            config_root: row.config_root,
            mode: row.mode,
            skill_revision: row.skill_revision,
            content_digest: row.content_digest,
            in_sync,
            deployed_at_ms: row.deployed_at,
        });
    }
    Ok(out)
}

/// List deployments recorded for one central skill (any Pi root).
///
/// # Errors
/// Rejects an invalid identity and storage failures.
pub fn list_skill_deployments(
    library: &SkillLibrary,
    storage: &Storage,
    skill_id: &str,
) -> Result<Vec<SkillDeploymentRecord>, SkillError> {
    validation::identity(skill_id)?;
    let rows = repository::list_deployments_for_skill(storage, skill_id)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let managed = library.root().join(&row.skill_id);
        let root = PathBuf::from(&row.config_root);
        let target = root.join(SKILLS_DIR).join(&row.target_name);
        let owned = Some(row.clone());
        let state = classify_target(&target, &managed, owned.as_ref())?;
        let in_sync = matches!(
            state,
            SkillDeployTargetState::OwnedSymlink | SkillDeployTargetState::OwnedCopy
        );
        let skill_name = super::repository::get_skill(storage, &row.skill_id)
            .map(|skill| skill.name)
            .unwrap_or_else(|_| row.target_name.clone());
        out.push(SkillDeploymentRecord {
            skill_id: row.skill_id,
            skill_name,
            agent_id: row.agent_id,
            config_root: row.config_root,
            mode: row.mode,
            skill_revision: row.skill_revision,
            content_digest: row.content_digest,
            in_sync,
            deployed_at_ms: row.deployed_at,
        });
    }
    Ok(out)
}

/// Whether any deployment row still references this skill.
pub(super) fn skill_has_deployments(storage: &Storage, skill_id: &str) -> Result<bool, SkillError> {
    repository::skill_has_deployments(storage, skill_id)
}

fn display_root(path: &Path) -> Result<String, SkillError> {
    let value = path.to_str().ok_or(SkillError::AgentLocationUnavailable)?;
    if !path.is_absolute()
        || value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
    {
        return Err(SkillError::AgentLocationUnavailable);
    }
    Ok(value.to_owned())
}

fn classify_target(
    target: &Path,
    managed: &Path,
    owned: Option<&repository::StoredDeployment>,
) -> Result<SkillDeployTargetState, SkillError> {
    let meta = match fs::symlink_metadata(target) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SkillDeployTargetState::Absent);
        }
        Err(_) => return Err(SkillError::ReadFailed),
    };
    let Some(owned) = owned else {
        return Ok(SkillDeployTargetState::Foreign);
    };
    if meta.file_type().is_symlink() {
        if owned.mode != SkillDeployMode::Symlink {
            return Ok(SkillDeployTargetState::Foreign);
        }
        return Ok(if symlink_points_to(target, managed)? {
            SkillDeployTargetState::OwnedSymlink
        } else {
            SkillDeployTargetState::Foreign
        });
    }
    if !meta.is_dir() {
        return Ok(SkillDeployTargetState::Foreign);
    }
    if owned.mode != SkillDeployMode::Copy {
        return Ok(SkillDeployTargetState::Foreign);
    }
    // Re-digest the copy; a mismatch means the user replaced our content.
    match manifest::inspect(target).and_then(|plan| digest_tree(target, &plan)) {
        Ok(digest) if digest == owned.content_digest => Ok(SkillDeployTargetState::OwnedCopy),
        Ok(_) | Err(_) => Ok(SkillDeployTargetState::Foreign),
    }
}

fn symlink_points_to(link: &Path, managed: &Path) -> Result<bool, SkillError> {
    let target = fs::read_link(link).map_err(|_| SkillError::ReadFailed)?;
    let resolved = if target.is_absolute() {
        target
    } else {
        link.parent().ok_or(SkillError::ReadFailed)?.join(target)
    };
    let left = fs::canonicalize(&resolved).ok();
    let right = fs::canonicalize(managed).ok();
    Ok(left.is_some() && left == right)
}

/// Try creating and removing a directory symlink under `parent`.
fn probe_symlink_available(parent: &Path, managed: &Path) -> Result<bool, SkillError> {
    fs::create_dir_all(parent).map_err(|_| SkillError::WriteFailed)?;
    let probe = parent.join(SYMLINK_PROBE);
    let _ = fs::remove_file(&probe);
    let _ = fs::remove_dir_all(&probe);
    match create_dir_symlink(managed, &probe) {
        Ok(()) => {
            let _ = remove_path(&probe);
            Ok(true)
        }
        Err(_) => {
            let _ = remove_path(&probe);
            Ok(false)
        }
    }
}

fn create_dir_symlink(source: &Path, link: &Path) -> Result<(), SkillError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link).map_err(|_| SkillError::WriteFailed)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, link).map_err(|_| SkillError::WriteFailed)
    }
}

fn remove_path(path: &Path) -> Result<(), SkillError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(SkillError::WriteFailed),
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path).map_err(|_| SkillError::WriteFailed)
    } else if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|_| SkillError::WriteFailed)
    } else {
        Err(SkillError::WriteFailed)
    }
}
