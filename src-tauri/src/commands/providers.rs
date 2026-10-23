//! Provider instance and credential IPC commands; domain behavior stays in Rust services.

use super::state::{with_credential_writes, with_provider_storage};
use crate::credentials::{OsCredentialStore, Secret};
use crate::providers::{
    self, CreateProviderRequest, ListProvidersRequest, ProviderError, ProviderPage, ProviderRecord,
    ProviderSecretStatus, ProviderTemplate, ReplaceProviderSecretRequest, UpdateProviderRequest,
};

/// Return the built-in provider templates (default URL, allowed protocols, and
/// extension keys). Reads constants only, so it cannot fail and touches no storage.
#[tauri::command]
pub(crate) fn list_provider_templates() -> Vec<ProviderTemplate> {
    providers::provider_templates().to_vec()
}

/// Read one bounded page of provider instances in stable order.
/// Only safe provider error codes cross the boundary.
#[tauri::command]
pub(crate) async fn list_providers(
    app: tauri::AppHandle,
    request: ListProvidersRequest,
) -> Result<ProviderPage, ProviderError> {
    // Same reasoning as the preference commands: SQLite may wait for a lock, so
    // the work runs on the blocking pool and owns `app` until it finishes.
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| providers::list_providers(storage, &request))
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("list_providers", result)
}

/// Read one provider instance by its stable ID.
#[tauri::command]
pub(crate) async fn get_provider(
    app: tauri::AppHandle,
    id: String,
) -> Result<ProviderRecord, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| providers::get_provider(storage, &id))
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("get_provider", result)
}

/// Validate and save a new provider instance and its API key, returning the
/// instance after the commit. The key goes to the OS credential store only.
/// A task failure has an unknown outcome: the caller must reload the list before
/// retrying, or it may create a second instance.
#[tauri::command]
pub(crate) async fn create_provider(
    app: tauri::AppHandle,
    request: CreateProviderRequest,
) -> Result<ProviderRecord, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            with_credential_writes(&app, || {
                let now_ms = providers::current_unix_millis()?;
                providers::create_provider(storage, &OsCredentialStore, request, now_ms)
            })
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("create_provider", result)
}

/// Save provider settings and an optional replacement key under one credential lock.
/// Missing `secret` preserves the key without OS credential access. Only safe codes
/// cross IPC; uncertain outcomes require reload before retrying.
#[tauri::command]
pub(crate) async fn update_provider(
    app: tauri::AppHandle,
    request: UpdateProviderRequest,
    secret: Option<Secret>,
) -> Result<ProviderRecord, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            with_credential_writes(&app, || {
                let now_ms = providers::current_unix_millis()?;
                providers::update_provider_with_secret(
                    storage,
                    &OsCredentialStore,
                    &request,
                    secret,
                    now_ms,
                )
            })
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("update_provider", result)
}

/// Read whether a provider has an API key. Reads SQLite only: this never touches
/// the OS credential store, so it cannot trigger a system prompt.
#[tauri::command]
pub(crate) async fn get_provider_secret_status(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ProviderSecretStatus, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            providers::get_secret_status(storage, &provider_id)
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("get_provider_secret_status", result)
}

/// Remove one provider instance, its saved models and its API key.
/// The credential deletion and the database commit are ordered so a definite
/// failure restores the key; `delete_outcome_unknown` means the caller must reload
/// the list before claiming anything. No Agent file is touched.
#[tauri::command]
pub(crate) async fn delete_provider(
    app: tauri::AppHandle,
    request: providers::DeleteProviderRequest,
) -> Result<providers::DeletedProvider, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            with_credential_writes(&app, || {
                providers::delete_provider(storage, &OsCredentialStore, request)
            })
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("delete_provider", result)
}

/// List the Agent bindings that block deleting one Provider.
#[tauri::command]
pub(crate) async fn provider_deletion_impact(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<providers::ProviderDeletionImpact, ProviderError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            providers::provider_deletion_impact(storage, &provider_id)
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)?
}

/// Remove one stale Pi Provider binding or Grok alias binding.
#[tauri::command]
pub(crate) async fn clear_provider_reference(
    app: tauri::AppHandle,
    request: providers::ClearProviderReferenceRequest,
) -> Result<(), ProviderError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            providers::clear_provider_reference(storage, request)
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)?
}

/// Replace a provider's API key (or set a missing one) and return the new status.
/// The key goes to the OS credential store only. A task failure has an unknown
/// outcome: the caller must re-read the status before acting.
#[tauri::command]
pub(crate) async fn replace_provider_secret(
    app: tauri::AppHandle,
    request: ReplaceProviderSecretRequest,
) -> Result<ProviderSecretStatus, ProviderError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        with_provider_storage(&app, |storage| {
            with_credential_writes(&app, || {
                let now_ms = providers::current_unix_millis()?;
                providers::replace_provider_secret(storage, &OsCredentialStore, request, now_ms)
            })
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("replace_provider_secret", result)
}
