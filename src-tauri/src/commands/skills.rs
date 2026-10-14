//! Skill library IPC: list entries, choose a folder, import its skills, record
//! a source and delete a reviewed entry. Importing copies files into the private
//! library and never executes a skill command; deleting removes only vibemate's
//! own copy. All work happens on the blocking pool because it touches the disk,
//! and the folder dialog is opened from Rust so the frontend never calls the
//! dialog plugin's own commands.

use super::state::with_skill_library;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

/// Longest accepted dialog title. The interface sends a translated label.
const MAX_TITLE_LENGTH: usize = 120;

/// Open the system folder picker and report the choice.
///
/// Closing the dialog is a normal outcome (`cancelled`), not a failure. The
/// picker accepts any folder: the import decides whether it is one skill or a
/// folder of skills.
#[tauri::command]
pub(crate) async fn pick_skill_folder(
    app: tauri::AppHandle,
    title: String,
) -> Result<crate::skills::PickedFolder, crate::skills::SkillError> {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_TITLE_LENGTH {
        return Err(crate::skills::SkillError::InvalidRequest);
    }
    let mut dialog = app.dialog().file().set_title(trimmed);
    // A window-scoped sheet keeps the dialog attached to the vibemate window on
    // platforms that support it; other platforms ignore the parent.
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    // The blocking call must stay off the main thread; `spawn_blocking` above
    // guarantees that, and the plugin dispatches the UI work to the main thread.
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .map_err(|_| crate::skills::SkillError::OperationFailed)?;
    match picked.and_then(|folder| folder.into_path().ok()) {
        Some(path) => match path.to_str() {
            Some(text) => Ok(crate::skills::PickedFolder::Selected {
                path: text.to_string(),
            }),
            // A path the interface cannot carry as text is reported like a cancel.
            None => Ok(crate::skills::PickedFolder::Cancelled),
        },
        None => Ok(crate::skills::PickedFolder::Cancelled),
    }
}

/// Import every skill in the chosen folder: the folder itself when it is a
/// skill, otherwise each of its immediate skill subfolders.
#[tauri::command]
pub(crate) async fn import_skill_folder(
    app: tauri::AppHandle,
    request: crate::skills::ImportFolderRequest,
) -> Result<crate::skills::ImportFolderReport, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        let now = crate::shared::current_unix_millis()
            .map_err(|_| crate::skills::SkillError::OperationFailed)?;
        with_skill_library(&app, |library, storage| {
            crate::skills::import_folder(library, storage, request, now)
        })
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}

/// Read central skill metadata without copying or inspecting any directory.
#[tauri::command]
pub(crate) async fn list_skills(
    app: tauri::AppHandle,
    after: Option<String>,
    limit: u32,
) -> Result<crate::skills::SkillPage, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_skill_library(&app, |_, storage| {
            crate::skills::list_skills(storage, after.as_deref(), limit)
        })
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}

/// Read one skill's safe metadata for the source editor or a retry.
#[tauri::command]
pub(crate) async fn get_skill(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::skills::SkillRecord, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_skill_library(&app, |_, storage| crate::skills::get_skill(storage, &id))
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}

/// Record or clear the upstream source of an existing skill.
#[tauri::command]
pub(crate) async fn set_skill_source(
    app: tauri::AppHandle,
    request: crate::skills::SetSkillSourceRequest,
) -> Result<crate::skills::SkillRecord, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        let now = crate::shared::current_unix_millis()
            .map_err(|_| crate::skills::SkillError::OperationFailed)?;
        with_skill_library(&app, |_, storage| {
            crate::skills::set_skill_source(storage, request, now)
        })
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}

/// Delete one reviewed skill record and its managed copy; the user's imported
/// folder is untouched. A copy that cannot be removed is reported as pending
/// cleanup instead of making the deletion look refused.
#[tauri::command]
pub(crate) async fn delete_skill(
    app: tauri::AppHandle,
    request: crate::skills::DeleteSkillRequest,
) -> Result<crate::skills::DeletedSkill, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_skill_library(&app, |library, storage| {
            crate::skills::delete_skill(library, storage, request)
        })
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}

/// Retry the removal of parked copies left by finished deletions.
#[tauri::command]
pub(crate) async fn cleanup_deleted_skill_copies(
    app: tauri::AppHandle,
) -> Result<bool, crate::skills::SkillError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_skill_library(&app, |library, storage| {
            crate::skills::cleanup_deleted_skill_copies(library, storage)
        })
    })
    .await
    .map_err(|_| crate::skills::SkillError::OperationFailed)?
}
