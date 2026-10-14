//! Central MCP domain facade. Definitions never execute their servers; secret values
//! stay in OS storage while SQLite holds metadata and opaque references.

mod types;
mod validation;
pub use types::{
    ConnectionInput, ConnectionRecord, DeleteMcpRequest, DeletedMcp, McpError, McpPage, McpRecord,
    SaveMcpRequest, SecretInput,
};
mod repository;
mod service;
pub use repository::{get_definition, list_definitions};
pub use service::{
    cleanup_credentials, cleanup_deleted_credentials, delete_definition, save_definition,
};

#[cfg(test)]
mod tests;
