//! Grok Build model preview, application and recovery IPC. All paths come from the
//! native environment; a reviewed path is a concurrency token, never a write target.
use super::agents::{environment, model_failure, provider_failure};
use super::state::{with_apply_storage, with_credential_lock, with_plan_storage};
use crate::agents::{ConfigApplied, ConfigApplyError, grok_build};
use crate::config_plan::PlanError;
use crate::models::{GetModelSnapshotRequest, ModelSnapshot};
use crate::providers::ProviderRecord;
use crate::storage::Storage;
use tauri::Manager;

/// Read both stored identities; their individual tokens are rechecked by a plan.
fn selection(
    storage: &Storage,
    provider_id: &str,
    model_id: String,
) -> Result<(ProviderRecord, ModelSnapshot), PlanError> {
    let provider =
        crate::providers::get_provider(storage, provider_id).map_err(provider_failure)?;
    let model = crate::models::get_model_snapshot(
        storage,
        &GetModelSnapshotRequest {
            provider_id: provider_id.to_owned(),
            model_id,
        },
    )
    .map_err(model_failure)?;
    Ok((provider, model))
}

/// Preview `config.toml` without reading OS credentials or launching Grok.
#[tauri::command]
pub(crate) async fn plan_grok_settings(
    app: tauri::AppHandle,
    provider_id: String,
    model_id: String,
    entry_path: Option<String>,
) -> Result<grok_build::GrokPlan, PlanError> {
    let environment = environment(&app).map_err(|()| PlanError::LocationUnavailable)?;
    crate::models::parse_provider_id(&provider_id).map_err(|_| PlanError::InvalidRequest)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = grok_build::configuration_root(&environment, entry_path)?;
        with_plan_storage(&app, |storage| {
            let (provider, model) = selection(storage, &provider_id, model_id)?;
            grok_build::plan_grok_settings(storage, &root, &provider, &model)
        })
    })
    .await
    .map_err(|_| PlanError::OperationFailed)?
}

/// Recheck the whole preview, read the key and write through a private backup.
#[tauri::command]
pub(crate) async fn apply_grok_settings(
    app: tauri::AppHandle,
    input: grok_build::ApplyGrokSettingsRequest,
) -> Result<ConfigApplied, ConfigApplyError> {
    let environment = environment(&app).map_err(|()| ConfigApplyError::LocationUnavailable)?;
    crate::models::parse_provider_id(&input.provider_id)
        .map_err(|_| ConfigApplyError::InvalidRequest)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ConfigApplyError::LocationUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        let root = grok_build::configuration_root(&environment, input.entry_path)
            .map_err(ConfigApplyError::from_plan)?;
        // Serializes credential replacement and every adapter's application.
        with_credential_lock(&app, ConfigApplyError::OperationFailed, || {
            with_apply_storage(&app, |storage| {
                let (provider, model) = selection(storage, &input.provider_id, input.model_id)
                    .map_err(ConfigApplyError::from_plan)?;
                grok_build::apply_grok_settings(
                    storage,
                    &crate::credentials::OsCredentialStore,
                    &root,
                    &provider,
                    &model,
                    &input.reviewed,
                    backups,
                )
            })
        })
    })
    .await
    .map_err(|_| ConfigApplyError::OperationFailed)?
}

/// Preview recovery of a Grok backup; another adapter's operation ID is refused.
#[tauri::command]
pub(crate) async fn plan_grok_restore(
    app: tauri::AppHandle,
    operation_id: String,
) -> Result<crate::agents::ClaudeCodeRestorePlan, crate::agents::ClaudeCodeRestoreError> {
    tauri::async_runtime::spawn_blocking(move || {
        super::state::with_restore_storage(&app, |storage| {
            crate::agents::plan_config_restore(
                storage,
                &operation_id,
                crate::config_apply::OperationKind::GrokSettings,
            )
        })
    })
    .await
    .map_err(|_| crate::agents::ClaudeCodeRestoreError::OperationFailed)?
}

/// Restore the settings file from the reviewed Grok write, checking later edits.
#[tauri::command]
pub(crate) async fn apply_grok_restore(
    app: tauri::AppHandle,
    input: crate::agents::ApplyClaudeCodeRestoreRequest,
) -> Result<crate::config_apply::RestoreOutcome, crate::agents::ClaudeCodeRestoreError> {
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| crate::agents::ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        super::state::with_restore_storage(&app, |storage| {
            crate::agents::apply_config_restore(
                storage,
                &input,
                &backups,
                crate::config_apply::OperationKind::GrokSettings,
            )
        })
    })
    .await
    .map_err(|_| crate::agents::ClaudeCodeRestoreError::OperationFailed)?
}
