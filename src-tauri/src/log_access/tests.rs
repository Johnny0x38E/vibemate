//! Log access tests use temporary paths and fake openers, never desktop apps.

use super::*;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "vibemate-log-access-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn access(&self) -> LogAccess {
        LogAccess::new(Some(self.0.clone()), true)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn metadata_preserves_actual_paths_and_fallback_status_without_creating_files() {
    let directory = TestDirectory::new();
    let missing = directory.0.join("missing directory");
    let access = LogAccess::new(Some(missing.clone()), false);
    let location = access.location().unwrap();
    assert_eq!(
        location.file_path,
        missing.join("vibemate.log").to_str().unwrap()
    );
    assert_eq!(location.directory_path, missing.to_str().unwrap());
    assert!(!location.file_logging_active);
    assert!(!missing.exists());
    let json = serde_json::to_value(location).unwrap();
    assert_eq!(json["fileLoggingActive"], false);
    assert!(json.get("filePath").is_some());
    assert!(json.get("directoryPath").is_some());
}

#[test]
fn unresolved_paths_fail_without_calling_an_opener() {
    let access = LogAccess::new(None, false);
    assert!(matches!(
        access.location(),
        Err(LogAccessError::PathUnavailable)
    ));
    assert_eq!(
        access.open_file_with(|_| panic!("must not open")),
        Err(LogAccessError::PathUnavailable)
    );
    assert_eq!(
        access.open_directory_with(|_| panic!("must not open")),
        Err(LogAccessError::PathUnavailable)
    );
}

#[test]
fn missing_or_wrong_kind_paths_are_not_opened_or_created() {
    let directory = TestDirectory::new();
    let access = directory.access();
    assert_eq!(
        access.open_file_with(|_| panic!("must not open")),
        Err(LogAccessError::LogUnavailable)
    );
    assert!(!directory.0.join("vibemate.log").exists());
    std::fs::create_dir(directory.0.join("vibemate.log")).unwrap();
    assert_eq!(
        access.open_file_with(|_| panic!("must not open")),
        Err(LogAccessError::LogUnavailable)
    );
    let missing = directory.0.join("missing");
    let access = LogAccess::new(Some(missing.clone()), true);
    assert_eq!(
        access.open_directory_with(|_| panic!("must not open")),
        Err(LogAccessError::LogUnavailable)
    );
    assert!(!missing.exists());
}

#[test]
fn explicit_actions_pass_only_the_fixed_file_and_directory_and_preserve_contents() {
    let directory = TestDirectory::new();
    let file = directory.0.join("vibemate.log");
    std::fs::write(&file, "event=test").unwrap();
    let access = directory.access();
    access
        .open_file_with(|path| {
            assert_eq!(path, file);
            Ok(())
        })
        .unwrap();
    access
        .open_directory_with(|path| {
            assert_eq!(path, directory.0);
            Ok(())
        })
        .unwrap();
    assert_eq!(std::fs::read_to_string(file).unwrap(), "event=test");
}

#[test]
fn opener_failure_is_returned_and_an_old_log_can_be_opened_after_stderr_fallback() {
    let directory = TestDirectory::new();
    std::fs::write(directory.0.join("vibemate.log"), "old log").unwrap();
    let access = LogAccess::new(Some(directory.0.clone()), false);
    assert_eq!(
        access.open_file_with(|_| Err(LogAccessError::OpenFailed)),
        Err(LogAccessError::OpenFailed)
    );
    assert_eq!(
        access.open_directory_with(|_| Err(LogAccessError::OpenFailed)),
        Err(LogAccessError::OpenFailed)
    );
    assert_eq!(access.open_file_with(|_| Ok(())), Ok(()));
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_not_lossily_displayed_as_another_path() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec()));
    let access = LogAccess::new(Some(path), false);
    assert!(matches!(
        access.location(),
        Err(LogAccessError::PathUnavailable)
    ));
}
