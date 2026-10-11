//! Snapshot and credential read for one model-list fetch.
//! The desktop command composes download and merge around this step.

use super::repository;
use super::types::PreparedFetch;
use crate::credentials::CredentialStore;
use crate::model_error::ModelError;
use crate::providers::ProviderId;
use crate::storage::Storage;

/// Read the provider snapshot, release the database, then read the key.
///
/// # Errors
/// `NotFound`, `InvalidStoredProvider`, `SecretMissing`, a credential-store code,
/// or a storage code.
pub fn prepare_fetch(
    storage: &Storage,
    store: &dyn CredentialStore,
    provider_id: &ProviderId,
) -> Result<PreparedFetch, ModelError> {
    let (snapshot, reference) = repository::read_provider_snapshot(storage, provider_id)?;
    let reference = reference.ok_or(ModelError::SecretMissing)?;
    let key = store.load(&reference)?.ok_or(ModelError::SecretMissing)?;
    Ok(PreparedFetch { snapshot, key })
}
