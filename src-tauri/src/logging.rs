//! Local Rust diagnostics, with bounded files and a stderr-only fallback.
//!
//! Only the exact `vibemate` target is accepted. Dependency logs (especially HTTP
//! diagnostics) are excluded even in debug builds. Callers must use fixed event
//! names and safe error enums, never request values, raw errors, or credentials.
//! The official plugin supplies the sinks; its IPC plugin is not registered.

use std::io::Write;

use log::{LevelFilter, Metadata};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::providers::ProviderError;
use crate::settings::SettingsError;

const LOG_TARGET: &str = "vibemate";
const MAX_FILE_BYTES: u128 = 1024 * 1024;
const ARCHIVE_COUNT: usize = 3;

fn accepts_record(metadata: &Metadata<'_>) -> bool {
    metadata.target() == LOG_TARGET
}

fn builder(file_target: Option<TargetKind>) -> tauri_plugin_log::Builder {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let mut targets = vec![Target::new(TargetKind::Stderr)];
    if let Some(file_target) = file_target {
        targets.push(Target::new(file_target));
    }
    tauri_plugin_log::Builder::new()
        .level(level)
        .filter(accepts_record)
        .max_file_size(MAX_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(ARCHIVE_COUNT))
        .targets(targets)
}

struct LocalLogger {
    level: LevelFilter,
    logger: Box<dyn log::Log>,
    file_unavailable: bool,
}

fn prepare_logger<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    file_target: TargetKind,
) -> Result<LocalLogger, tauri_plugin_log::Error> {
    let (logger, file_unavailable) = match builder(Some(file_target)).split(app) {
        Ok(logger) => (Ok(logger), false),
        Err(_) => (builder(None).split(app), true),
    };
    let (_unused_plugin, level, logger) = logger?;
    Ok(LocalLogger {
        level,
        logger,
        file_unavailable,
    })
}

/// Install local logging before opening storage. Failure never stops startup.
/// Returns whether file logging was enabled, so settings can report stderr fallback.
///
/// `split` creates sinks without installing a global logger. If opening the file
/// fails, retry with stderr only. Discard the returned IPC plugin: the WebView
/// must not gain an arbitrary-text logging endpoint or receive diagnostic events.
/// Raw setup errors may contain local paths, so fallback messages are fixed text.
pub(crate) fn initialize<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let Ok(local) = prepare_logger(
        app,
        TargetKind::LogDir {
            file_name: Some("vibemate".into()),
        },
    ) else {
        let _ = writeln!(std::io::stderr(), "vibemate: logging initialization failed");
        return false;
    };
    if tauri_plugin_log::attach_logger(local.level, local.logger).is_err() {
        let _ = writeln!(std::io::stderr(), "vibemate: logger could not be installed");
        return false;
    }
    if local.file_unavailable {
        log::warn!(target: LOG_TARGET, "event=log_file_unavailable fallback=stderr");
    }
    log::info!(target: LOG_TARGET, "event=application_start version={}", env!("CARGO_PKG_VERSION"));
    !local.file_unavailable
}

/// Record a provider command's safe error code, without inspecting its payload.
///
/// `operation` must be a fixed command name, never user input. The result is
/// returned unchanged, including unknown-outcome errors. Successful reads and
/// writes are debug events so release logs stay small.
pub(crate) fn provider_outcome<T>(
    operation: &'static str,
    result: Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    match &result {
        Ok(_) => log::debug!(target: LOG_TARGET, "event=command_complete operation={operation}"),
        Err(error) => {
            log::warn!(target: LOG_TARGET, "event=command_failed operation={operation} code={error:?}")
        }
    }
    result
}

/// Record a preference command's safe enum code and return its result unchanged.
/// Only fixed command names are allowed; no preference contents are formatted.
pub(crate) fn settings_outcome<T>(
    operation: &'static str,
    result: Result<T, SettingsError>,
) -> Result<T, SettingsError> {
    match &result {
        Ok(_) => log::debug!(target: LOG_TARGET, "event=command_complete operation={operation}"),
        Err(error) => {
            log::warn!(target: LOG_TARGET, "event=command_failed operation={operation} code={error:?}")
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "vibemate-logging-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn target(&self) -> TargetKind {
            TargetKind::Folder {
                path: self.0.clone(),
                file_name: Some("vibemate".into()),
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_record(logger: &dyn log::Log, target: &str, message: &str) {
        logger.log(
            &log::Record::builder()
                .target(target)
                .level(log::Level::Info)
                .args(format_args!("{message}"))
                .build(),
        );
        logger.flush();
    }

    #[test]
    fn file_sink_appends_on_restart_and_excludes_dependency_messages() {
        let directory = TestDirectory::new();
        let app = tauri::test::mock_app();
        let first = prepare_logger(app.handle(), directory.target()).unwrap();
        assert!(!first.file_unavailable);
        write_record(first.logger.as_ref(), LOG_TARGET, "event=first_start");
        write_record(
            first.logger.as_ref(),
            "reqwest",
            "Authorization: test-secret",
        );
        drop(first);
        let second = prepare_logger(app.handle(), directory.target()).unwrap();
        write_record(second.logger.as_ref(), LOG_TARGET, "event=second_start");
        drop(second);
        let text = std::fs::read_to_string(directory.0.join("vibemate.log")).unwrap();
        assert!(text.contains("event=first_start"));
        assert!(text.contains("event=second_start"));
        assert!(!text.contains("test-secret"));
        assert!(!text.contains("Authorization"));
    }

    #[test]
    fn file_setup_failure_returns_a_working_stderr_logger() {
        let directory = TestDirectory::new();
        let blocked = directory.0.join("not-a-directory");
        std::fs::write(&blocked, "untouched").unwrap();
        let app = tauri::test::mock_app();
        let local = prepare_logger(
            app.handle(),
            TargetKind::Folder {
                path: blocked.clone(),
                file_name: Some("vibemate".into()),
            },
        )
        .unwrap();
        assert!(local.file_unavailable);
        write_record(local.logger.as_ref(), LOG_TARGET, "event=fallback_test");
        assert_eq!(std::fs::read_to_string(blocked).unwrap(), "untouched");
    }

    #[test]
    fn rotation_keeps_bounded_archives_and_preserves_unrelated_files() {
        let directory = TestDirectory::new();
        // Old archives use the plugin's documented timestamp naming convention.
        for day in 1..=8 {
            std::fs::write(
                directory
                    .0
                    .join(format!("vibemate_2020-01-{day:02}_00-00-00.log")),
                "old event",
            )
            .unwrap();
        }
        std::fs::write(directory.0.join("other.log"), "untouched").unwrap();
        std::fs::write(directory.0.join("vibemate.log"), "x".repeat(256)).unwrap();
        let app = tauri::test::mock_app();
        let (_, _, logger) = builder(Some(directory.target()))
            .max_file_size(200)
            .split(app.handle())
            .unwrap();
        write_record(logger.as_ref(), LOG_TARGET, "event=after_rotation");
        drop(logger);
        let archives = std::fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("vibemate_"))
            .count();
        assert_eq!(archives, ARCHIVE_COUNT);
        assert!(
            std::fs::metadata(directory.0.join("vibemate.log"))
                .unwrap()
                .len()
                < 200
        );
        assert!(
            std::fs::read_to_string(directory.0.join("vibemate.log"))
                .unwrap()
                .contains("event=after_rotation")
        );
        assert_eq!(
            std::fs::read_to_string(directory.0.join("other.log")).unwrap(),
            "untouched"
        );
        assert!(
            !directory
                .0
                .join("vibemate_2020-01-01_00-00-00.log")
                .exists()
        );
    }

    #[test]
    fn only_the_application_target_is_allowed_at_every_level() {
        for level in log::Level::iter() {
            for target in [
                "reqwest",
                "hyper",
                "rustls",
                "webview",
                "vibemate::http_client",
                "",
            ] {
                let metadata = Metadata::builder().target(target).level(level).build();
                assert!(!accepts_record(&metadata), "allowed {target} at {level}");
            }
            let metadata = Metadata::builder().target(LOG_TARGET).level(level).build();
            assert!(accepts_record(&metadata));
        }
    }

    #[test]
    fn reporting_does_not_change_results_or_inspect_success_payloads() {
        // A non-Debug payload cannot accidentally be formatted by the helpers.
        struct PrivatePayload;
        assert!(provider_outcome("create_provider", Ok(PrivatePayload)).is_ok());
        assert!(settings_outcome("save_locale_preference", Ok(PrivatePayload)).is_ok());
        assert_eq!(
            provider_outcome::<()>(
                "replace_provider_secret",
                Err(ProviderError::SecretOutcomeUnknown)
            ),
            Err(ProviderError::SecretOutcomeUnknown)
        );
        assert_eq!(
            settings_outcome::<()>(
                "save_appearance_preference",
                Err(SettingsError::OperationFailed)
            ),
            Err(SettingsError::OperationFailed)
        );
    }
}
