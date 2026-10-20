//! Pi model preview/application IPC. All paths come from the native environment;
//! reviewed paths are concurrency tokens, never arbitrary filesystem targets.
use super::agents::{environment, model_failure, provider_failure};
use super::state::{with_apply_storage, with_credential_lock, with_plan_storage};
use crate::agents::{ConfigApplied, ConfigApplyError, pi};
use crate::config_plan::PlanError;
use crate::models::{GetModelSnapshotRequest, ModelSnapshot};
use crate::providers::ProviderRecord;
use crate::storage::Storage;
use tauri::Manager;

/// Read both stored identities; their individual tokens are rechecked on apply.
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

/// Preview models.json/settings.json without reading OS credentials or launching Pi.
#[tauri::command]
pub(crate) async fn plan_pi_settings(
    app: tauri::AppHandle,
    provider_id: String,
    model_id: String,
    entry_path: Option<String>,
    thinking_level: Option<pi::PiThinkingLevel>,
) -> Result<pi::PiPlan, PlanError> {
    let environment = environment(&app).map_err(|()| PlanError::LocationUnavailable)?;
    crate::models::parse_provider_id(&provider_id).map_err(|_| PlanError::InvalidRequest)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = pi::configuration_root(&environment, entry_path)?;
        with_plan_storage(&app, |storage| {
            let (provider, model) = selection(storage, &provider_id, model_id)?;
            pi::plan_pi_settings(storage, &root, &provider, &model, thinking_level)
        })
    })
    .await
    .map_err(|_| PlanError::OperationFailed)?
}

/// Recheck the whole preview, read the key and write through private backups.
#[tauri::command]
pub(crate) async fn apply_pi_settings(
    app: tauri::AppHandle,
    input: pi::ApplyPiSettingsRequest,
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
        let root = pi::configuration_root(&environment, input.entry_path)
            .map_err(ConfigApplyError::from_plan)?;
        // Serializes credential replacement and both adapters' applications.
        with_credential_lock(&app, ConfigApplyError::OperationFailed, || {
            with_apply_storage(&app, |storage| {
                let (provider, model) = selection(storage, &input.provider_id, input.model_id)
                    .map_err(ConfigApplyError::from_plan)?;
                pi::apply_pi_settings(
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

/// Preview recovery of a Pi backup; another adapter's operation ID is refused.
#[tauri::command]
pub(crate) async fn plan_pi_restore(
    app: tauri::AppHandle,
    operation_id: String,
) -> Result<crate::agents::ClaudeCodeRestorePlan, crate::agents::ClaudeCodeRestoreError> {
    tauri::async_runtime::spawn_blocking(move || {
        super::state::with_restore_storage(&app, |storage| {
            crate::agents::plan_config_restore(
                storage,
                &operation_id,
                crate::config_apply::OperationKind::PiSettings,
            )
        })
    })
    .await
    .map_err(|_| crate::agents::ClaudeCodeRestoreError::OperationFailed)?
}

/// Restore both files from the reviewed Pi write, checking subsequent edits.
#[tauri::command]
pub(crate) async fn apply_pi_restore(
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
                crate::config_apply::OperationKind::PiSettings,
            )
        })
    })
    .await
    .map_err(|_| crate::agents::ClaudeCodeRestoreError::OperationFailed)?
}
