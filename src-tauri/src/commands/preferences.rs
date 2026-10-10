//! Language and appearance preference IPC commands.

use crate::appearance::{self, AppearancePreference};
use crate::settings::{self, LocalePreference, SettingsError};
use crate::storage::StorageStatus;
use tauri::Manager;

/// Read the saved language choice without writing defaults. Returns safe error
/// codes if startup storage or the read fails; never exposes SQLite diagnostics.
#[tauri::command]
pub(crate) async fn get_locale_preference(
    app: tauri::AppHandle,
) -> Result<LocalePreference, SettingsError> {
    // SQLite can wait for another process's lock. Keep that blocking wait off both
    // the window thread and async executor; the owned handle lives until it ends.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::load_locale_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_locale_preference", result)
}

/// Save a validated language choice, returning it after the single-row commit.
/// Storage/write failures are safe codes; a task failure has an unknown outcome
/// and callers should reload before claiming which preference is persisted.
#[tauri::command]
pub(crate) async fn save_locale_preference(
    app: tauri::AppHandle,
    preference: LocalePreference,
) -> Result<LocalePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::save_locale_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_locale_preference", result)
}

/// Read the validated appearance/theme pair without writing a default row.
/// Database work uses the blocking pool; only safe settings error codes escape.
#[tauri::command]
pub(crate) async fn get_appearance_preference(
    app: tauri::AppHandle,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::load_appearance_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_appearance_preference", result)
}

/// Save a typed pair atomically, returning the committed choice. A task failure
/// has an unknown outcome and requires a read before another save is attempted.
#[tauri::command]
pub(crate) async fn save_appearance_preference(
    app: tauri::AppHandle,
    preference: AppearancePreference,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::save_appearance_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_appearance_preference", result)
}
