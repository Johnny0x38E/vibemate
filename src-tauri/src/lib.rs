//! Desktop startup and command registration for vibemate.

pub mod appearance;
mod commands;
pub mod credentials;
pub mod http_client;
mod log_access;
mod logging;
pub mod mcp;
pub mod model_catalog;
pub mod model_fetch;
pub mod model_search;
pub mod models;
pub mod provider_secrets;
pub mod providers;
pub mod settings;
pub mod storage;

use tauri::Manager;

/// Start the desktop runtime, open private storage, and register UI commands.
///
/// The window opens even when private storage is unavailable. The storage outcome
/// stays in managed state, so commands can report it and a later backup restore can
/// rebuild the database. Only a failure of Tauri itself stops the process.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let file_logging_active = logging::initialize(app.handle());
            app.manage(log_access::LogAccess::new(
                app.path().app_log_dir().ok(),
                file_logging_active,
            ));
            // The database lives in the platform app-data folder, never in the project.
            let status = match app.path().app_data_dir() {
                Ok(data_directory) => storage::StorageStatus::open_in_directory(&data_directory),
                Err(_) => {
                    storage::StorageStatus::Unavailable(storage::StorageError::LocateDataDirectory)
                }
            };
            if let Err(error) = status.storage() {
                // Display is intentionally safe; never format Debug or the error source.
                log::error!(target: "vibemate", "event=storage_unavailable reason={error}");
            } else {
                log::info!(target: "vibemate", "event=storage_ready");
            }
            // Managed state is shared by all commands; `StorageStatus` is safe across threads.
            app.manage(status);
            app.manage(commands::CredentialLock::default());
            app.manage(model_fetch::ModelFetchRegistry::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_mcp_definitions,
            commands::get_mcp_definition,
            commands::save_mcp_definition,
            commands::cleanup_mcp_credentials,
            commands::get_log_location,
            commands::open_log_file,
            commands::open_log_directory,
            commands::open_project_repository,
            commands::get_locale_preference,
            commands::save_locale_preference,
            commands::get_appearance_preference,
            commands::save_appearance_preference,
            commands::list_provider_templates,
            commands::list_providers,
            commands::get_provider,
            commands::create_provider,
            commands::update_provider,
            commands::get_provider_secret_status,
            commands::replace_provider_secret,
            commands::fetch_provider_models,
            commands::browse_upstream_models_page,
            commands::cancel_provider_model_fetch,
            commands::get_provider_model_fetch_status,
            commands::list_provider_models,
            commands::save_provider_model_selections,
            commands::set_provider_models_selected,
            commands::add_manual_provider_model,
            commands::delete_manual_provider_model,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|_| {
            log::error!(target: "vibemate", "event=desktop_runtime_failed");
            log::logger().flush();
            panic!("failed to start vibemate desktop runtime");
        });
    log::info!(target: "vibemate", "event=application_exit");
    log::logger().flush();
}
