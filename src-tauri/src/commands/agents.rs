//! Thin Agent IPC: discovery, explicit version launch, Claude Code configuration,
//! and Agent-scoped review/recording of interrupted mutation observations.
//!
//! Discovery, preview and restore planning read the filesystem; the version
//! launch is the only operation that runs another program, and it is not a
//! sandbox.

use super::state::{
    with_apply_storage, with_credential_lock, with_plan_storage, with_restore_storage,
};
use crate::agents::{
    self, AgentObservation, AgentVersionResult, ApplyClaudeCodeRestoreRequest,
    ApplyClaudeCodeSettingsRequest, ClaudeCodeApplied, ClaudeCodeApplyError, ClaudeCodeApplyInput,
    ClaudeCodePlanInput, ClaudeCodeRestoreError, ClaudeCodeRestorePlan, DirectorySource,
    DiscoveryEnvironment, DiscoveryError, DiscoveryInput, ModelFacts, ProviderFacts,
    VersionProbeError, VersionProbeGate, VersionProbeInput,
};
use crate::config_apply::RestoreOutcome;
use crate::config_plan::{ConfigPlan, PlanError};
use crate::credentials::OsCredentialStore;
use crate::models::{GetModelSnapshotRequest, ModelError};
use crate::providers::ProviderError;
use tauri::Manager;

pub(super) fn environment(app: &tauri::AppHandle) -> Result<DiscoveryEnvironment, ()> {
    let home = app.path().home_dir().map_err(|_| ())?;
    Ok(DiscoveryEnvironment {
        home,
        path: std::env::var_os("PATH"),
        pi_directory: std::env::var_os("PI_CODING_AGENT_DIR"),
        grok_directory: std::env::var_os("GROK_HOME"),
        claude_directory: std::env::var_os("CLAUDE_CONFIG_DIR"),
        codex_directory: std::env::var_os("CODEX_HOME"),
        opencode_directory: std::env::var_os("OPENCODE_CONFIG_DIR"),
        xdg_config_directory: std::env::var_os("XDG_CONFIG_HOME"),
        hermes_directory: std::env::var_os("HERMES_HOME"),
        local_app_data: std::env::var_os("LOCALAPPDATA"),
    })
}

/// Observe six entries without running them or reading configuration contents.
/// Filesystem work runs off the async executor; errors contain no I/O diagnostics.
#[tauri::command]
pub(crate) async fn discover_agents(
    app: tauri::AppHandle,
    input: DiscoveryInput,
) -> Result<Vec<AgentObservation>, DiscoveryError> {
    let environment = environment(&app).map_err(|_| DiscoveryError::LocationUnavailable)?;
    tauri::async_runtime::spawn_blocking(move || agents::discover_agents(&environment, &input))
        .await
        .map_err(|_| DiscoveryError::OperationFailed)?
}

/// Run only the fixed version command after explicit UI confirmation.
/// Returns bounded version data or sanitized failure. This is not a sandbox;
/// the selected program can access user files/network or start descendants.
#[tauri::command]
pub(crate) async fn probe_agent_version(
    app: tauri::AppHandle,
    input: VersionProbeInput,
    gate: tauri::State<'_, VersionProbeGate>,
) -> Result<AgentVersionResult, VersionProbeError> {
    let environment = environment(&app).map_err(|_| VersionProbeError::OperationFailed)?;
    agents::probe_agent_version(environment, input, gate.inner()).await
}

/// Preview the Claude Code settings a Provider/model link would write.
///
/// Read-only: the user settings file is read for its change token, never
/// written, and no credential value is read. The plan binds the settings file,
/// the Provider revision and the model content digest, so an edit to any of the
/// three makes the plan stale before it could be applied.
#[tauri::command]
pub(crate) async fn plan_claude_code_settings(
    app: tauri::AppHandle,
    provider_id: String,
    model_id: String,
) -> Result<ConfigPlan, PlanError> {
    let environment = environment(&app).map_err(|()| PlanError::LocationUnavailable)?;
    let (config_root, directory_source) = agents::claude_config_root(&environment);
    if directory_source == DirectorySource::Unavailable {
        return Err(PlanError::LocationUnavailable);
    }
    let config_root = config_root.ok_or(PlanError::LocationUnavailable)?;
    // Reject a malformed identity before any storage access.
    let provider =
        crate::models::parse_provider_id(&provider_id).map_err(|_| PlanError::InvalidRequest)?;

    tauri::async_runtime::spawn_blocking(move || {
        with_plan_storage(&app, |storage| {
            // Two reads, each bound into the plan by its own change token. An
            // edit between them cannot be applied silently: the apply step
            // rechecks both the Provider revision and the model digest.
            let record = crate::providers::get_provider(storage, provider.as_str())
                .map_err(provider_failure)?;
            let snapshot = crate::models::get_model_snapshot(
                storage,
                &GetModelSnapshotRequest {
                    provider_id: provider.as_str().to_owned(),
                    model_id,
                },
            )
            .map_err(model_failure)?;
            let input = ClaudeCodePlanInput {
                config_root,
                provider: ProviderFacts {
                    id: record.id.as_str().to_owned(),
                    kind: record.kind,
                    base_url: record.base_url,
                    revision: record.revision,
                },
                model: ModelFacts {
                    id: snapshot.model.model_id,
                    content_digest: snapshot.content_digest,
                },
            };
            agents::plan_claude_code_settings(&input)
        })
    })
    .await
    .map_err(|_| PlanError::OperationFailed)?
}

/// Apply a reviewed Claude Code plan: write the user settings file with the key.
///
/// The first command that changes an Agent configuration file. It rechecks every
/// token the preview bound, reads the Provider key from OS credential storage
/// without returning it, and records the operation in the write log. The caller
/// receives the state, the per-file outcome and where the copy of the previous
/// content sits. File-level refusals are results; an error means the write could
/// not be recorded.
#[tauri::command]
pub(crate) async fn apply_claude_code_settings(
    app: tauri::AppHandle,
    input: ApplyClaudeCodeSettingsRequest,
) -> Result<ClaudeCodeApplied, ClaudeCodeApplyError> {
    let environment = environment(&app).map_err(|()| ClaudeCodeApplyError::LocationUnavailable)?;
    let (config_root, directory_source) = agents::claude_config_root(&environment);
    if directory_source == DirectorySource::Unavailable {
        return Err(ClaudeCodeApplyError::LocationUnavailable);
    }
    let config_root = config_root.ok_or(ClaudeCodeApplyError::LocationUnavailable)?;
    // Reject a malformed identity before any storage or credential access.
    let provider = crate::models::parse_provider_id(&input.provider_id)
        .map_err(|_| ClaudeCodeApplyError::InvalidRequest)?;
    // Replaced settings content is kept next to the database, never inside the
    // settings folder it was read from.
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeApplyError::LocationUnavailable)?
        .join("backups");

    tauri::async_runtime::spawn_blocking(move || {
        // The credential lock is taken before the key is read, so a replacement
        // started by another command cannot be observed halfway through.
        with_credential_lock(&app, ClaudeCodeApplyError::OperationFailed, || {
            with_apply_storage(&app, |storage| {
                let record = crate::providers::get_provider(storage, provider.as_str())
                    .map_err(apply_provider_failure)?;
                let snapshot = crate::models::get_model_snapshot(
                    storage,
                    &GetModelSnapshotRequest {
                        provider_id: provider.as_str().to_owned(),
                        model_id: input.model_id,
                    },
                )
                .map_err(apply_model_failure)?;
                let plan = ClaudeCodeApplyInput {
                    plan: ClaudeCodePlanInput {
                        config_root,
                        provider: ProviderFacts {
                            id: record.id.as_str().to_owned(),
                            kind: record.kind,
                            base_url: record.base_url,
                            revision: record.revision,
                        },
                        model: ModelFacts {
                            id: snapshot.model.model_id,
                            content_digest: snapshot.content_digest,
                        },
                    },
                    reviewed: input.reviewed,
                    backups,
                };
                agents::apply_claude_code_settings(storage, &OsCredentialStore, &plan)
            })
        })
    })
    .await
    .map_err(|_| ClaudeCodeApplyError::OperationFailed)?
}

/// Preview what undoing one recorded write would do, without writing anything.
///
/// The answer names the files, the action per file and — when the log can prove
/// it — the Provider whose key the restored content carries. No file content and
/// no credential value is read into it.
#[tauri::command]
pub(crate) async fn plan_claude_code_restore(
    app: tauri::AppHandle,
    operation_id: String,
) -> Result<ClaudeCodeRestorePlan, ClaudeCodeRestoreError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::plan_claude_code_restore(storage, &operation_id)
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Apply a restore the user confirmed, writing the previous content back.
///
/// The write layer rebuilds the plan and compares every token the interface sent
/// back, so a settings edit made after the preview is refused with `plan_stale`
/// before any file is touched. Restoring reads no credential: the previous
/// content comes from the private copy, and the settings file ends up with
/// whichever key it held before the write being undone.
#[tauri::command]
pub(crate) async fn apply_claude_code_restore(
    app: tauri::AppHandle,
    input: ApplyClaudeCodeRestoreRequest,
) -> Result<RestoreOutcome, ClaudeCodeRestoreError> {
    // Restored content is copied from and to the private backup directory next
    // to the database, never inside the settings folder it was read from.
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::apply_claude_code_restore(storage, &input, &backups)
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Read a root-scoped history page, including separate restore/compensation batches.
/// The root comes only from the desktop environment; no CLI or config is opened.
#[tauri::command]
pub(crate) async fn configuration_history_page(
    app: tauri::AppHandle,
    input: agents::history::ConfigurationHistoryRequest,
) -> Result<agents::history::ConfigurationHistoryPage, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::history::configuration_history_page(
                storage,
                &environment,
                &input,
                &crate::config_apply::BackupStore::new(backups),
            )
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Preview restoring one history entry's retained version.
///
/// The Agent identity selects the root and owned files; the request carries no
/// paths. Reading a preview never writes files or reads credentials.
#[tauri::command]
pub(crate) async fn plan_configuration_version_restore(
    app: tauri::AppHandle,
    input: agents::version_restore::VersionRestoreRequest,
) -> Result<crate::config_apply::VersionRestorePlan, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::version_restore::plan_version_restore(
                storage,
                &environment,
                &input,
                &crate::config_apply::BackupStore::new(backups),
            )
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Delete one history entry and its exclusive private backups (H3).
///
/// Accepts only Agent + entryKey. Roots and backup locations come from the
/// desktop environment. Does not change Agent configuration files, credentials
/// or alias ownership. Retryable leftovers return as an outcome, not an error.
#[tauri::command]
pub(crate) async fn delete_configuration_history_entry(
    app: tauri::AppHandle,
    input: agents::history_delete::DeleteHistoryRequest,
) -> Result<crate::config_apply::HistoryDeleteOutcome, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::history_delete::delete_history_entry(
                storage,
                &environment,
                &input,
                &crate::config_apply::BackupStore::new(backups),
            )
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Apply a reviewed version restore, keeping every snapshot for a later undo.
#[tauri::command]
pub(crate) async fn apply_configuration_version_restore(
    app: tauri::AppHandle,
    input: agents::version_restore::ApplyVersionRestoreRequest,
) -> Result<crate::config_apply::VersionRestoreOutcome, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::version_restore::apply_version_restore(
                storage,
                &environment,
                &input,
                &crate::config_apply::BackupStore::new(backups),
            )
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Read sanitized current/retained text for one version-restore file.
///
/// The preview's current digest is required to match; text is sanitized in Rust
/// before it crosses IPC, and no backup path or credential value is returned.
#[tauri::command]
pub(crate) async fn read_configuration_version_text(
    app: tauri::AppHandle,
    input: agents::version_restore::VersionTextRequest,
) -> Result<crate::config_apply::VersionText, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::version_restore::read_version_text(
                storage,
                &environment,
                &input,
                &crate::config_apply::BackupStore::new(backups),
            )
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Read the live, sanitized configuration text for one Agent scope file.
///
/// Independent of history matching: the dialog remains usable after an
/// external edit changed the file. The input names an Agent and one of the
/// paths the history response already showed; Rust revalidates it against the
/// trusted scope and never treats it as an arbitrary filesystem path.
#[tauri::command]
pub(crate) async fn read_current_configuration_text(
    app: tauri::AppHandle,
    input: agents::version_restore::CurrentConfigTextRequest,
) -> Result<crate::config_apply::CurrentConfigText, ClaudeCodeRestoreError> {
    let environment = environment(&app).map_err(|_| ClaudeCodeRestoreError::OperationFailed)?;
    tauri::async_runtime::spawn_blocking(move || {
        agents::version_restore::read_current_configuration_text(&environment, &input)
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Inspect one supported Agent's interrupted write using the private backup root.
/// Reads metadata/files on the blocking pool; never reads credential storage.
#[tauri::command]
pub(crate) async fn review_configuration_resolution(
    app: tauri::AppHandle,
    input: agents::recovery::ReviewResolutionRequest,
) -> Result<agents::recovery::ConfigurationResolutionReview, ClaudeCodeRestoreError> {
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::recovery::review_configuration_resolution(storage, &input, &backups)
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Record reviewed observations only; restoration requires separate confirmation.
/// The domain checks Agent ownership and approval digest before locked resolution.
#[tauri::command]
pub(crate) async fn resolve_configuration_operation(
    app: tauri::AppHandle,
    input: agents::recovery::ResolveConfigurationRequest,
) -> Result<agents::recovery::ConfigurationResolutionOutcome, ClaudeCodeRestoreError> {
    let backups = app
        .path()
        .app_data_dir()
        .map_err(|_| ClaudeCodeRestoreError::StorageUnavailable)?
        .join("backups");
    tauri::async_runtime::spawn_blocking(move || {
        with_restore_storage(&app, |storage| {
            agents::recovery::resolve_configuration_operation(storage, &input, &backups)
        })
    })
    .await
    .map_err(|_| ClaudeCodeRestoreError::OperationFailed)?
}

/// Map a Provider read failure onto the preview's error namespace.
pub(super) fn provider_failure(error: ProviderError) -> PlanError {
    match error {
        ProviderError::NotFound => PlanError::ProviderNotFound,
        ProviderError::InvalidRequest => PlanError::InvalidRequest,
        ProviderError::StorageUnavailable
        | ProviderError::ReadFailed
        | ProviderError::WriteFailed => PlanError::StorageUnavailable,
        _ => PlanError::OperationFailed,
    }
}

/// Map a model read failure onto the preview's error namespace.
pub(super) fn model_failure(error: ModelError) -> PlanError {
    match error {
        ModelError::NotFound => PlanError::ProviderNotFound,
        ModelError::ModelNotFound => PlanError::ModelNotFound,
        ModelError::InvalidRequest | ModelError::ModelIdInvalid => PlanError::InvalidRequest,
        ModelError::StorageUnavailable | ModelError::ReadFailed | ModelError::WriteFailed => {
            PlanError::StorageUnavailable
        }
        _ => PlanError::OperationFailed,
    }
}

/// Map a Provider read failure onto the apply step's error namespace.
pub(super) fn apply_provider_failure(error: ProviderError) -> ClaudeCodeApplyError {
    match error {
        ProviderError::NotFound => ClaudeCodeApplyError::ProviderNotFound,
        ProviderError::InvalidRequest => ClaudeCodeApplyError::InvalidRequest,
        ProviderError::StorageUnavailable
        | ProviderError::ReadFailed
        | ProviderError::WriteFailed => ClaudeCodeApplyError::StorageUnavailable,
        _ => ClaudeCodeApplyError::OperationFailed,
    }
}

/// Map a model read failure onto the apply step's error namespace.
pub(super) fn apply_model_failure(error: ModelError) -> ClaudeCodeApplyError {
    match error {
        ModelError::NotFound => ClaudeCodeApplyError::ProviderNotFound,
        ModelError::ModelNotFound => ClaudeCodeApplyError::ModelNotFound,
        ModelError::InvalidRequest | ModelError::ModelIdInvalid => {
            ClaudeCodeApplyError::InvalidRequest
        }
        ModelError::StorageUnavailable | ModelError::ReadFailed | ModelError::WriteFailed => {
            ClaudeCodeApplyError::StorageUnavailable
        }
        _ => ClaudeCodeApplyError::OperationFailed,
    }
}
