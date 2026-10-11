//! Thin read-only Agent discovery IPC. Filesystem work runs on the blocking pool;
//! no database, credential state, executable launch or Agent writes are involved.

use crate::agents::{self, AgentObservation, DiscoveryEnvironment, DiscoveryError, DiscoveryInput};
use tauri::Manager;

/// Observe two Agent entries using the desktop environment and optional absolute
/// entry paths. Returns only bounded metadata or sanitized failures. This command
/// never executes the discovered entry or reads Agent configuration contents.
#[tauri::command]
pub(crate) async fn discover_agents(
    app: tauri::AppHandle,
    input: DiscoveryInput,
) -> Result<Vec<AgentObservation>, DiscoveryError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|_| DiscoveryError::LocationUnavailable)?;
    let environment = DiscoveryEnvironment {
        home,
        path: std::env::var_os("PATH"),
        pi_directory: std::env::var_os("PI_CODING_AGENT_DIR"),
        grok_directory: std::env::var_os("GROK_HOME"),
    };
    tauri::async_runtime::spawn_blocking(move || agents::discover_agents(&environment, &input))
        .await
        .map_err(|_| DiscoveryError::OperationFailed)?
}
