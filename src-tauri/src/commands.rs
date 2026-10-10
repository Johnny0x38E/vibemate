//! Desktop IPC composition. Each feature owns its command module; managed-state
//! coordination is shared here, while domain behavior remains independent of Tauri.

pub(crate) mod app;
pub(crate) mod mcp;
pub(crate) mod models;
pub(crate) mod preferences;
pub(crate) mod providers;
mod state;

pub(crate) use state::CredentialLock;
