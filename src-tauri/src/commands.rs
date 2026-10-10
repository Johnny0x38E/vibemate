//! Desktop IPC composition. Each feature owns its command module; managed-state
//! coordination is shared here, while domain behavior remains independent of Tauri.

pub(crate) mod app;
mod state;

pub(crate) use state::CredentialLock;

use crate::appearance::{self, AppearancePreference};
use crate::credentials::{OsCredentialStore, Secret};
use crate::model_fetch::{
    self, ModelFetchRegistry, ModelFetchStatus, ModelFetchSummary, ModelFetcher,
};
use crate::models::{
    self, AddManualModelRequest, DeleteManualModelRequest, ListProviderModelsRequest, ModelError,
    ProviderModel, ProviderModelPage, SetModelsSelectedRequest,
};
use crate::provider_secrets::{self, ProviderSecretStatus, ReplaceProviderSecretRequest};
use crate::providers::{
    self, CreateProviderRequest, ListProvidersRequest, ProviderError, ProviderPage, ProviderRecord,
    ProviderTemplate, UpdateProviderRequest,
};
use crate::settings::{self, LocalePreference, SettingsError};
use crate::storage::StorageStatus;
use serde::Serialize;
use state::{
    model_timestamp, with_credential_writes, with_mcp_storage, with_model_storage,
    with_provider_storage,
};
use std::sync::PoisonError;
use tauri::Manager;

/// Read the saved language choice without writing defaults. Returns safe error
/// codes if startup storage or the read fails; never exposes SQLite diagnostics.
#[tauri::command]
pub(crate) async fn get_locale_preference(
    app: tauri::AppHandle,
) -> Result<LocalePreference, SettingsError> {
    // SQLite can wait for another process's lock. Keep that blocking wait off both
    // the window thread and async executor; the owned handle lives until it ends.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::load_locale_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_locale_preference", result)
}

/// Save a validated language choice, returning it after the single-row commit.
/// Storage/write failures are safe codes; a task failure has an unknown outcome
/// and callers should reload before claiming which preference is persisted.
#[tauri::command]
pub(crate) async fn save_locale_preference(
    app: tauri::AppHandle,
    preference: LocalePreference,
) -> Result<LocalePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::save_locale_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_locale_preference", result)
}

/// Read the validated appearance/theme pair without writing a default row.
/// Database work uses the blocking pool; only safe settings error codes escape.
#[tauri::command]
pub(crate) async fn get_appearance_preference(
    app: tauri::AppHandle,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::load_appearance_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_appearance_preference", result)
}

/// Save a typed pair atomically, returning the committed choice. A task failure
/// has an unknown outcome and requires a read before another save is attempted.
#[tauri::command]
pub(crate) async fn save_appearance_preference(
    app: tauri::AppHandle,
    preference: AppearancePreference,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::save_appearance_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_appearance_preference", result)
}

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
            provider_secrets::get_secret_status(storage, &provider_id)
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("get_provider_secret_status", result)
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
                provider_secrets::replace_provider_secret(
                    storage,
                    &OsCredentialStore,
                    request,
                    now_ms,
                )
            })
        })
    })
    .await
    .map_err(|_| ProviderError::OperationFailed)
    .and_then(|result| result);
    crate::logging::provider_outcome("replace_provider_secret", result)
}

/// Fetch models only on explicit invocation. Blocking storage and credential
/// access run on the blocking pool; network I/O remains cancellable and async.
/// Returns safe model error codes, never credential or network diagnostics.
#[tauri::command]
pub(crate) async fn fetch_provider_models(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ModelFetchSummary, ModelError> {
    let id = models::parse_provider_id(&provider_id)?;
    let registry = app
        .try_state::<ModelFetchRegistry>()
        .ok_or(ModelError::OperationFailed)?;
    // Keep the registration alive through the merge, including error paths.
    // Its Drop implementation releases the provider for the next invocation.
    let (registration, cancel) = registry.begin(&id)?;
    let prepare_app = app.clone();
    let (prepared, fetcher) = tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&prepare_app, |storage| {
            let lock = prepare_app
                .try_state::<CredentialLock>()
                .ok_or(ModelError::OperationFailed)?;
            // Do not read a key between a replacement and its compensation.
            let _credential_access = lock.0.lock().unwrap_or_else(PoisonError::into_inner);
            let prepared = model_fetch::prepare_fetch(storage, &OsCredentialStore, &id)?;
            Ok((prepared, ModelFetcher::production()?))
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)??;
    let catalog = model_fetch::download_catalog(&fetcher, &prepared, cancel).await?;
    let snapshot = prepared.snapshot.clone();
    // Release the secret before moving the non-secret snapshot to the merge task.
    drop(prepared);
    registration.enter_merge();
    tauri::async_runtime::spawn_blocking(move || {
        // The task owns the registration: even if its caller stops waiting, a
        // second fetch cannot begin while this database transaction is running.
        let _registration = registration;
        with_model_storage(&app, |storage| {
            model_fetch::merge_catalog(storage, &snapshot, &catalog, model_timestamp()?)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Whether cancellation reached a fetch before merging began.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelModelFetchResult {
    was_running: bool,
}

/// Signal cancellation without waiting for SQLite or an OS credential prompt.
/// Invalid IDs and missing runtime state return safe model error codes.
#[tauri::command]
pub(crate) fn cancel_provider_model_fetch(
    registry: tauri::State<'_, ModelFetchRegistry>,
    provider_id: String,
) -> Result<CancelModelFetchResult, ModelError> {
    let id = models::parse_provider_id(&provider_id)?;
    Ok(CancelModelFetchResult {
        was_running: registry.cancel(&id),
    })
}

/// Read running state and the last persisted fetch without network access.
#[tauri::command]
pub(crate) async fn get_provider_model_fetch_status(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ModelFetchStatus, ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = app
            .try_state::<ModelFetchRegistry>()
            .ok_or(ModelError::OperationFailed)?;
        with_model_storage(&app, |storage| {
            model_fetch::fetch_status(storage, &registry, &provider_id)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Read a bounded model page or fuzzy search result. Returns safe storage codes.
#[tauri::command]
pub(crate) async fn list_provider_models(
    app: tauri::AppHandle,
    request: ListProviderModelsRequest,
) -> Result<ProviderModelPage, ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&app, |storage| {
            models::list_provider_models(storage, &request)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Download one upstream models page without merging into SQLite (P12.c.5).
#[tauri::command]
pub(crate) async fn browse_upstream_models_page(
    app: tauri::AppHandle,
    request: model_fetch::BrowseUpstreamModelsRequest,
) -> Result<model_fetch::UpstreamBrowsePage, ModelError> {
    let id = models::parse_provider_id(&request.provider_id)?;
    let registry = app
        .try_state::<ModelFetchRegistry>()
        .ok_or(ModelError::OperationFailed)?
        .inner()
        .clone();
    let (registration, cancel) = registry.begin(&id)?;
    let prepare_app = app.clone();
    let offset = request.offset;
    let (prepared, fetcher) = tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&prepare_app, |storage| {
            let lock = prepare_app
                .try_state::<CredentialLock>()
                .ok_or(ModelError::OperationFailed)?;
            let _credential_access = lock.0.lock().unwrap_or_else(PoisonError::into_inner);
            let prepared = model_fetch::prepare_fetch(storage, &OsCredentialStore, &id)?;
            Ok((prepared, ModelFetcher::production()?))
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)??;
    let query = match request.query.as_deref() {
        Some(raw) => {
            crate::model_search::SearchQuery::parse(raw).map_err(|_| ModelError::InvalidRequest)?
        }
        None => None,
    };
    let page = model_fetch::browse_upstream_page(
        &fetcher,
        &registry,
        &prepared,
        cancel,
        offset,
        query.as_ref(),
    )
    .await?;
    drop(prepared);
    drop(registration);
    Ok(page)
}

/// Save checked models and delete unchecked ones in one transaction (P12.c.5).
#[tauri::command]
pub(crate) async fn save_provider_model_selections(
    app: tauri::AppHandle,
    request: models::SaveProviderModelSelectionsRequest,
) -> Result<(), ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&app, |storage| {
            models::save_provider_model_selections(storage, &request, model_timestamp()?)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Persist a selection batch atomically; unsupported routes return a safe code.
#[tauri::command]
pub(crate) async fn set_provider_models_selected(
    app: tauri::AppHandle,
    request: SetModelsSelectedRequest,
) -> Result<Vec<ProviderModel>, ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&app, |storage| {
            models::set_models_selected(storage, &request, model_timestamp()?)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Validate and persist a selected manual model. No network or credential access.
#[tauri::command]
pub(crate) async fn add_manual_provider_model(
    app: tauri::AppHandle,
    request: AddManualModelRequest,
) -> Result<ProviderModel, ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&app, |storage| {
            models::add_manual_model(storage, &request, model_timestamp()?)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Delete only a manual model; fetched rows are rejected by the domain layer.
#[tauri::command]
pub(crate) async fn delete_manual_provider_model(
    app: tauri::AppHandle,
    request: DeleteManualModelRequest,
) -> Result<(), ModelError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_model_storage(&app, |storage| {
            models::delete_manual_model(storage, &request)
        })
    })
    .await
    .map_err(|_| ModelError::OperationFailed)?
}

/// Read central MCP metadata, without executing servers or opening credentials.
#[tauri::command]
pub(crate) async fn list_mcp_definitions(
    app: tauri::AppHandle,
    after: Option<String>,
    limit: u32,
) -> Result<crate::mcp::McpPage, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            crate::mcp::list_definitions(storage, after.as_deref(), limit)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Read one non-secret central definition for editing or result reconciliation.
#[tauri::command]
pub(crate) async fn get_mcp_definition(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| crate::mcp::get_definition(storage, &id))
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Save only central metadata and OS credential references. No process or HTTP
/// API is involved; the credential lock excludes in-process secret mutations.
#[tauri::command]
pub(crate) async fn save_mcp_definition(
    app: tauri::AppHandle,
    request: crate::mcp::SaveMcpRequest,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            let credential_lock = app
                .try_state::<CredentialLock>()
                .ok_or(crate::mcp::McpError::OperationFailed)?;
            let _guard = credential_lock
                .0
                .lock()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            let now = providers::current_unix_millis()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            crate::mcp::save_definition(storage, &OsCredentialStore, request, now)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Retry deletion of recorded obsolete credential entries and return safe status.
#[tauri::command]
pub(crate) async fn cleanup_mcp_credentials(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            let credential_lock = app
                .try_state::<CredentialLock>()
                .ok_or(crate::mcp::McpError::OperationFailed)?;
            let _guard = credential_lock
                .0
                .lock()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            crate::mcp::get_definition(storage, &id)?;
            crate::mcp::cleanup_credentials(storage, &OsCredentialStore, &id)?;
            crate::mcp::get_definition(storage, &id)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}
