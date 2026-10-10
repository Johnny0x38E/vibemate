//! Desktop metadata, preference, and provider commands exposed to the React interface.
//!
//! Keep this boundary thin: future configuration behavior belongs in feature
//! modules so it can be tested without starting a Tauri window.

use crate::appearance::{self, AppearancePreference};
use crate::credentials::{OsCredentialStore, Secret};
use crate::log_access::{self, LogAccess, LogAccessError, LogLocation};
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
use crate::storage::{Storage, StorageStatus};
use serde::Serialize;
use std::sync::{Mutex, PoisonError};
use tauri::Manager;

/// Serializes this app's writes to the OS credential store.
///
/// A replacement reads the old key as a backup, writes the new one, and may put
/// the old one back. Two such sequences for the same provider must not interleave.
/// This is a separate lock from the database connection: the credential store may
/// wait for a system prompt, and database reads must not wait with it.
///
/// The mutex only excludes other threads of this process. It does not stop a
/// second vibemate process from writing the same entry at the same time; the
/// app does not prevent a second instance yet.
#[derive(Default)]
pub(crate) struct CredentialLock(Mutex<()>);

/// Application metadata returned to the frontend without reading user config.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    name: &'static str,
    version: &'static str,
}

/// Return build metadata. This command has no filesystem or network side effects.
#[tauri::command]
pub(crate) fn get_app_info() -> AppInfo {
    AppInfo {
        name: "vibemate",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Return the configured log paths and startup file-logging status without I/O.
#[tauri::command]
pub(crate) fn get_log_location(
    access: tauri::State<'_, LogAccess>,
) -> Result<LogLocation, LogAccessError> {
    access.location()
}

/// Open only the fixed app log file with a platform text tool.
/// No frontend path or executable is accepted. Files are never created here.
#[tauri::command]
pub(crate) async fn open_log_file(app: tauri::AppHandle) -> Result<(), LogAccessError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let access = app
            .try_state::<LogAccess>()
            .ok_or(LogAccessError::PathUnavailable)?;
        access.open_file_with(log_access::open_in_text_tool)
    })
    .await
    .map_err(|_| LogAccessError::OperationFailed)
    .and_then(|result| result);
    if let Err(error) = result {
        log::warn!(target: "vibemate", "event=command_failed operation=open_log_file code={error:?}");
    }
    result
}

/// Open only the fixed app log directory with the system file manager.
/// Returns safe error codes without creating a missing directory.
#[tauri::command]
pub(crate) async fn open_log_directory(app: tauri::AppHandle) -> Result<(), LogAccessError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let access = app
            .try_state::<LogAccess>()
            .ok_or(LogAccessError::PathUnavailable)?;
        access.open_directory_with(log_access::open_in_file_manager)
    })
    .await
    .map_err(|_| LogAccessError::OperationFailed)
    .and_then(|result| result);
    if let Err(error) = result {
        log::warn!(target: "vibemate", "event=command_failed operation=open_log_directory code={error:?}");
    }
    result
}

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

/// Open the project's fixed GitHub repository in the system browser.
///
/// Accepts no frontend URL or executable. Returns a safe code if the operating
/// system cannot open the browser or the blocking task cannot complete.
#[tauri::command]
pub(crate) async fn open_project_repository() -> Result<(), &'static str> {
    tauri::async_runtime::spawn_blocking(|| {
        tauri_plugin_opener::open_url("https://github.com/Johnny0x38E/vibemate", None::<&str>)
            .map_err(|_| "open_failed")
    })
    .await
    .map_err(|_| "open_failed")?
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

fn model_timestamp() -> Result<i64, ModelError> {
    providers::current_unix_millis().map_err(|_| ModelError::OperationFailed)
}

/// Resolve startup storage while keeping failures in the model error namespace.
fn with_model_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, ModelError>,
) -> Result<T, ModelError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(ModelError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| ModelError::StorageUnavailable)?;
    operation(storage)
}

/// Run `operation` while holding the app-wide `CredentialLock`.
///
/// The lock guards no data, only ordering, so a panic elsewhere ("poisoning")
/// leaves nothing inconsistent to protect and the lock is used as usual.
/// It gives mutual exclusion within this process only (see `CredentialLock`).
///
/// A missing lock means the app was wired up wrongly, not that storage failed,
/// so it is reported as the generic internal `OperationFailed`, before anything
/// was written.
fn with_credential_writes<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce() -> Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    let lock = app
        .try_state::<CredentialLock>()
        .ok_or(ProviderError::OperationFailed)?;
    let _credential_writes = lock.0.lock().unwrap_or_else(PoisonError::into_inner);
    operation()
}

/// Run one provider operation against the storage opened at startup.
///
/// Every provider command needs the same two lookups, and both failures mean
/// `StorageUnavailable`. Keeping them here leaves each command one line of logic.
fn with_provider_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(ProviderError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    operation(storage)
}
