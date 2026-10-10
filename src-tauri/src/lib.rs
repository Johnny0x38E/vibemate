//! Desktop startup and command registration for vibemate.

pub mod appearance;
mod commands;
pub mod credentials;
pub mod settings;
pub mod storage;

use std::io::Write;

use tauri::Manager;

/// Start the desktop runtime, open private storage, and register UI commands.
///
/// The window opens even when private storage is unavailable. The storage outcome
/// stays in managed state, so commands can report it and a later backup restore can
/// rebuild the database. Only a failure of Tauri itself stops the process.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // The database lives in the platform app-data folder, never in the project.
            let status = match app.path().app_data_dir() {
                Ok(data_directory) => storage::StorageStatus::open_in_directory(&data_directory),
                Err(error) => {
                    report_startup_problem(&format!(
                        "vibemate could not locate its app-data folder: {error}"
                    ));
                    storage::StorageStatus::Unavailable(storage::StorageError::LocateDataDirectory)
                }
            };
            if let Err(error) = status.storage() {
                report_startup_problem(&error.to_string());
            }
            // Managed state is shared by all commands; `StorageStatus` is safe across threads.
            app.manage(status);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_locale_preference,
            commands::save_locale_preference,
            commands::get_appearance_preference,
            commands::save_appearance_preference,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start vibemate desktop runtime");
}

/// Write a startup problem to stderr so a developer running from a terminal can see it.
///
/// Writing is best effort. A GUI launch may have no terminal, and a failed log line
/// must not stop the app. Messages must never contain secrets.
fn report_startup_problem(message: &str) {
    let _ = writeln!(std::io::stderr(), "{message}");
}
