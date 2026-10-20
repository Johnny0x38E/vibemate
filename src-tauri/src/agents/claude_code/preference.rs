//! Reusable Claude Code effort drafts in private SQLite, not Agent files.
//!
//! A preference belongs to one Provider instance/model pair. It does not change
//! model capabilities, credential storage, configuration approval or history.
//! Snapshot reads and compare-and-swap saves use one SQLite transaction each.

use super::{ModelFacts, effort, is_digest};
use crate::model_catalog::EffortLevel;
use crate::model_identity::is_valid_model_id;
use crate::models::{
    GetModelSnapshotRequest, ModelError, ModelSnapshot, parse_provider_id, read_model_snapshot,
};
use crate::providers::{ProviderId, ProviderKind};
use crate::shared::is_random_id;
use crate::storage::Storage;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

/// Safe, payload-free failures for reading and remembering a starting choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreferenceError {
    /// Startup storage failed or its mutex is poisoned.
    StorageUnavailable,
    /// SQLite could not read a complete snapshot.
    ReadFailed,
    /// SQLite could not commit; the earlier preference is unchanged.
    WriteFailed,
    /// Identity, digest or revision has an invalid shape.
    InvalidRequest,
    /// The Provider or saved model no longer exists.
    NotFound,
    /// Persisted metadata or a preference is invalid; do not substitute defaults.
    InvalidStoredData,
    /// Model metadata changed after this starting choice was loaded.
    ModelChanged,
    /// Another save changed the preference; reload before editing it again.
    PreferenceChanged,
    /// The model/adapter intersection does not include the requested effort.
    EffortNotMapped,
    /// A blocking desktop operation did not finish.
    OperationFailed,
}

impl From<ModelError> for PreferenceError {
    fn from(error: ModelError) -> Self {
        match error {
            ModelError::InvalidRequest => Self::InvalidRequest,
            ModelError::NotFound | ModelError::ModelNotFound => Self::NotFound,
            ModelError::StorageUnavailable => Self::StorageUnavailable,
            ModelError::ReadFailed => Self::ReadFailed,
            _ => Self::InvalidStoredData,
        }
    }
}

/// Current model metadata and a remembered starting choice, read consistently.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffortPreference {
    /// Includes model identity, current capabilities and their content token.
    pub snapshot: ModelSnapshot,
    /// Opaque compare-and-swap token; absent only when no preference row exists.
    pub revision: Option<String>,
    /// `None` means do not overwrite native effort, not reset the model default.
    pub selected_effort: Option<EffortLevel>,
}

/// Explicitly remember a draft, independently of previewing or applying files.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveEffortPreference {
    /// Exact owning Provider instance.
    pub provider_id: String,
    /// Exact upstream ID, not a display alias.
    pub model_id: String,
    /// The model snapshot the editor used to offer choices.
    pub expected_model_digest: String,
    /// The last read preference revision, including absence on first use.
    pub expected_revision: Option<String>,
    /// Requested starting choice; clearing retains a revision to prevent ABA races.
    pub selected_effort: Option<EffortLevel>,
}

fn identity(provider_id: &str, model_id: &str) -> Result<ProviderId, PreferenceError> {
    if !is_valid_model_id(model_id) {
        return Err(PreferenceError::InvalidRequest);
    }
    parse_provider_id(provider_id).map_err(PreferenceError::from)
}

fn read(
    connection: &Connection,
    provider_id: &ProviderId,
    model_id: &str,
) -> Result<EffortPreference, PreferenceError> {
    let snapshot = read_model_snapshot(connection, provider_id, model_id)?;
    let stored: Option<(String, Option<String>)> = connection.query_row(
        "SELECT revision, effort FROM claude_effort_preference WHERE provider_id=?1 AND model_id=?2",
        params![provider_id.as_str(), model_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|_| PreferenceError::ReadFailed)?;
    let (revision, selected_effort) = match stored {
        None => (None, None),
        Some((revision, value)) => {
            if !is_random_id(&revision) {
                return Err(PreferenceError::InvalidStoredData);
            }
            let selected = value
                .as_deref()
                .map(|value| {
                    EffortLevel::parse(value)
                        .filter(|level| effort::is_native_effort(*level))
                        .ok_or(PreferenceError::InvalidStoredData)
                })
                .transpose()?;
            (Some(revision), selected)
        }
    };
    Ok(EffortPreference {
        snapshot,
        revision,
        selected_effort,
    })
}

/// Load the model and its starting choice without writing a default.
/// Obsolete but well-formed saved efforts are returned, not silently downgraded.
/// No file, network or credential access occurs. Missing identity, read and
/// invalid-stored-data failures are returned as safe codes.
pub(crate) fn load_preference(
    storage: &Storage,
    request: &GetModelSnapshotRequest,
) -> Result<EffortPreference, PreferenceError> {
    let provider = identity(&request.provider_id, &request.model_id)?;
    let mut connection = storage
        .lock()
        .map_err(|_| PreferenceError::StorageUnavailable)?;
    let transaction = connection
        .transaction()
        .map_err(|_| PreferenceError::ReadFailed)?;
    read(&transaction, &provider, &request.model_id)
}

/// Compare both change tokens and remember one validated draft atomically.
/// An immediate transaction serializes independent app connections. A no-op
/// preserves its token; clearing an existing value keeps a new token rather
/// than deleting the row, so save-and-clear cannot look like the original absence.
///
/// No Agent file, credential or history is changed. Malformed tokens, missing
/// rows, changed metadata/preferences and unmapped efforts refuse the save;
/// SQLite failures roll back without reporting success.
pub(crate) fn save_preference(
    storage: &Storage,
    request: &SaveEffortPreference,
) -> Result<EffortPreference, PreferenceError> {
    let provider = identity(&request.provider_id, &request.model_id)?;
    if !is_digest(&request.expected_model_digest)
        || request
            .expected_revision
            .as_deref()
            .is_some_and(|value| !is_random_id(value))
    {
        return Err(PreferenceError::InvalidRequest);
    }
    let mut connection = storage
        .lock()
        .map_err(|_| PreferenceError::StorageUnavailable)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| PreferenceError::WriteFailed)?;
    let current = read(&transaction, &provider, &request.model_id)?;
    if current.snapshot.content_digest != request.expected_model_digest {
        return Err(PreferenceError::ModelChanged);
    }
    if current.revision != request.expected_revision {
        return Err(PreferenceError::PreferenceChanged);
    }
    let kind: String = transaction
        .query_row(
            "SELECT kind FROM provider_instance WHERE id=?1",
            [provider.as_str()],
            |row| row.get(0),
        )
        .map_err(|_| PreferenceError::ReadFailed)?;
    let kind = ProviderKind::from_value(&kind).ok_or(PreferenceError::InvalidStoredData)?;
    effort::validate_selection(
        kind,
        &ModelFacts::from_snapshot(&current.snapshot),
        request.selected_effort,
    )
    .map_err(|_| PreferenceError::EffortNotMapped)?;
    if current.selected_effort == request.selected_effort {
        return Ok(current);
    }
    let revision: String = transaction
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| PreferenceError::WriteFailed)?;
    transaction.execute(
        "INSERT INTO claude_effort_preference(provider_id,model_id,revision,effort) VALUES(?1,?2,?3,?4)
         ON CONFLICT(provider_id,model_id) DO UPDATE SET revision=excluded.revision,effort=excluded.effort",
        params![provider.as_str(),request.model_id,revision,request.selected_effort.map(EffortLevel::as_str)],
    ).map_err(|_| PreferenceError::WriteFailed)?;
    transaction
        .commit()
        .map_err(|_| PreferenceError::WriteFailed)?;
    Ok(EffortPreference {
        revision: Some(revision),
        selected_effort: request.selected_effort,
        ..current
    })
}

#[cfg(test)]
mod tests;
