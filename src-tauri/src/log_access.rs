//! Read and open only the application's fixed local log file and directory.
//!
//! Paths are resolved at startup, never supplied by the WebView. Reading this
//! metadata does not create files or directories. OS launching is kept separate
//! from path checks so tests never start an editor or file manager.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(test)]
mod tests;

/// Safe failures for reading log metadata and requesting an OS open operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogAccessError {
    /// The platform log path cannot be resolved or represented in the interface.
    PathUnavailable,
    /// The requested log file or directory does not exist or cannot be inspected.
    LogUnavailable,
    /// The OS could not accept the open request; no raw platform error is exposed.
    OpenFailed,
    /// The blocking operation could not finish.
    OperationFailed,
}

/// Display-only paths and whether this process successfully enabled file logging.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogLocation {
    file_path: String,
    directory_path: String,
    file_logging_active: bool,
}

/// Startup-owned log location. Commands can open only these two fixed paths.
pub(crate) struct LogAccess {
    directory: Option<PathBuf>,
    file_logging_active: bool,
}

impl LogAccess {
    /// Remember the resolved directory and startup logger outcome without I/O.
    pub(crate) fn new(directory: Option<PathBuf>, file_logging_active: bool) -> Self {
        Self {
            directory,
            file_logging_active,
        }
    }

    fn directory(&self) -> Result<&Path, LogAccessError> {
        self.directory
            .as_deref()
            .ok_or(LogAccessError::PathUnavailable)
    }

    fn file(&self) -> Result<PathBuf, LogAccessError> {
        Ok(self.directory()?.join("vibemate.log"))
    }

    /// Return the actual configured paths, even when logging fell back to stderr.
    /// Returns `PathUnavailable` if resolution or UTF-8 conversion is impossible.
    pub(crate) fn location(&self) -> Result<LogLocation, LogAccessError> {
        let file = self.file()?;
        Ok(LogLocation {
            file_path: file.to_str().ok_or(LogAccessError::PathUnavailable)?.into(),
            directory_path: self
                .directory()?
                .to_str()
                .ok_or(LogAccessError::PathUnavailable)?
                .into(),
            file_logging_active: self.file_logging_active,
        })
    }

    /// Check the fixed log file before requesting an editor. No file is created.
    /// Missing/inaccessible files return `LogUnavailable`; opener errors pass through.
    pub(crate) fn open_file_with(
        &self,
        open: impl FnOnce(&Path) -> Result<(), LogAccessError>,
    ) -> Result<(), LogAccessError> {
        let file = self.file()?;
        if !file.is_file() {
            return Err(LogAccessError::LogUnavailable);
        }
        open(&file)
    }

    /// Check the fixed log directory before requesting a file manager.
    /// Missing/inaccessible directories return `LogUnavailable`; nothing is created.
    pub(crate) fn open_directory_with(
        &self,
        open: impl FnOnce(&Path) -> Result<(), LogAccessError>,
    ) -> Result<(), LogAccessError> {
        let directory = self.directory()?;
        if !directory.is_dir() {
            return Err(LogAccessError::LogUnavailable);
        }
        open(directory)
    }
}

/// Ask a known text tool to open a log file; never select a frontend-supplied app.
///
/// macOS uses TextEdit, Windows uses Notepad, Linux uses the default association.
/// The opener confirms only dispatch, not that a user saw or read the file.
/// Raw OS failures are discarded and mapped to `OpenFailed`.
pub(crate) fn open_in_text_tool(path: &Path) -> Result<(), LogAccessError> {
    #[cfg(target_os = "macos")]
    let editor = Some("TextEdit");
    #[cfg(target_os = "windows")]
    let editor = Some("notepad");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let editor: Option<&str> = None;
    tauri_plugin_opener::open_path(path, editor).map_err(|_| LogAccessError::OpenFailed)
}

/// Ask the default file manager to open the fixed log folder.
/// Returns `OpenFailed` when dispatch fails; does not inspect the launched app.
pub(crate) fn open_in_file_manager(path: &Path) -> Result<(), LogAccessError> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|_| LogAccessError::OpenFailed)
}
