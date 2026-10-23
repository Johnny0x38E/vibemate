//! SQL for skill deployment ownership rows. Filesystem work stays in `deploy`.

use super::super::repository::lock;
use super::super::types::{SkillDeployMode, SkillError};
use crate::storage::Storage;
use rusqlite::{OptionalExtension, params};

/// One stored deployment before filesystem classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StoredDeployment {
    pub(super) skill_id: String,
    pub(super) agent_id: String,
    pub(super) config_root: String,
    pub(super) target_name: String,
    pub(super) mode: SkillDeployMode,
    pub(super) skill_revision: i64,
    pub(super) content_digest: String,
    pub(super) deployed_at: i64,
}

/// Values written when a deploy succeeds.
pub(super) struct DeploymentUpsert<'a> {
    pub(super) skill_id: &'a str,
    pub(super) agent_id: &'a str,
    pub(super) config_root: &'a str,
    pub(super) target_name: &'a str,
    pub(super) mode: SkillDeployMode,
    pub(super) skill_revision: i64,
    pub(super) content_digest: &'a str,
    pub(super) deployed_at: i64,
}

fn mode_value(mode: SkillDeployMode) -> &'static str {
    match mode {
        SkillDeployMode::Symlink => "symlink",
        SkillDeployMode::Copy => "copy",
    }
}

fn mode_from_stored(value: &str) -> Result<SkillDeployMode, SkillError> {
    match value {
        "symlink" => Ok(SkillDeployMode::Symlink),
        "copy" => Ok(SkillDeployMode::Copy),
        _ => Err(SkillError::InvalidStoredSkill),
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> Result<StoredDeployment, rusqlite::Error> {
    let mode: String = row.get(4)?;
    Ok(StoredDeployment {
        skill_id: row.get(0)?,
        agent_id: row.get(1)?,
        config_root: row.get(2)?,
        target_name: row.get(3)?,
        mode: mode_from_stored(&mode).map_err(|_| rusqlite::Error::InvalidQuery)?,
        skill_revision: row.get(5)?,
        content_digest: row.get(6)?,
        deployed_at: row.get(7)?,
    })
}

const SELECT_COLS: &str = "skill_id, agent_id, config_root, target_name, mode,
    skill_revision, content_digest, deployed_at";

/// Read the deployment for one skill at one Agent root, if any.
pub(super) fn get_deployment(
    storage: &Storage,
    skill_id: &str,
    agent_id: &str,
    config_root: &str,
) -> Result<Option<StoredDeployment>, SkillError> {
    let connection = lock(storage)?;
    connection
        .query_row(
            &format!(
                "SELECT {SELECT_COLS} FROM skill_deployment
                 WHERE skill_id=?1 AND agent_id=?2 AND config_root=?3"
            ),
            params![skill_id, agent_id, config_root],
            map_row,
        )
        .optional()
        .map_err(|_| SkillError::ReadFailed)
}

/// Insert or replace the ownership row after a successful filesystem write.
pub(super) fn upsert_deployment(
    storage: &Storage,
    row: &DeploymentUpsert<'_>,
) -> Result<(), SkillError> {
    let connection = lock(storage)?;
    connection
        .execute(
            "INSERT INTO skill_deployment(
                skill_id, agent_id, config_root, target_name, mode,
                skill_revision, content_digest, deployed_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(agent_id, config_root, target_name) DO UPDATE SET
                skill_id=excluded.skill_id,
                mode=excluded.mode,
                skill_revision=excluded.skill_revision,
                content_digest=excluded.content_digest,
                deployed_at=excluded.deployed_at",
            params![
                row.skill_id,
                row.agent_id,
                row.config_root,
                row.target_name,
                mode_value(row.mode),
                row.skill_revision,
                row.content_digest,
                row.deployed_at,
            ],
        )
        .map_err(|_| SkillError::WriteFailed)?;
    Ok(())
}

/// Remove the ownership row after a successful revoke (or detached clear).
pub(super) fn delete_deployment(
    storage: &Storage,
    skill_id: &str,
    agent_id: &str,
    config_root: &str,
) -> Result<(), SkillError> {
    let connection = lock(storage)?;
    let changed = connection
        .execute(
            "DELETE FROM skill_deployment
             WHERE skill_id=?1 AND agent_id=?2 AND config_root=?3",
            params![skill_id, agent_id, config_root],
        )
        .map_err(|_| SkillError::WriteFailed)?;
    if changed == 0 {
        return Err(SkillError::NotFound);
    }
    Ok(())
}

/// Every deployment under one Agent configuration root.
pub(super) fn list_deployments_for_root(
    storage: &Storage,
    agent_id: &str,
    config_root: &str,
) -> Result<Vec<StoredDeployment>, SkillError> {
    let connection = lock(storage)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {SELECT_COLS} FROM skill_deployment
             WHERE agent_id=?1 AND config_root=?2
             ORDER BY target_name, skill_id"
        ))
        .map_err(|_| SkillError::ReadFailed)?;
    let rows = statement
        .query_map(params![agent_id, config_root], map_row)
        .map_err(|_| SkillError::ReadFailed)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SkillError::ReadFailed)?;
    Ok(rows)
}

/// Every deployment recorded for one central skill.
pub(super) fn list_deployments_for_skill(
    storage: &Storage,
    skill_id: &str,
) -> Result<Vec<StoredDeployment>, SkillError> {
    let connection = lock(storage)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {SELECT_COLS} FROM skill_deployment
             WHERE skill_id=?1
             ORDER BY agent_id, config_root"
        ))
        .map_err(|_| SkillError::ReadFailed)?;
    let rows = statement
        .query_map(params![skill_id], map_row)
        .map_err(|_| SkillError::ReadFailed)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SkillError::ReadFailed)?;
    Ok(rows)
}

/// Whether any Agent still holds a deployment of this skill.
pub(super) fn skill_has_deployments(storage: &Storage, skill_id: &str) -> Result<bool, SkillError> {
    let connection = lock(storage)?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skill_deployment WHERE skill_id=?1)",
            [skill_id],
            |row| row.get(0),
        )
        .map_err(|_| SkillError::ReadFailed)
}
