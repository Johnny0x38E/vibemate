//! Pure Pi MCP deployment preflight. Only a new redacted entry leaves this module;
//! existing native content, credentials, filesystem access and writes stay out.

use serde::Serialize;
use serde_json::{Value, json};

use super::{ConnectionRecord, McpError, get_definition};
use crate::{config_plan::SECRET_PLACEHOLDER, storage::Storage};

/// Safe preflight failures without parser diagnostics or native file content.
#[derive(Debug, PartialEq, Eq)]
pub enum PiMcpPreflightError {
    /// Central metadata could not be read or its revision is stale.
    Definition(McpError),
    /// Native bytes exceed the limit or are not a supported JSON container.
    InvalidConfig,
    /// An existing entry owns this name or its hyphen/underscore namespace.
    NameConflict,
}

/// Sanitized proposal, not an approval token or evidence of a connected server.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiMcpPreflight {
    /// Central definition used for this proposal.
    pub definition_id: String,
    /// Central revision that must be checked again before any future write.
    pub revision: i64,
    /// Native key to add under `mcpServers`.
    pub server_name: String,
    /// New entry only; all environment/header values are placeholders.
    pub redacted_entry: Value,
    /// Pi resolves relative cwd against its session, not the central library.
    pub session_relative_cwd: bool,
    /// Pi expands a leading `~/` in command, arguments and cwd at runtime.
    pub expands_home: bool,
}

/// Check a proposed first registration against supplied user-level config bytes.
///
/// `None` means the file is absent; empty bytes are an invalid existing file.
/// Bytes must come from a Rust-selected target when this is wired to IPC. This
/// function does not read files, access credentials, contact or execute servers,
/// or modify storage. Project overrides and replacement extensions can still
/// prevent Pi from using a future registration; `/reload` is required afterward.
///
/// # Errors
/// Rejects missing/stale central definitions, oversized or malformed containers,
/// and every existing name collision, including disabled or invalid entries.
/// Existing entries cannot be adopted without a verified ownership record.
pub fn preflight_pi_registration(
    storage: &Storage,
    definition_id: &str,
    expected_revision: i64,
    native_bytes: Option<&[u8]>,
) -> Result<PiMcpPreflight, PiMcpPreflightError> {
    use PiMcpPreflightError::{Definition, InvalidConfig, NameConflict};
    let record = get_definition(storage, definition_id).map_err(Definition)?;
    if record.revision != expected_revision {
        return Err(Definition(McpError::RevisionConflict));
    }
    if let Some(bytes) = native_bytes {
        if bytes.len() as u64 > crate::config_plan::MAX_TARGET_BYTES {
            return Err(InvalidConfig);
        }
        // Pi strips a UTF-8 BOM before JSON.parse; accept the same file shape.
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let value: Value = serde_json::from_slice(bytes).map_err(|_| InvalidConfig)?;
        let root = value.as_object().ok_or(InvalidConfig)?;
        if let Some(servers) = root.get("mcpServers") {
            let servers = servers.as_object().ok_or(InvalidConfig)?;
            let namespace = record.server_name.replace('-', "_");
            if servers
                .keys()
                .any(|name| name.replace('-', "_") == namespace)
            {
                return Err(NameConflict);
            }
        }
    }
    let mut session_relative_cwd = false;
    let mut expands_home = false;
    let redacted_entry = match record.connection {
        ConnectionRecord::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            session_relative_cwd = cwd.as_ref().is_some_and(|value| {
                !std::path::Path::new(value).is_absolute() && !value.starts_with("~/")
            });
            expands_home = command.starts_with("~/")
                || args.iter().any(|value| value.starts_with("~/"))
                || cwd.as_ref().is_some_and(|value| value.starts_with("~/"));
            let env: serde_json::Map<String, Value> = env
                .into_iter()
                .map(|name| (name, json!(SECRET_PLACEHOLDER)))
                .collect();
            let mut entry =
                json!({"command": command, "args": args, "env": env, "enabled": record.enabled});
            if let Some(cwd) = cwd {
                entry["cwd"] = json!(cwd);
            }
            entry
        }
        ConnectionRecord::Http { url, headers } => {
            let headers: serde_json::Map<String, Value> = headers
                .into_iter()
                .map(|name| (name, json!(SECRET_PLACEHOLDER)))
                .collect();
            json!({"url": url, "headers": headers, "enabled": record.enabled})
        }
    };
    Ok(PiMcpPreflight {
        definition_id: record.id,
        revision: record.revision,
        server_name: record.server_name,
        redacted_entry,
        session_relative_cwd,
        expands_home,
    })
}
