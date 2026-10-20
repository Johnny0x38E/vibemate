//! Non-secret Claude Code starting-choice IPC; no Agent file or credential access.

use crate::agents::claude_code::preference::{
    self, EffortPreference, PreferenceError, SaveEffortPreference,
};
use crate::models::GetModelSnapshotRequest;
use crate::storage::{Storage, StorageStatus};
use tauri::Manager;

fn with_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, PreferenceError>,
) -> Result<T, PreferenceError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(PreferenceError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| PreferenceError::StorageUnavailable)?;
    operation(storage)
}

/// Read one binding's saved draft and current model metadata consistently.
/// Does not write defaults or inspect native configuration.
#[tauri::command]
pub(crate) async fn get_claude_effort_preference(
    app: tauri::AppHandle,
    request: GetModelSnapshotRequest,
) -> Result<EffortPreference, PreferenceError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_storage(&app, |storage| {
            preference::load_preference(storage, &request)
        })
    })
    .await
    .map_err(|_| PreferenceError::OperationFailed)?
}

/// Remember a draft only when its model and preference tokens still match.
/// The domain transaction owns validation and rollback; this never applies config.
#[tauri::command]
pub(crate) async fn save_claude_effort_preference(
    app: tauri::AppHandle,
    request: SaveEffortPreference,
) -> Result<EffortPreference, PreferenceError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_storage(&app, |storage| {
            preference::save_preference(storage, &request)
        })
    })
    .await
    .map_err(|_| PreferenceError::OperationFailed)?
}
