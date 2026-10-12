//! Thin Agent IPC: static discovery and a separately confirmed version launch.
//! No storage or credential access. The launched CLI can have its own side effects.

use crate::agents::{
    self, AgentObservation, AgentVersionResult, DiscoveryEnvironment, DiscoveryError,
    DiscoveryInput, VersionProbeError, VersionProbeGate, VersionProbeInput,
};
use tauri::Manager;

fn environment(app: &tauri::AppHandle) -> Result<DiscoveryEnvironment, ()> {
    let home = app.path().home_dir().map_err(|_| ())?;
    Ok(DiscoveryEnvironment {
        home,
        path: std::env::var_os("PATH"),
        pi_directory: std::env::var_os("PI_CODING_AGENT_DIR"),
        grok_directory: std::env::var_os("GROK_HOME"),
        claude_directory: std::env::var_os("CLAUDE_CONFIG_DIR"),
        codex_directory: std::env::var_os("CODEX_HOME"),
        opencode_directory: std::env::var_os("OPENCODE_CONFIG_DIR"),
        xdg_config_directory: std::env::var_os("XDG_CONFIG_HOME"),
        hermes_directory: std::env::var_os("HERMES_HOME"),
        local_app_data: std::env::var_os("LOCALAPPDATA"),
    })
}

/// Observe six entries without running them or reading configuration contents.
/// Filesystem work runs off the async executor; errors contain no I/O diagnostics.
#[tauri::command]
pub(crate) async fn discover_agents(
    app: tauri::AppHandle,
    input: DiscoveryInput,
) -> Result<Vec<AgentObservation>, DiscoveryError> {
    let environment = environment(&app).map_err(|_| DiscoveryError::LocationUnavailable)?;
    tauri::async_runtime::spawn_blocking(move || agents::discover_agents(&environment, &input))
        .await
        .map_err(|_| DiscoveryError::OperationFailed)?
}

/// Run only the fixed version command after explicit UI confirmation.
/// Returns bounded version data or sanitized failure. This is not a sandbox;
/// the selected program can access user files/network or start descendants.
#[tauri::command]
pub(crate) async fn probe_agent_version(
    app: tauri::AppHandle,
    input: VersionProbeInput,
    gate: tauri::State<'_, VersionProbeGate>,
) -> Result<AgentVersionResult, VersionProbeError> {
    let environment = environment(&app).map_err(|_| VersionProbeError::OperationFailed)?;
    agents::probe_agent_version(environment, input, gate.inner()).await
}
