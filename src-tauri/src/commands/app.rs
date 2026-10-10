//! Application metadata, fixed repository action and log-access IPC commands.

use crate::log_access::{self, LogAccess, LogAccessError, LogLocation};
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

/// Return the configured log paths and startup file-logging status without I/O.
#[tauri::command]
pub(crate) fn get_log_location(
    access: tauri::State<'_, LogAccess>,
) -> Result<LogLocation, LogAccessError> {
    access.location()
}

/// Open only the fixed app log file with a platform text tool.
/// No frontend path or executable is accepted. Files are never created here.
#[tauri::command]
pub(crate) async fn open_log_file(app: tauri::AppHandle) -> Result<(), LogAccessError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let access = app
            .try_state::<LogAccess>()
            .ok_or(LogAccessError::PathUnavailable)?;
        access.open_file_with(log_access::open_in_text_tool)
    })
    .await
    .map_err(|_| LogAccessError::OperationFailed)
    .and_then(|result| result);
    if let Err(error) = result {
        log::warn!(target: "vibemate", "event=command_failed operation=open_log_file code={error:?}");
    }
    result
}

/// Open only the fixed app log directory with the system file manager.
/// Returns safe error codes without creating a missing directory.
#[tauri::command]
pub(crate) async fn open_log_directory(app: tauri::AppHandle) -> Result<(), LogAccessError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let access = app
            .try_state::<LogAccess>()
            .ok_or(LogAccessError::PathUnavailable)?;
        access.open_directory_with(log_access::open_in_file_manager)
    })
    .await
    .map_err(|_| LogAccessError::OperationFailed)
    .and_then(|result| result);
    if let Err(error) = result {
        log::warn!(target: "vibemate", "event=command_failed operation=open_log_directory code={error:?}");
    }
    result
}

/// Open the project's fixed GitHub repository in the system browser.
///
/// Accepts no frontend URL or executable. Returns a safe code if the operating
/// system cannot open the browser or the blocking task cannot complete.
#[tauri::command]
pub(crate) async fn open_project_repository() -> Result<(), &'static str> {
    tauri::async_runtime::spawn_blocking(|| {
        tauri_plugin_opener::open_url("https://github.com/Johnny0x38E/vibemate", None::<&str>)
            .map_err(|_| "open_failed")
    })
    .await
    .map_err(|_| "open_failed")?
}
