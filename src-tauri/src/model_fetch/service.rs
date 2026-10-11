//! Fetch orchestration across registration, snapshot/credentials, HTTP and merge.
//! Prepared credentials are dropped before the merge; the registration survives it.

use super::download::{ModelFetcher, download_catalog};
use super::merge::merge_catalog;
use super::registry::ModelFetchRegistry;
use super::repository;
use super::types::{ModelFetchSummary, PreparedFetch};
use crate::credentials::CredentialStore;
use crate::models::{ModelError, parse_provider_id};
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

/// Run a whole fetch: register, snapshot and key, download, merge.
///
/// The snapshot and merge steps block on SQLite and the credential store, so call
/// this only where blocking is allowed. The Tauri command runs those steps with
/// `spawn_blocking` instead.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `ModelFetchInProgress`, and every error of
/// `prepare_fetch`, `download_catalog` and `merge_catalog`.
pub async fn fetch_provider_models(
    storage: &Storage,
    store: &dyn CredentialStore,
    fetcher: &ModelFetcher,
    registry: &ModelFetchRegistry,
    provider_id: &str,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let (registration, cancel) = registry.begin(&provider_id)?;
    let prepared = prepare_fetch(storage, store, &provider_id)?;
    let catalog = download_catalog(fetcher, &prepared, cancel).await?;
    // The key is no longer needed; drop it before touching the database again.
    let PreparedFetch { snapshot, key } = prepared;
    drop(key);
    registration.enter_merge();
    merge_catalog(storage, &snapshot, &catalog, now_ms)
}
