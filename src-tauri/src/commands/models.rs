//! Model discovery, cancellation, browse and selection IPC commands.
//! Blocking storage/credential steps and async downloads retain their original ordering.

use super::state::{CredentialLock, model_timestamp, with_model_storage};
use crate::credentials::OsCredentialStore;
use crate::model_fetch::{
    self, ModelFetchRegistry, ModelFetchStatus, ModelFetchSummary, ModelFetcher,
};
use crate::models::{
    self, AddManualModelRequest, DeleteManualModelRequest, ListProviderModelsRequest, ModelError,
    ProviderModel, ProviderModelPage, SetModelsSelectedRequest,
};
use serde::Serialize;
use std::sync::PoisonError;
use tauri::Manager;

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
