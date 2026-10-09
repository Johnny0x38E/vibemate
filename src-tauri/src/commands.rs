//! Desktop metadata and preference commands exposed to the React interface.
//!
//! Keep this boundary thin: future configuration behavior belongs in feature
//! modules so it can be tested without starting a Tauri window.

use crate::settings::{self, LocalePreference, SettingsError};
use crate::storage::StorageStatus;
use serde::Serialize;
use tauri::Manager;

/// Application metadata returned to the frontend without reading user config.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    name: &'static str,
    version: &'static str,
}

/// Return build metadata. This command has no filesystem or network side effects.
#[tauri::command]
pub(crate) fn get_app_info() -> AppInfo {
    AppInfo {
        name: "vibemate",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Read the saved language choice without writing defaults. Returns safe error
/// codes if startup storage or the read fails; never exposes SQLite diagnostics.
#[tauri::command]
pub(crate) async fn get_locale_preference(
    app: tauri::AppHandle,
) -> Result<LocalePreference, SettingsError> {
    // SQLite can wait for another process's lock. Keep that blocking wait off both
    // the window thread and async executor; the owned handle lives until it ends.
    tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::load_locale_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)?
}

/// Save a validated language choice, returning it after the single-row commit.
/// Storage/write failures are safe codes; a task failure has an unknown outcome
/// and callers should reload before claiming which preference is persisted.
#[tauri::command]
pub(crate) async fn save_locale_preference(
    app: tauri::AppHandle,
    preference: LocalePreference,
) -> Result<LocalePreference, SettingsError> {
    tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::save_locale_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)?
}
