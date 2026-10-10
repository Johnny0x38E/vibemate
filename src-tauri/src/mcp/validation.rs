//! Validate MCP metadata without executing servers or reading credentials.

use super::types::*;
use crate::shared::{bounded_text, validate_display_name};
use std::collections::BTreeSet;

pub(super) fn identity(value: &str) -> Result<McpId, McpError> {
    McpId::parse(value).ok_or(McpError::InvalidRequest)
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

pub(super) fn normalize_url(value: &str) -> Result<String, McpError> {
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

pub(super) fn field_name(name: &str, kind: &str) -> Result<String, McpError> {
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

pub(super) fn validate(
    request: SaveMcpRequest,
) -> Result<(Option<String>, Option<i64>, bool, Validated), McpError> {
    let id = request
        .id
        .as_deref()
        .map(identity)
        .transpose()?
        .map(McpId::into_string);
    if request.id.is_some() != request.expected_revision.is_some()
        || request
            .expected_revision
            .is_some_and(|r| r < 1 || r == i64::MAX)
    {
        return Err(McpError::InvalidRequest);
    }
    let display_name = validate_display_name(&request.display_name, 64)
        .map_err(|_| McpError::DisplayNameInvalid)?;
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
        id,
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
