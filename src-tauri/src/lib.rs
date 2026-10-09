//! Desktop startup and command registration for vibemate.

mod commands;

/// Start the desktop runtime and register the commands callable by the UI.
///
/// Tauri owns the window and event loop. Startup failure terminates the app
/// because there is no usable interface without the runtime.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
        .expect("failed to start vibemate desktop runtime");
}
