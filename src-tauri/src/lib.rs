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
mod shared;
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
            commands::app::get_app_info,
            commands::mcp::list_mcp_definitions,
            commands::mcp::get_mcp_definition,
            commands::mcp::save_mcp_definition,
            commands::mcp::cleanup_mcp_credentials,
            commands::app::get_log_location,
            commands::app::open_log_file,
            commands::app::open_log_directory,
            commands::app::open_project_repository,
            commands::preferences::get_locale_preference,
            commands::preferences::save_locale_preference,
            commands::preferences::get_appearance_preference,
            commands::preferences::save_appearance_preference,
            commands::providers::list_provider_templates,
            commands::providers::list_providers,
            commands::providers::get_provider,
            commands::providers::create_provider,
            commands::providers::update_provider,
            commands::providers::get_provider_secret_status,
            commands::providers::replace_provider_secret,
            commands::models::fetch_provider_models,
            commands::models::browse_upstream_models_page,
            commands::models::cancel_provider_model_fetch,
            commands::models::get_provider_model_fetch_status,
            commands::models::list_provider_models,
            commands::models::save_provider_model_selections,
            commands::models::set_provider_models_selected,
            commands::models::add_manual_provider_model,
            commands::models::delete_manual_provider_model,
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
