//! Desktop startup and command registration for vibemate.

mod commands;
pub mod credentials;
pub mod storage;

use tauri::Manager;

/// Start the desktop runtime, open private storage, and register UI commands.
///
/// Tauri owns the window and event loop. Startup stops if the private database
/// cannot be opened or upgraded, because configuration features cannot work
/// safely without it. Tauri reports the returned error.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // The database lives in the platform app-data folder, never in the project.
            let data_directory = app.path().app_data_dir()?;
            let storage = storage::Storage::open_in_directory(&data_directory)?;
            // Managed state is shared by all commands; `Storage` serializes its own access.
            app.manage(storage);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
        .expect("failed to start vibemate desktop runtime");
}
