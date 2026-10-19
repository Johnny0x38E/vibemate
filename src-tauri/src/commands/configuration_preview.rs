//! Read-only model-preview presentation IPC; native discovery owns every path.

use super::agents::{environment, model_failure, provider_failure};
use super::state::with_plan_storage;
use crate::configuration_preview::{PreviewError, PreviewFile, PreviewRequest};

/// Rebuild a reviewed native model plan and return sanitized presentation only.
/// No OS credential access, writes, Agent execution or frontend-selected paths.
#[tauri::command]
pub(crate) async fn read_model_configuration_preview(
    app: tauri::AppHandle,
    input: PreviewRequest,
) -> Result<Vec<PreviewFile>, PreviewError> {
    input.validate()?;
    let environment = environment(&app).map_err(|()| PreviewError::OperationFailed)?;
    tauri::async_runtime::spawn_blocking(move || {
        with_plan_storage(&app, |storage| {
            let provider = crate::providers::get_provider(storage, &input.provider_id)
                .map_err(provider_failure)?;
            let model = crate::models::get_model_snapshot(
                storage,
                &crate::models::GetModelSnapshotRequest {
                    provider_id: input.provider_id.clone(),
                    model_id: input.model_id.clone(),
                },
            )
            .map_err(model_failure)?;
            // The storage helper uses PlanError; preserve presentation errors in
            // the inner result rather than collapsing a stale plan to a generic failure.
            Ok(crate::configuration_preview::read_preview(
                storage,
                &environment,
                &provider,
                &model,
                &input,
            ))
        })
        .map_err(|error| match error {
            crate::config_plan::PlanError::ProviderNotFound
            | crate::config_plan::PlanError::ModelNotFound => PreviewError::PlanStale,
            _ => PreviewError::OperationFailed,
        })?
    })
    .await
    .map_err(|_| PreviewError::OperationFailed)?
}
