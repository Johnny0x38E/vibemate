//! Central MCP definitions: validation, SQLite metadata and OS credential references.
//!
//! This module never launches a server, performs HTTP, or changes Agent files.
//! Replacement values use fresh credential entries; SQLite revision checks and
//! compensating deletions keep failed saves from overwriting existing secrets.

use crate::credentials::{CredentialError, CredentialStore, Secret};
use crate::providers::{ProviderId, validate_display_name};
use crate::storage::Storage;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
enum StoredConnection {
    Stdio {
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
    },
    Http {
        url: String,
    },
}
struct Validated {
    display_name: String,
    server_name: String,
    namespace: String,
    connection: StoredConnection,
    fields: Vec<SecretInput>,
    kind: &'static str,
}
fn lock(storage: &Storage) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, McpError> {
    storage.lock().map_err(|_| McpError::StorageUnavailable)
}
fn identity(value: &str) -> Result<(), McpError> {
    ProviderId::parse(value)
        .map(|_| ())
        .ok_or(McpError::InvalidRequest)
}
fn bounded_text(value: &str, max: usize, nonempty: bool) -> bool {
    (!nonempty || !value.trim().is_empty())
        && value.chars().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() || crate::providers::is_hidden_format_character(c))
}
fn server_namespace(name: &str) -> Result<String, McpError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(McpError::ServerNameInvalid);
    }
    Ok(name.replace('-', "_"))
}
fn normalize_url(value: &str) -> Result<String, McpError> {
    if !bounded_text(value, 2048, true) || value.chars().any(char::is_whitespace) {
        return Err(McpError::UrlInvalid);
    }
    let url = url::Url::parse(value).map_err(|_| McpError::UrlInvalid)?;
    let loopback = match url.host() {
        Some(url::Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if url.host().is_none()
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(McpError::UrlInvalid);
    }
    Ok(url.to_string())
}
fn field_name(name: &str, kind: &str) -> Result<String, McpError> {
    if name.is_empty() || name.len() > 128 {
        return Err(McpError::FieldNameInvalid);
    }
    if kind == "env" {
        if !name
            .bytes()
            .enumerate()
            .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        {
            return Err(McpError::FieldNameInvalid);
        }
        Ok(name.to_string())
    } else {
        if !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
        {
            return Err(McpError::FieldNameInvalid);
        }
        let name = name.to_ascii_lowercase();
        if [
            "host",
            "connection",
            "content-length",
            "transfer-encoding",
            "mcp-session-id",
            "mcp-protocol-version",
        ]
        .contains(&name.as_str())
        {
            return Err(McpError::FieldNameInvalid);
        }
        Ok(name)
    }
}
fn validate(
    request: SaveMcpRequest,
) -> Result<(Option<String>, Option<i64>, bool, Validated), McpError> {
    if let Some(id) = &request.id {
        identity(id)?;
    }
    if request.id.is_some() != request.expected_revision.is_some()
        || request
            .expected_revision
            .is_some_and(|r| r < 1 || r == i64::MAX)
    {
        return Err(McpError::InvalidRequest);
    }
    let display_name =
        validate_display_name(&request.display_name).map_err(|_| McpError::DisplayNameInvalid)?;
    let server_name = request.server_name.trim().to_string();
    let namespace = server_namespace(&server_name)?;
    let (connection, mut fields, kind) = match request.connection {
        ConnectionInput::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let command = command.trim().to_string();
            if !bounded_text(&command, 2048, true) {
                return Err(McpError::CommandInvalid);
            }
            if args.len() > 64 || args.iter().any(|a| !bounded_text(a, 2048, false)) {
                return Err(McpError::ArgsInvalid);
            }
            let cwd = cwd.map(|p| p.trim().to_string());
            if cwd.as_ref().is_some_and(|p| !bounded_text(p, 2048, true)) {
                return Err(McpError::CwdInvalid);
            }
            (StoredConnection::Stdio { command, args, cwd }, env, "env")
        }
        ConnectionInput::Http { url, headers } => (
            StoredConnection::Http {
                url: normalize_url(url.trim())?,
            },
            headers,
            "header",
        ),
    };
    if fields.len() > 32 {
        return Err(McpError::InvalidRequest);
    }
    let mut seen = BTreeSet::new();
    for field in &mut fields {
        field.name = field_name(&field.name, kind)?;
        if !seen.insert(field.name.clone()) {
            return Err(McpError::FieldNameInvalid);
        }
        if let Some(value) = &field.value {
            let raw = value.expose();
            if raw.is_empty()
                || raw.encode_utf16().count() > crate::credentials::MAX_SECRET_UTF16_UNITS
                || raw.chars().any(char::is_control)
                || raw.trim_start().starts_with('!')
                || raw.contains("${")
            {
                return Err(McpError::SecretInvalid);
            }
        }
    }
    Ok((
        request.id,
        request.expected_revision,
        request.enabled,
        Validated {
            display_name,
            server_name,
            namespace,
            connection,
            fields,
            kind,
        },
    ))
}
fn random_id(connection: &rusqlite::Connection) -> Result<String, McpError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| McpError::WriteFailed)
}
fn references(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<BTreeMap<(String, String), String>, McpError> {
    let mut statement = connection.prepare("SELECT kind, name, credential_ref FROM mcp_secret WHERE definition_id = ?1 ORDER BY kind, name").map_err(|_| McpError::ReadFailed)?;
    statement
        .query_map([id], |row| Ok(((row.get(0)?, row.get(1)?), row.get(2)?)))
        .map_err(|_| McpError::ReadFailed)?
        .collect::<Result<_, _>>()
        .map_err(|_| McpError::ReadFailed)
}
fn read_record(connection: &rusqlite::Connection, id: &str) -> Result<McpRecord, McpError> {
    let row: Option<(String,String,bool,i64,String,i64,i64)> = connection.query_row(
        "SELECT display_name,server_name,enabled,revision,connection,created_at,updated_at FROM mcp_definition WHERE id=?1", [id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(|_| McpError::ReadFailed)?;
    let (display_name, server_name, enabled, revision, json, created_at_ms, updated_at_ms) =
        row.ok_or(McpError::NotFound)?;
    let stored: StoredConnection =
        serde_json::from_str(&json).map_err(|_| McpError::InvalidStoredDefinition)?;
    let refs = references(connection, id)?;
    let connection_record = match &stored {
        StoredConnection::Stdio { command, args, cwd } => ConnectionRecord::Stdio {
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.clone(),
            env: refs.keys().map(|(_, name)| name.clone()).collect(),
        },
        StoredConnection::Http { url } => ConnectionRecord::Http {
            url: url.clone(),
            headers: refs.keys().map(|(_, name)| name.clone()).collect(),
        },
    };
    // Reuse input validation with no values. This validates persisted metadata
    // without reading the OS store or interpreting any stored command.
    let input_connection = match stored {
        StoredConnection::Stdio { command, args, cwd } => ConnectionInput::Stdio {
            command,
            args,
            cwd,
            env: refs
                .keys()
                .map(|(_, name)| SecretInput {
                    name: name.clone(),
                    value: None,
                })
                .collect(),
        },
        StoredConnection::Http { url } => ConnectionInput::Http {
            url,
            headers: refs
                .keys()
                .map(|(_, name)| SecretInput {
                    name: name.clone(),
                    value: None,
                })
                .collect(),
        },
    };
    let (_, _, _, valid) = validate(SaveMcpRequest {
        id: Some(id.to_string()),
        expected_revision: Some(revision),
        display_name: display_name.clone(),
        server_name: server_name.clone(),
        enabled,
        connection: input_connection,
    })
    .map_err(|_| McpError::InvalidStoredDefinition)?;
    if refs.iter().any(|((kind, name), reference)| {
        kind != valid.kind
            || field_name(name, kind).ok().as_ref() != Some(name)
            || !reference.starts_with(&format!("mcp-{id}-"))
            || reference.len() != 69
            || identity(&reference[37..]).is_err()
    }) || created_at_ms < 0
        || updated_at_ms < created_at_ms
    {
        return Err(McpError::InvalidStoredDefinition);
    }
    let cleanup_pending = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM mcp_cleanup WHERE definition_id=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| McpError::ReadFailed)?;
    Ok(McpRecord {
        id: id.to_string(),
        display_name,
        server_name,
        enabled,
        revision,
        connection: connection_record,
        created_at_ms,
        updated_at_ms,
        cleanup_pending,
    })
}

/// Read a definition without touching credentials, executing a process or networking.
///
/// # Errors
/// Returns safe errors for an invalid ID, missing row, inaccessible storage or
/// unrecognized stored metadata. Secret values are never included.
pub fn get_definition(storage: &Storage, id: &str) -> Result<McpRecord, McpError> {
    identity(id)?;
    let connection = lock(storage)?;
    read_record(&connection, id)
}
/// Read a stable ID cursor page (1–100 rows), without credential access.
///
/// # Errors
/// Rejects invalid cursors or limits and reports storage/read/metadata failures.
pub fn list_definitions(
    storage: &Storage,
    after: Option<&str>,
    limit: u32,
) -> Result<McpPage, McpError> {
    if !(1..=100).contains(&limit) {
        return Err(McpError::InvalidRequest);
    }
    if let Some(cursor) = after {
        identity(cursor)?;
    }
    let connection = lock(storage)?;
    let mut statement = connection
        .prepare("SELECT id FROM mcp_definition WHERE (?1 IS NULL OR id > ?1) ORDER BY id LIMIT ?2")
        .map_err(|_| McpError::ReadFailed)?;
    let ids: Vec<String> = statement
        .query_map(params![after, limit + 1], |r| r.get(0))
        .map_err(|_| McpError::ReadFailed)?
        .collect::<Result<_, _>>()
        .map_err(|_| McpError::ReadFailed)?;
    let next_cursor = if ids.len() > limit as usize {
        ids.get(limit as usize - 1).cloned()
    } else {
        None
    };
    let items = ids
        .iter()
        .take(limit as usize)
        .map(|id| read_record(&connection, id))
        .collect::<Result<_, _>>()?;
    Ok(McpPage { items, next_cursor })
}

/// Delete only recorded obsolete references, retryably. Current references are
/// checked before every deletion. Call under the app's credential mutation lock.
/// Returns whether all recorded deletions for this definition finished.
///
/// # Errors
/// Reports invalid identity and storage/read/write failures. Failed OS deletion
/// remains queued and yields `false`, so the caller can offer a retry.
pub fn cleanup_credentials(
    storage: &Storage,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<bool, McpError> {
    identity(id)?;
    let refs: Vec<String> = {
        let connection = lock(storage)?;
        let mut statement=connection.prepare("SELECT credential_ref FROM mcp_cleanup WHERE definition_id=?1 AND NOT EXISTS(SELECT 1 FROM mcp_secret WHERE mcp_secret.credential_ref=mcp_cleanup.credential_ref)").map_err(|_|McpError::ReadFailed)?;
        statement
            .query_map([id], |r| r.get(0))
            .map_err(|_| McpError::ReadFailed)?
            .collect::<Result<_, _>>()
            .map_err(|_| McpError::ReadFailed)?
    };
    for reference in refs {
        // Only references from this module's private namespace may reach the store.
        if !reference.starts_with(&format!("mcp-{id}-"))
            || reference.len() != 69
            || identity(&reference[37..]).is_err()
        {
            return Err(McpError::InvalidStoredDefinition);
        }
        if store.delete(&reference).is_ok() {
            lock(storage)?
                .execute(
                    "DELETE FROM mcp_cleanup WHERE credential_ref=?1",
                    [reference],
                )
                .map_err(|_| McpError::WriteFailed)?;
        }
    }
    let connection = lock(storage)?;
    connection
        .query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM mcp_cleanup WHERE definition_id=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| McpError::ReadFailed)
}

/// Save metadata and literal values, never starting the described server.
///
/// New values use fresh references. A known failed DB transaction deletes only
/// those new entries; uncertain commits retain them so a saved row stays usable.
/// Old entries are queued for retryable cleanup in the same metadata transaction.
/// A process crash between OS and DB writes may leave an orphan; no cross-store
/// transaction is claimed. Call under the app's credential mutation lock.
/// `now_ms` is a nonnegative Unix-millisecond timestamp used for safe metadata.
///
/// # Errors
/// Rejects unsupported or invalid fields before touching credentials. Reports
/// conflicts, unavailable credential storage and known database failures safely.
/// `OutcomeUnknown` requires reloading before another mutation.
pub fn save_definition(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: SaveMcpRequest,
    now_ms: i64,
) -> Result<McpRecord, McpError> {
    let (existing_id, revision, enabled, valid) = validate(request)?;
    if now_ms < 0 {
        return Err(McpError::InvalidRequest);
    }
    let (id, old_refs, new_ids) = {
        let connection = lock(storage)?;
        let id = match existing_id.as_ref() {
            Some(id) => id.clone(),
            None => random_id(&connection)?,
        };
        if let Some(expected) = revision
            && read_record(&connection, &id)?.revision != expected
        {
            return Err(McpError::RevisionConflict);
        }
        let old_refs = references(&connection, &id)?;
        let ids = valid
            .fields
            .iter()
            .map(|field| {
                if field.value.is_some() {
                    random_id(&connection).map(Some)
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        (id, old_refs, ids)
    };
    let mut next_refs = BTreeMap::new();
    let mut writes = Vec::new();
    for (field, new_id) in valid.fields.iter().zip(new_ids) {
        let key = (valid.kind.to_string(), field.name.clone());
        let reference = if let Some(random) = new_id {
            format!("mcp-{id}-{random}")
        } else {
            old_refs
                .get(&key)
                .cloned()
                .ok_or(McpError::SecretRequired)?
        };
        if let Some(value) = &field.value {
            writes.push((reference.clone(), value));
        }
        next_refs.insert(key, reference);
    }
    let mut written = Vec::new();
    for (reference, value) in writes {
        if let Err(error) = store.save(&reference, value) {
            // Backend initialization/access failures cannot have created this
            // fresh entry. Other platform failures may have an uncertain effect,
            // so include their reference in compensation as well.
            if !matches!(
                error,
                CredentialError::Unavailable | CredentialError::AccessDenied
            ) {
                written.push(reference);
            }
            return Err(if compensate(store, &written) {
                error.into()
            } else {
                McpError::OutcomeUnknown
            });
        }
        written.push(reference);
    }
    let commit = commit_definition(storage, &id, revision, enabled, &valid, &next_refs, now_ms);
    match commit {
        Ok(()) => {
            // Cleanup failure does not negate a confirmed metadata save. The UI
            // receives cleanupPending and a dedicated retry action.
            let _ = cleanup_credentials(storage, store, &id);
            get_definition(storage, &id).map_err(|_| McpError::OutcomeUnknown)
        }
        Err((error, uncertain)) => Err(if uncertain || !compensate(store, &written) {
            McpError::OutcomeUnknown
        } else {
            error
        }),
    }
}
fn compensate(store: &dyn CredentialStore, refs: &[String]) -> bool {
    let mut clean = true;
    for reference in refs {
        if store.delete(reference).is_err() {
            clean = false;
        }
    }
    clean
}
fn commit_definition(
    storage: &Storage,
    id: &str,
    expected: Option<i64>,
    enabled: bool,
    valid: &Validated,
    next: &BTreeMap<(String, String), String>,
    now: i64,
) -> Result<(), (McpError, bool)> {
    let operation = || -> Result<(), (McpError, bool)> {
        let mut connection = lock(storage).map_err(|e| (e, false))?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| (McpError::WriteFailed, false))?;
        let clash: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM mcp_definition WHERE namespace=?1 AND id!=?2)",
                params![valid.namespace, id],
                |r| r.get(0),
            )
            .map_err(|_| (McpError::WriteFailed, false))?;
        if clash {
            return Err((McpError::ServerNameTaken, false));
        }
        let json =
            serde_json::to_string(&valid.connection).map_err(|_| (McpError::WriteFailed, false))?;
        if let Some(revision) = expected {
            let changed = tx
                .execute(
                    "UPDATE mcp_definition
                     SET display_name=?1, server_name=?2, namespace=?3,
                         enabled=?4, connection=?5, revision=revision+1,
                         updated_at=max(updated_at,?6)
                     WHERE id=?7 AND revision=?8",
                    params![
                        valid.display_name,
                        valid.server_name,
                        valid.namespace,
                        enabled,
                        json,
                        now,
                        id,
                        revision
                    ],
                )
                .map_err(|_| (McpError::WriteFailed, false))?;
            if changed != 1 {
                return Err((McpError::RevisionConflict, false));
            }
        } else {
            tx.execute(
                "INSERT INTO mcp_definition
                 (id, display_name, server_name, namespace, enabled, revision,
                  connection, created_at, updated_at)
                 VALUES(?1,?2,?3,?4,?5,1,?6,?7,?7)",
                params![
                    id,
                    valid.display_name,
                    valid.server_name,
                    valid.namespace,
                    enabled,
                    json,
                    now
                ],
            )
            .map_err(|_| (McpError::WriteFailed, false))?;
        }
        let old = references(&tx, id).map_err(|error| (error, false))?;
        for (key, reference) in &old {
            if next.get(key) != Some(reference) {
                tx.execute(
                    "INSERT OR IGNORE INTO mcp_cleanup(definition_id,credential_ref) VALUES(?1,?2)",
                    params![id, reference],
                )
                .map_err(|_| (McpError::WriteFailed, false))?;
            }
        }
        tx.execute("DELETE FROM mcp_secret WHERE definition_id=?1", [id])
            .map_err(|_| (McpError::WriteFailed, false))?;
        for ((kind, name), reference) in next {
            tx.execute(
                "INSERT INTO mcp_secret(definition_id,kind,name,credential_ref)
                 VALUES(?1,?2,?3,?4)",
                params![id, kind, name, reference],
            )
            .map_err(|_| (McpError::WriteFailed, false))?;
        }
        tx.commit().map_err(|_| (McpError::OutcomeUnknown, true))
    };
    operation()
}

#[cfg(test)]
mod tests;
