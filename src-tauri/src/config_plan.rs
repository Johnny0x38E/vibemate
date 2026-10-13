//! Agent configuration preview plans: the target file and its change token, the
//! key-level changes, and the notices a user needs before applying anything.
//!
//! Nothing in this module writes files, reads credential values or sends
//! requests. An Agent adapter builds a plan from vendor instructions that were
//! verified first; the Claude Code mapping is recorded in
//! `docs/integrations/claude-code.md` and `docs/plans/p18-claude-code-link.md`.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::ErrorKind, path::Path};

use crate::agents::AgentId;

/// Largest settings file a preview parses. Real Agent settings are far smaller;
/// the cap keeps an unexpectedly large file out of memory.
pub const MAX_TARGET_BYTES: u64 = 1024 * 1024;

/// Text shown instead of a credential value while previewing.
pub const SECRET_PLACEHOLDER: &str = "••••••••";

/// Identity and change token of the file a plan would change.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetFile {
    /// Absolute path shown to the user.
    pub path: String,
    /// Whether the file exists at plan time.
    pub exists: bool,
    /// SHA-256 of the current bytes; `None` when the file does not exist. A plan
    /// is stale once this digest changes, so the apply step must compare it again.
    pub content_digest: Option<String>,
    /// Whether the path is a symbolic link. The preview only reports this; the
    /// write step (P19) decides what to do with a link it did not create.
    pub symlink: bool,
}

/// The Provider and model revisions a plan was built from.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSource {
    /// Stored Provider instance ID.
    pub provider_id: String,
    /// Stored Provider `revision`, the same token provider edits use.
    pub provider_revision: i64,
    /// Upstream model ID, not its display alias.
    pub model_id: String,
    /// SHA-256 of the stored model record, the token alias edits already use.
    pub model_content_digest: String,
}

/// One key-level change, already sanitized for display.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChange {
    /// Dotted path inside the JSON document, for example `env.ANTHROPIC_MODEL`.
    pub key_path: String,
    /// Current value as display text; `None` when the key is absent.
    pub before: Option<String>,
    /// Planned value as display text, or the placeholder for a credential.
    pub after: String,
    /// Whether `after` hides a credential instead of carrying its value.
    pub secret: bool,
}

/// Notice kinds the current adapters produce.
///
/// Codes use the same camelCase form as the rest of the Agent catalog
/// (`AgentId::ClaudeCode` serializes as `claudeCode`), and the interface keeps
/// its own list in `src/lib/desktop/agents.ts`. Renaming a variant without
/// updating that list would reach the user as `invalid_response`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeCode {
    /// The real credential is read from OS storage when the plan is applied.
    CredentialFromStore,
    /// Higher-precedence sources were not read by this preview.
    OverridesNotChecked,
    /// An existing `model` key is overridden by the planned `env` variable.
    ModelKeyOverridden,
    /// The vendor documents a limitation for this Provider.
    ProviderLimited,
}

/// Something the user must know before applying a plan. The interface
/// translates `code`; `detail` is bounded display text and never a credential.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanNotice {
    /// Notice kind, translated by the interface.
    pub code: NoticeCode,
    /// Optional extra display text, already sanitized by the adapter.
    pub detail: Option<String>,
}

impl PlanNotice {
    /// A notice without extra text.
    pub fn code(code: NoticeCode) -> Self {
        Self { code, detail: None }
    }

    /// A notice carrying bounded display text.
    pub fn with_detail(code: NoticeCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }
}

/// Why a preview could not be built.
///
/// Refusals are expected outcomes: a combination without verified instructions
/// is reported instead of being planned from a guess.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanError {
    /// Provider revision, model ID or model digest was malformed.
    InvalidRequest,
    /// This Provider has no verified Claude Code gateway instructions.
    GatewayUnverified,
    /// The instance address is not one the vendor documents for this Provider.
    BaseUrlUnverified,
    /// The target path cannot be shown or is not absolute.
    TargetPathInvalid,
    /// The target file could not be read or is not a JSON object.
    TargetUnreadable,
    /// The target file is larger than [`MAX_TARGET_BYTES`].
    TargetTooLarge,
    /// A key the plan must change holds a value of the wrong shape.
    TargetConflict,
    /// The configuration database is not open, or reading it failed.
    StorageUnavailable,
    /// No provider instance has the requested ID.
    ProviderNotFound,
    /// The provider instance has no saved model with the requested ID.
    ModelNotFound,
    /// The user configuration directory cannot be resolved.
    LocationUnavailable,
    /// The operation could not finish.
    OperationFailed,
}

/// A parsed target document together with its file identity.
#[derive(Debug)]
pub struct TargetDocument {
    /// File identity and change token.
    pub target: TargetFile,
    /// Parsed JSON object; empty when the file is missing or blank.
    pub document: Map<String, Value>,
}

/// The value one planned write stores.
#[derive(Debug, PartialEq, Eq)]
pub enum PlannedValue {
    /// Text the preview shows and a later apply step writes.
    Literal(String),
    /// A credential read from OS storage at apply time. The preview only marks
    /// the position of the value, so a secret never travels through a plan.
    Credential,
}

/// A planned write of one value.
#[derive(Debug)]
pub struct PlannedWrite {
    /// Dotted path inside the JSON document.
    pub key_path: &'static str,
    /// What the key will hold.
    pub value: PlannedValue,
}

/// The plan the interface renders.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPlan {
    /// Agent the plan configures.
    pub agent: AgentId,
    /// File the plan would change, unchanged on disk.
    pub target: TargetFile,
    /// Provider and model revisions the plan was built from.
    pub source: PlanSource,
    /// Sanitized key-level changes.
    pub changes: Vec<ConfigChange>,
    /// What the user must know before applying.
    pub notices: Vec<PlanNotice>,
    /// Whether the Agent has to start a new session for the change to apply.
    pub restart_required: bool,
}

/// Read the JSON object a plan would change.
///
/// A missing file reads as an empty document without a digest; a blank file
/// reads as an empty document with the digest of its bytes. The digest covers
/// the exact bytes, so any concurrent edit invalidates an older plan.
///
/// # Errors
/// Returns `TargetPathInvalid` for a relative or unprintable path,
/// `TargetUnreadable` for a directory, a dangling symbolic link, an unreadable
/// file, or content that is not a JSON object, and `TargetTooLarge` above
/// [`MAX_TARGET_BYTES`].
pub fn read_target_document(path: &Path) -> Result<TargetDocument, PlanError> {
    let path_text = display_path(path).ok_or(PlanError::TargetPathInvalid)?;
    let symlink = fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        // A dangling symbolic link is a broken target, not a missing file: a
        // later write would replace the link instead of the file it names.
        Err(error) if error.kind() == ErrorKind::NotFound && !symlink => {
            return Ok(TargetDocument {
                target: TargetFile {
                    path: path_text,
                    exists: false,
                    content_digest: None,
                    symlink,
                },
                document: Map::new(),
            });
        }
        Err(_) => return Err(PlanError::TargetUnreadable),
    };
    if !metadata.is_file() {
        return Err(PlanError::TargetUnreadable);
    }
    if metadata.len() > MAX_TARGET_BYTES {
        return Err(PlanError::TargetTooLarge);
    }
    let bytes = fs::read(path).map_err(|_| PlanError::TargetUnreadable)?;
    let content_digest = digest_hex(&bytes);
    // A blank file is a valid empty settings document; whitespace-only content
    // must not be reported as a parse failure.
    let document = if bytes.iter().all(u8::is_ascii_whitespace) {
        Map::new()
    } else {
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(object)) => object,
            _ => return Err(PlanError::TargetUnreadable),
        }
    };
    Ok(TargetDocument {
        target: TargetFile {
            path: path_text,
            exists: true,
            content_digest: Some(content_digest),
            symlink,
        },
        document,
    })
}

/// Lowercase SHA-256 of the given bytes.
pub fn digest_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Apply planned writes to a parsed document and describe each change.
///
/// Missing intermediate objects are created. A nested key that already holds a
/// value of another type is a conflict instead of a silent overwrite. Keys the
/// plan does not name are left exactly as they were, which is what the preview
/// promises the user.
///
/// # Errors
/// Returns `InvalidRequest` for a key path with an empty segment and
/// `TargetConflict` when a parent key holds a non-object value.
pub fn planned_changes(
    document: &mut Map<String, Value>,
    writes: &[PlannedWrite],
) -> Result<Vec<ConfigChange>, PlanError> {
    let mut changes = Vec::with_capacity(writes.len());
    for write in writes {
        let segments: Vec<&str> = write.key_path.split('.').collect();
        if segments.iter().any(|segment| segment.is_empty()) {
            return Err(PlanError::InvalidRequest);
        }
        let (key, parents) = segments.split_last().ok_or(PlanError::InvalidRequest)?;
        let parent = parent_object(document, parents)?;
        let before = parent.get(*key).map(value_text);
        let (stored, after, secret) = match &write.value {
            PlannedValue::Literal(text) => (Value::String(text.clone()), text.clone(), false),
            PlannedValue::Credential => (
                Value::String(SECRET_PLACEHOLDER.to_owned()),
                SECRET_PLACEHOLDER.to_owned(),
                true,
            ),
        };
        parent.insert((*key).to_owned(), stored);
        changes.push(ConfigChange {
            key_path: write.key_path.to_owned(),
            before,
            after,
            secret,
        });
    }
    Ok(changes)
}

/// Walk to the object holding the last segment of a key path.
fn parent_object<'a>(
    document: &'a mut Map<String, Value>,
    parents: &[&str],
) -> Result<&'a mut Map<String, Value>, PlanError> {
    match parents.split_first() {
        None => Ok(document),
        Some((head, rest)) => {
            let entry = document
                .entry((*head).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            match entry {
                Value::Object(child) => parent_object(child, rest),
                _ => Err(PlanError::TargetConflict),
            }
        }
    }
}

/// Render a JSON value as short display text.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Absolute, printable path text matching the discovery module's rules.
fn display_path(path: &Path) -> Option<String> {
    let value = path.to_str()?;
    (path.is_absolute() && value.len() <= 4096 && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

#[cfg(test)]
mod tests;
