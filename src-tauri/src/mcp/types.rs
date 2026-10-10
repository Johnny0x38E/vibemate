//! Central MCP requests, safe records, stable errors, and internal validated metadata.

use crate::credentials::{CredentialError, Secret};
use serde::{Deserialize, Serialize};

/// Safe failures without SQL, paths, URLs, field values or credential payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpError {
    /// Private storage is unavailable.
    StorageUnavailable,
    /// Reading persisted metadata failed.
    ReadFailed,
    /// A known failed write left the definition unchanged.
    WriteFailed,
    /// The request shape, limit or cursor is invalid.
    InvalidRequest,
    /// The display name is invalid.
    DisplayNameInvalid,
    /// The portable server identifier is invalid.
    ServerNameInvalid,
    /// Another definition owns this server namespace.
    ServerNameTaken,
    /// A stdio executable is missing or invalid.
    CommandInvalid,
    /// Arguments exceed bounds or contain invalid characters.
    ArgsInvalid,
    /// A working directory is invalid.
    CwdInvalid,
    /// An HTTP URL violates the supported literal URL rules.
    UrlInvalid,
    /// Secret field names are invalid, duplicated or reserved.
    FieldNameInvalid,
    /// A new field needs a value.
    SecretRequired,
    /// A secret exceeds bounds or uses unsupported dynamic syntax.
    SecretInvalid,
    /// The OS store is unavailable.
    CredentialStoreUnavailable,
    /// Access to the OS store was denied.
    CredentialStoreAccessDenied,
    /// An OS credential operation failed.
    CredentialStoreFailed,
    /// The definition no longer exists.
    NotFound,
    /// Another writer changed this definition.
    RevisionConflict,
    /// Persisted metadata failed validation and was left unchanged.
    InvalidStoredDefinition,
    /// Commit or compensation could not establish a clean result; reload first.
    OutcomeUnknown,
    /// A worker task failed, possibly after a mutation; reload first.
    OperationFailed,
}

impl From<CredentialError> for McpError {
    fn from(error: CredentialError) -> Self {
        match error {
            CredentialError::Unavailable => Self::CredentialStoreUnavailable,
            CredentialError::AccessDenied => Self::CredentialStoreAccessDenied,
            _ => Self::CredentialStoreFailed,
        }
    }
}

/// One secret field. `None` retains the matching existing field without loading it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretInput {
    /// Environment variable or HTTP header name.
    pub name: String,
    /// New literal value; never serialized to an IPC response.
    pub value: Option<Secret>,
}

/// Transport-specific input, rejecting unsupported and mixed transport fields.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConnectionInput {
    /// A local executable definition, not a shell command to execute.
    Stdio {
        /// Executable name or path.
        command: String,
        /// Ordered literal arguments.
        args: Vec<String>,
        /// Optional working directory, resolved by a future Agent adapter.
        cwd: Option<String>,
        /// Environment values stored only in the OS store.
        env: Vec<SecretInput>,
    },
    /// Streamable HTTP endpoint definition, with no connection check.
    Http {
        /// HTTPS, or HTTP on a loopback host, without query or credentials.
        url: String,
        /// Header values stored only in the OS store.
        headers: Vec<SecretInput>,
    },
}

/// Create when both identity fields are null; update only at the expected revision.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveMcpRequest {
    /// Existing stable identity, or null for creation.
    pub id: Option<String>,
    /// Existing revision, or null for creation.
    pub expected_revision: Option<i64>,
    /// User-facing label.
    pub display_name: String,
    /// Portable Agent configuration key.
    pub server_name: String,
    /// Central definition enablement, never a connection status.
    pub enabled: bool,
    /// Exactly one supported transport.
    pub connection: ConnectionInput,
}

/// Safe transport metadata; values and credential references never cross IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConnectionRecord {
    /// Local process metadata, displayed without executing it.
    Stdio {
        /// Executable name or path.
        command: String,
        /// Ordered arguments.
        args: Vec<String>,
        /// Optional working directory.
        cwd: Option<String>,
        /// Configured environment names, not values.
        env: Vec<String>,
    },
    /// HTTP metadata, displayed without a request.
    Http {
        /// Literal endpoint.
        url: String,
        /// Configured header names, not values.
        headers: Vec<String>,
    },
}

/// A persisted, non-secret definition returned to the interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRecord {
    /// Random stable identity, independent of editable names.
    pub id: String,
    /// User-facing label.
    pub display_name: String,
    /// Portable server identifier.
    pub server_name: String,
    /// Enablement of this central definition only.
    pub enabled: bool,
    /// Monotonic optimistic-concurrency revision.
    pub revision: i64,
    /// Transport metadata with names only for secret fields.
    pub connection: ConnectionRecord,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Last save timestamp in Unix milliseconds.
    pub updated_at_ms: i64,
    /// Old, unused credentials still need deletion; current values are safe.
    pub cleanup_pending: bool,
}

/// One bounded page, ordered by the immutable ID.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPage {
    /// Full metadata rows, without credential values.
    pub items: Vec<McpRecord>,
    /// Last identity, or null when no further page exists.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StoredConnection {
    Stdio {
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
    },
    Http {
        url: String,
    },
}

pub(super) struct Validated {
    pub(super) display_name: String,
    pub(super) server_name: String,
    pub(super) namespace: String,
    pub(super) connection: StoredConnection,
    pub(super) fields: Vec<SecretInput>,
    pub(super) kind: &'static str,
}

/// Validated identity of a central MCP definition, independent of ProviderId.
/// Its wire and database representation stays the existing 32-character string.
pub(super) struct McpId(String);

impl McpId {
    /// Accept only the random-ID format generated for central definitions.
    pub(super) fn parse(value: &str) -> Option<Self> {
        crate::shared::is_random_id(value).then(|| Self(value.to_string()))
    }

    /// Return the validated string without changing its database or wire format.
    pub(super) fn into_string(self) -> String {
        self.0
    }
}
