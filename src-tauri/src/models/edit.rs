//! Single-model snapshots and optimistic model edits (alias and capability
//! declarations). No network or credential access.
//!
//! A content digest detects changes even when two writes share a timestamp. The
//! transaction reads and compares the current snapshot before touching the row,
//! so an editor cannot silently overwrite a newer model description.

use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::repository::{lock, read_model, read_provider, stored_overrides};
use super::{ModelError, ProviderModel, parse_provider_id};
use crate::model_catalog::CapabilityOverrides;
use crate::model_identity::is_valid_model_id;
use crate::providers::validate_display_name;
use crate::storage::Storage;

/// Identity of one saved model. Both fetched and manually added rows are supported.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetModelSnapshotRequest {
    /// Provider instance owning the model.
    pub provider_id: String,
    /// Exact upstream model ID, not its display alias.
    pub model_id: String,
}

/// Saved model and an opaque version token for optimistic edits.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshot {
    /// Validated model metadata, with no credentials.
    pub model: ProviderModel,
    /// SHA-256 of the serialized record. This is a change token, not authentication.
    pub content_digest: String,
}

/// Apply the model editor's fields at once: alias and capability declarations.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateModelEditRequest {
    /// Provider instance owning the model.
    pub provider_id: String,
    /// Exact model ID; renaming the model identity is not supported.
    pub model_id: String,
    /// Digest returned when the editor read the model.
    pub expected_digest: String,
    /// Display label; `None` clears it. Nonempty values use provider name validation.
    pub alias: Option<String>,
    /// The complete set of user capability declarations. A `None` field follows
    /// the catalog evidence; an all-`None` set is "restore default".
    pub capability_overrides: CapabilityOverrides,
}

/// Whether a change token has the shape [`snapshot`] produces: 64 lowercase hex.
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn snapshot(model: ProviderModel) -> Result<ModelSnapshot, ModelError> {
    // Struct serialization has a fixed field order. Including the entire record
    // automatically brings future metadata fields into stale-edit detection.
    let bytes = serde_json::to_vec(&model).map_err(|_| ModelError::InvalidStoredModel)?;
    let content_digest = format!("{:x}", Sha256::digest(bytes));
    Ok(ModelSnapshot {
        model,
        content_digest,
    })
}

/// Read one saved model and its content digest without side effects.
///
/// # Errors
/// Returns `InvalidRequest` for malformed identities, `NotFound` for a missing
/// provider, `ModelNotFound` for a missing model, or a storage/validation error.
pub fn get_model_snapshot(
    storage: &Storage,
    request: &GetModelSnapshotRequest,
) -> Result<ModelSnapshot, ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    if !is_valid_model_id(&request.model_id) {
        return Err(ModelError::InvalidRequest);
    }
    let mut connection = lock(storage)?;
    // A read transaction keeps provider-derived routing and model fields in one
    // SQLite snapshot even if a second application connection writes meanwhile.
    let transaction = connection
        .transaction()
        .map_err(|_| ModelError::ReadFailed)?;
    let provider = read_provider(&transaction, &provider_id)?;
    let model = read_model(&transaction, &provider_id, &request.model_id)?
        .ok_or(ModelError::ModelNotFound)?
        .into_model(provider)?;
    snapshot(model)
}

/// Persist the model editor's fields (alias and capability declarations) at once,
/// preserving all other model metadata.
///
/// The immediate transaction serializes comparison and writing against other
/// connections. Unchanged values are a no-op with the same returned digest. An
/// all-empty declaration set stores no declarations, so "restore default" really
/// returns the model to the catalog evidence or unknown.
///
/// # Errors
/// Returns `InvalidRequest` for bad identity/digest/time, `ModelAliasInvalid`
/// for an invalid label, `ModelChanged` for stale content, missing-row errors,
/// or storage errors. No write is committed on these errors.
pub fn update_model_edit(
    storage: &Storage,
    request: &UpdateModelEditRequest,
    now_ms: i64,
) -> Result<ModelSnapshot, ModelError> {
    let provider_id = parse_provider_id(&request.provider_id)?;
    if !is_valid_model_id(&request.model_id) || now_ms < 0 || !is_digest(&request.expected_digest) {
        return Err(ModelError::InvalidRequest);
    }
    let alias = request
        .alias
        .as_deref()
        .map(|value| validate_display_name(value).map_err(|_| ModelError::ModelAliasInvalid))
        .transpose()?;
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;
    let provider = read_provider(&transaction, &provider_id)?;
    let current = snapshot(
        read_model(&transaction, &provider_id, &request.model_id)?
            .ok_or(ModelError::ModelNotFound)?
            .into_model(provider)?,
    )?;
    if current.content_digest != request.expected_digest {
        return Err(ModelError::ModelChanged);
    }
    if current.model.alias == alias
        && current.model.capability_overrides == request.capability_overrides
    {
        return Ok(current);
    }
    // An empty declaration set is stored as NULL, the same representation fresh
    // and migrated rows use, so "no declarations" has one stored form.
    let overrides = stored_overrides(&request.capability_overrides)?;
    transaction
        .execute(
            "UPDATE provider_model SET alias = ?3, capability_overrides = ?4,
             updated_at = max(updated_at, ?5)
             WHERE provider_id = ?1 AND model_id = ?2",
            params![
                provider_id.as_str(),
                request.model_id,
                alias,
                overrides,
                now_ms
            ],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    let updated = snapshot(
        read_model(&transaction, &provider_id, &request.model_id)?
            .ok_or(ModelError::ModelNotFound)?
            .into_model(provider)?,
    )?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(updated)
}
