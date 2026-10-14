//! Isolated configuration-write tests. Every fixture is a temporary directory
//! removed when the test ends; no real Agent configuration is touched.
use super::*;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// Settings content used in place of a real Agent file.
const FIRST: &str = "{\"theme\":\"dark\"}\n";
const SECOND: &str = "{\"theme\":\"light\"}\n";
/// Marker shared with the temporary file name built in [`replace`].
const TEMPORARY_MARKER: &str = ".vibemate-";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vibemate-config-apply-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn target(&self) -> PathBuf {
        self.0.join("settings.json")
    }

    fn backups(&self) -> BackupStore {
        BackupStore::new(self.0.join("backups"))
    }

    /// The token a preview would have produced for the current file state.
    fn plan(&self) -> WriteTarget {
        let document = read_target_document(&self.target()).unwrap();
        WriteTarget::from_plan(&document.target)
    }

    fn content(&self) -> String {
        fs::read_to_string(self.target()).unwrap()
    }

    fn write(&self, content: &str) {
        fs::write(self.target(), content).unwrap();
    }

    /// How many copies the backup store holds, whether or not its root exists.
    fn stored_backups(&self) -> usize {
        fs::read_dir(self.backups().root())
            .map(|entries| entries.count())
            .unwrap_or(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Temporary files a write could leave behind in `directory`.
fn leftovers(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(TEMPORARY_MARKER))
        .collect();
    names.sort();
    names
}

/// Permission bits of a file or directory, without the file-type bits.
#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

#[test]
fn an_unchanged_file_is_not_written_again() {
    let fixture = Fixture::new();
    fixture.write(FIRST);
    let applied = apply_file(&fixture.plan(), FIRST.as_bytes(), &fixture.backups()).unwrap();
    assert!(!applied.changed);
    assert_eq!(applied.backup, None);
    assert_eq!(
        applied.target.content_digest.as_deref(),
        Some(digest_hex(FIRST.as_bytes()).as_str())
    );
    assert_eq!(fixture.content(), FIRST);
    assert_eq!(fixture.stored_backups(), 0);
    assert!(leftovers(&fixture.0).is_empty());
}

#[cfg(unix)]
#[test]
fn replacing_a_file_keeps_its_permissions_and_backs_up_the_original() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.write(FIRST);
    fs::set_permissions(fixture.target(), fs::Permissions::from_mode(0o640)).unwrap();
    let applied = apply_file(&fixture.plan(), SECOND.as_bytes(), &fixture.backups()).unwrap();

    assert!(applied.changed);
    assert_eq!(fixture.content(), SECOND);
    assert_eq!(mode_of(&fixture.target()), 0o640);
    assert!(!applied.target.symlink);
    assert_eq!(
        applied.target.content_digest.as_deref(),
        Some(digest_hex(SECOND.as_bytes()).as_str())
    );

    let backup = applied.backup.unwrap();
    assert_eq!(fs::read(&backup.path).unwrap(), FIRST.as_bytes());
    assert_eq!(backup.content_digest, digest_hex(FIRST.as_bytes()));
    assert_eq!(mode_of(Path::new(&backup.path)), 0o600);
    assert_eq!(mode_of(Path::new(&backup.path).parent().unwrap()), 0o700);
    assert!(leftovers(&fixture.0).is_empty());
}

#[cfg(unix)]
#[test]
fn a_new_file_is_created_with_private_permissions() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    assert_eq!(plan.expected_digest, None);
    let applied = apply_file(&plan, FIRST.as_bytes(), &fixture.backups()).unwrap();

    assert!(applied.changed);
    assert_eq!(applied.backup, None);
    assert!(applied.target.exists);
    assert_eq!(mode_of(&fixture.target()), 0o600);
    assert!(!fixture.backups().root().exists());
    assert!(leftovers(&fixture.0).is_empty());
}

#[test]
fn a_missing_parent_directory_is_created() {
    let fixture = Fixture::new();
    let path = fixture.0.join("nested").join("deep").join("settings.json");
    let plan = WriteTarget {
        path: path.clone(),
        expected_digest: None,
    };
    let applied = apply_file(&plan, FIRST.as_bytes(), &fixture.backups()).unwrap();
    assert!(applied.changed);
    assert_eq!(fs::read_to_string(&path).unwrap(), FIRST);
}

#[test]
fn a_concurrent_change_is_refused_without_writing() {
    let fixture = Fixture::new();
    fixture.write(FIRST);
    let plan = fixture.plan();
    // Another editor replaced the file while the preview was on screen.
    fixture.write(SECOND);
    let error = apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err();

    assert_eq!(error, WriteError::TargetChanged);
    assert_eq!(fixture.content(), SECOND);
    assert_eq!(fixture.stored_backups(), 0);
    assert!(leftovers(&fixture.0).is_empty());
}

#[test]
fn a_file_that_appeared_after_the_plan_is_refused() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    fixture.write(FIRST);
    assert_eq!(
        apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err(),
        WriteError::TargetChanged
    );
    assert_eq!(fixture.content(), FIRST);
}

#[test]
fn a_file_that_disappeared_after_the_plan_is_refused() {
    let fixture = Fixture::new();
    fixture.write(FIRST);
    let plan = fixture.plan();
    fs::remove_file(fixture.target()).unwrap();
    assert_eq!(
        apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err(),
        WriteError::TargetChanged
    );
    assert!(!fixture.target().exists());
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_target_is_refused() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let real = fixture.0.join("real-settings.json");
    fs::write(&real, FIRST).unwrap();
    symlink(&real, fixture.target()).unwrap();
    let plan = fixture.plan();
    let error = apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err();

    assert_eq!(error, WriteError::SymlinkRefused);
    assert_eq!(fs::read_to_string(&real).unwrap(), FIRST);
    assert!(
        fs::symlink_metadata(fixture.target())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fixture.stored_backups(), 0);
    assert!(leftovers(&fixture.0).is_empty());
}

#[cfg(unix)]
#[test]
fn a_read_only_target_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.write(FIRST);
    fs::set_permissions(fixture.target(), fs::Permissions::from_mode(0o400)).unwrap();
    let error = apply_file(&fixture.plan(), SECOND.as_bytes(), &fixture.backups()).unwrap_err();

    assert_eq!(error, WriteError::TargetReadOnly);
    assert_eq!(fixture.content(), FIRST);
    assert_eq!(fixture.stored_backups(), 0);
}

#[test]
fn a_directory_target_is_refused() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.target()).unwrap();
    let plan = WriteTarget {
        path: fixture.target(),
        expected_digest: None,
    };
    assert_eq!(
        apply_file(&plan, FIRST.as_bytes(), &fixture.backups()).unwrap_err(),
        WriteError::TargetUnreadable
    );
    assert_eq!(fixture.stored_backups(), 0);
}

#[cfg(unix)]
#[test]
fn a_write_failure_leaves_the_target_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let directory = fixture.0.join("config");
    fs::create_dir(&directory).unwrap();
    let target = directory.join("settings.json");
    fs::write(&target, FIRST).unwrap();
    let plan = WriteTarget {
        path: target.clone(),
        expected_digest: Some(digest_hex(FIRST.as_bytes())),
    };
    // The backup root stays writable while the target directory refuses new files.
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).unwrap();
    let error = apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(error, WriteError::WriteFailed);
    assert_eq!(fs::read_to_string(&target).unwrap(), FIRST);
    assert!(leftovers(&directory).is_empty());
    assert_eq!(fixture.stored_backups(), 0);
}

#[test]
fn an_unavailable_backup_root_refuses_the_write() {
    let fixture = Fixture::new();
    fixture.write(FIRST);
    // A file where the backup directory would be created.
    let blocked = fixture.0.join("blocked");
    fs::write(&blocked, "not a directory").unwrap();
    let error = apply_file(
        &fixture.plan(),
        SECOND.as_bytes(),
        &BackupStore::new(blocked.join("backups")),
    )
    .unwrap_err();

    assert_eq!(error, WriteError::BackupUnavailable);
    assert_eq!(fixture.content(), FIRST);
    assert!(leftovers(&fixture.0).is_empty());
}

#[test]
fn each_write_keeps_its_own_backup() {
    let fixture = Fixture::new();
    fixture.write(FIRST);
    let first = apply_file(&fixture.plan(), SECOND.as_bytes(), &fixture.backups()).unwrap();
    let second = apply_file(&fixture.plan(), FIRST.as_bytes(), &fixture.backups()).unwrap();

    let (first_backup, second_backup) = (first.backup.unwrap(), second.backup.unwrap());
    assert_ne!(first_backup.id, second_backup.id);
    assert_eq!(fs::read_to_string(&first_backup.path).unwrap(), FIRST);
    assert_eq!(fs::read_to_string(&second_backup.path).unwrap(), SECOND);
    assert_eq!(fixture.content(), FIRST);
    assert_eq!(fixture.stored_backups(), 2);
}

#[test]
fn refusals_never_carry_file_content() {
    let fixture = Fixture::new();
    fixture.write("{\"env\":{\"ANTHROPIC_AUTH_TOKEN\":\"synthetic-secret\"}}\n");
    let plan = fixture.plan();
    fixture.write(SECOND);
    let error = apply_file(&plan, SECOND.as_bytes(), &fixture.backups()).unwrap_err();

    assert_eq!(error, WriteError::TargetChanged);
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("synthetic-secret"));
        assert!(!text.contains(fixture.0.to_str().unwrap()));
    }
}
