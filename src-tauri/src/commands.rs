//! Read-only desktop commands exposed to the React interface.
//!
//! Keep this boundary thin: future configuration behavior belongs in feature
//! modules so it can be tested without starting a Tauri window.

use serde::Serialize;

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
