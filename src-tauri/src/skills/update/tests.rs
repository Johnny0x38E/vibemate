//! Temporary-directory tests for skill update preview and apply.

use super::super::install::{Acquired, ResolvedSource, SkillInstaller};
use super::super::manifest::MANIFEST_FILE_NAME;
use super::super::service::{self, Provenance};
use super::super::types::{
    ApplySkillUpdateRequest, CheckSkillRevisionRequest, DeleteSkillRequest, FetchMethod,
    PreviewSkillUpdateRequest, SkillError, SkillRevisionCheckStatus, SkillUpdateStatus,
    SkillUpdateStrategy,
};
use super::{apply_with, check_revision_with, preview_with, source_from_record};
use crate::skills::{SkillLibrary, delete_skill};
use crate::storage::Storage;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_700_000_000_000;

struct Fixture {
    root: PathBuf,
    storage: Storage,
    library: SkillLibrary,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-skill-update-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let storage = Storage::open_in_directory(&root).expect("storage");
        let library = SkillLibrary::new(root.join("skills"));
        Self {
            root,
            storage,
            library,
        }
    }

    fn write_skill(&self, folder: &str, name: &str, body: &str) -> PathBuf {
        let directory = self.root.join("upstream").join(folder);
        fs::create_dir_all(&directory).expect("skill directory");
        fs::write(
            directory.join(MANIFEST_FILE_NAME),
            format!("---\nname: {name}\ndescription: A remote test skill.\n---\n\n{body}\n"),
        )
        .expect("manifest");
        directory
    }

    fn import_git(&self, name: &str, body: &str, revision: &str) -> crate::skills::SkillRecord {
        let source = self.write_skill(name, name, body);
        let provenance = Provenance {
            origin_path: None,
            source_url: Some("https://github.com/example/skills"),
            source_revision: Some(revision),
            fetch_method: Some(FetchMethod::Git),
            installer_version: None,
            installer_command: Some(
                "git fetch --depth 1 https://github.com/example/skills v1 && git checkout --detach FETCH_HEAD",
            ),
        };
        service::import_installed(&self.library, &self.storage, &source, &provenance, NOW)
            .expect("import")
    }

    fn preview(
        &self,
        id: &str,
        revision: i64,
        installer: &dyn SkillInstaller,
    ) -> Result<crate::skills::SkillUpdatePreview, SkillError> {
        preview_with(
            &self.library,
            &self.storage,
            PreviewSkillUpdateRequest {
                id: id.to_string(),
                expected_revision: revision,
            },
            installer,
        )
    }

    fn apply(
        &self,
        request: ApplySkillUpdateRequest,
        installer: &dyn SkillInstaller,
    ) -> Result<crate::skills::SkillUpdateApplied, SkillError> {
        apply_with(&self.library, &self.storage, request, NOW + 10, installer)
    }
}

fn apply_from_preview(
    preview: &crate::skills::SkillUpdatePreview,
    strategy: SkillUpdateStrategy,
) -> ApplySkillUpdateRequest {
    ApplySkillUpdateRequest {
        id: preview.skill_id.clone(),
        expected_revision: preview.skill_revision,
        strategy,
        expected_status: preview.status,
        expected_recorded_digest: preview.recorded_content_digest.clone(),
        expected_live_digest: preview.live_content_digest.clone(),
        expected_candidate_digest: preview.candidate_content_digest.clone(),
        expected_candidate_source_revision: preview.candidate_source_revision.clone(),
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target");
    for entry in fs::read_dir(source).expect("read") {
        let entry = entry.expect("entry");
        let destination = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(entry.path()).expect("meta");
        if metadata.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy");
        }
    }
}

struct FakeInstaller {
    source: PathBuf,
    revision: Option<String>,
    error: Option<SkillError>,
    workspace: Arc<Mutex<Option<PathBuf>>>,
}

impl FakeInstaller {
    fn new(source: PathBuf) -> Self {
        Self {
            source,
            revision: Some("a".repeat(40)),
            error: None,
            workspace: Arc::new(Mutex::new(None)),
        }
    }

    fn with_revision(mut self, revision: &str) -> Self {
        self.revision = Some(revision.to_string());
        self
    }

    fn with_error(mut self, error: SkillError) -> Self {
        self.error = Some(error);
        self
    }

    fn workspace_seen(&self) -> bool {
        self.workspace.lock().expect("lock").is_some()
    }
}

impl SkillInstaller for FakeInstaller {
    fn acquire(&self, _: &ResolvedSource, workspace: &Path) -> Result<Acquired, SkillError> {
        *self.workspace.lock().expect("lock") = Some(workspace.to_path_buf());
        if let Some(error) = self.error {
            return Err(error);
        }
        let root = workspace.join("acquired");
        copy_tree(&self.source, &root);
        Ok(Acquired {
            root,
            revision: self.revision.clone(),
            exit_status: 0,
        })
    }
}

#[test]
fn preview_refuses_skills_without_a_fetch_method() {
    let fixture = Fixture::new();
    let source = fixture.root.join("local");
    fs::create_dir_all(&source).expect("dir");
    fs::write(
        source.join(MANIFEST_FILE_NAME),
        "---\nname: local-only\ndescription: Local.\n---\n\n# local\n",
    )
    .expect("manifest");
    let record =
        service::import_one(&fixture.library, &fixture.storage, &source, NOW).expect("import");
    let error = fixture
        .preview(&record.id, record.revision, &FakeInstaller::new(source))
        .expect_err("no method");
    assert_eq!(error, SkillError::UpdateUnavailable);
}

#[test]
fn preview_reports_current_when_candidate_matches_library() {
    let fixture = Fixture::new();
    let record = fixture.import_git("demo", "# demo", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let upstream = fixture.root.join("upstream");
    let installer = FakeInstaller::new(upstream);
    let preview = fixture
        .preview(&record.id, record.revision, &installer)
        .expect("preview");
    assert_eq!(preview.status, SkillUpdateStatus::Current);
    assert!(preview.strategies.is_empty());
    assert_eq!(
        preview.live_content_digest,
        preview.candidate_content_digest
    );
    assert_eq!(
        preview.recorded_content_digest,
        preview.candidate_content_digest
    );
    assert!(installer.workspace_seen());
    // Workspace must be gone after the preview returns.
    assert!(
        !installer
            .workspace
            .lock()
            .expect("lock")
            .as_ref()
            .expect("path")
            .exists()
    );
}

#[test]
fn preview_detects_upstream_change_without_writing_the_library() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    let managed_before = tree_listing(fixture.library.root().join(&record.id));
    // Replace the upstream body so the candidate digest differs.
    fixture.write_skill("demo", "demo", "# updated upstream");
    let preview = fixture
        .preview(
            &record.id,
            record.revision,
            &FakeInstaller::new(fixture.root.join("upstream")),
        )
        .expect("preview");
    assert_eq!(preview.status, SkillUpdateStatus::UpstreamChanged);
    assert_eq!(
        preview.strategies,
        vec![SkillUpdateStrategy::Keep, SkillUpdateStrategy::Replace]
    );
    assert!(
        preview
            .diff
            .changed_files
            .contains(&MANIFEST_FILE_NAME.to_string())
            || preview
                .diff
                .changed_files
                .iter()
                .any(|path| path.contains("SKILL"))
    );
    assert_eq!(
        tree_listing(fixture.library.root().join(&record.id)),
        managed_before
    );
    let reloaded = crate::skills::get_skill(&fixture.storage, &record.id).expect("reload");
    assert_eq!(reloaded.content_digest, record.content_digest);
    assert_eq!(reloaded.revision, record.revision);
}

#[test]
fn preview_detects_local_modifications() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    fs::write(
        fixture
            .library
            .root()
            .join(&record.id)
            .join(MANIFEST_FILE_NAME),
        "---\nname: demo\ndescription: A remote test skill.\n---\n\n# locally edited\n",
    )
    .expect("edit");
    let preview = fixture
        .preview(
            &record.id,
            record.revision,
            &FakeInstaller::new(fixture.root.join("upstream")),
        )
        .expect("preview");
    assert_eq!(preview.status, SkillUpdateStatus::LocalModified);
    assert_eq!(preview.candidate_content_digest, record.content_digest);
    assert_ne!(preview.live_content_digest, record.content_digest);
}

#[test]
fn preview_detects_diverged_local_and_upstream() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    fs::write(
        fixture
            .library
            .root()
            .join(&record.id)
            .join(MANIFEST_FILE_NAME),
        "---\nname: demo\ndescription: A remote test skill.\n---\n\n# local\n",
    )
    .expect("edit");
    fixture.write_skill("demo", "demo", "# upstream new");
    let preview = fixture
        .preview(
            &record.id,
            record.revision,
            &FakeInstaller::new(fixture.root.join("upstream")),
        )
        .expect("preview");
    assert_eq!(preview.status, SkillUpdateStatus::Diverged);
    assert_ne!(
        preview.live_content_digest,
        preview.candidate_content_digest
    );
}

#[test]
fn preview_refuses_stale_revision_and_propagates_acquisition_errors() {
    let fixture = Fixture::new();
    let record = fixture.import_git("demo", "# demo", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let stale = fixture
        .preview(
            &record.id,
            record.revision + 1,
            &FakeInstaller::new(fixture.root.join("upstream")),
        )
        .expect_err("stale");
    assert_eq!(stale, SkillError::RevisionConflict);

    let timed_out = fixture
        .preview(
            &record.id,
            record.revision,
            &FakeInstaller::new(fixture.root.join("upstream"))
                .with_error(SkillError::InstallTimedOut),
        )
        .expect_err("timeout");
    assert_eq!(timed_out, SkillError::InstallTimedOut);
}

#[test]
fn source_from_record_requires_git_revision_and_fetches_head() {
    let fixture = Fixture::new();
    let record = fixture.import_git("demo", "# demo", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    match source_from_record(&record).expect("source") {
        crate::skills::SkillInstallSource::Git(git) => {
            assert_eq!(git.revision, "HEAD");
            assert_eq!(git.skill.as_deref(), Some("demo"));
        }
        crate::skills::SkillInstallSource::Npx(_) => panic!("expected git"),
    }
    // Simulate a broken stored row by building from a local import with only a URL.
    let local = fixture.root.join("broken");
    fs::create_dir_all(&local).expect("dir");
    fs::write(
        local.join(MANIFEST_FILE_NAME),
        "---\nname: broken\ndescription: Broken.\n---\n\n# x\n",
    )
    .expect("manifest");
    let mut broken =
        service::import_one(&fixture.library, &fixture.storage, &local, NOW).expect("import");
    broken.source_url = Some("https://github.com/example/skills".to_string());
    broken.fetch_method = Some(FetchMethod::Git);
    broken.source_revision = None;
    assert_eq!(
        source_from_record(&broken).unwrap_err(),
        SkillError::SourceRevisionRequired
    );
    let _ = delete_skill(
        &fixture.library,
        &fixture.storage,
        DeleteSkillRequest {
            id: broken.id,
            expected_revision: broken.revision,
        },
    );
}

#[test]
fn apply_keep_leaves_library_unchanged() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    fixture.write_skill("demo", "demo", "# updated upstream");
    let installer = FakeInstaller::new(fixture.root.join("upstream"));
    let preview = fixture
        .preview(&record.id, record.revision, &installer)
        .expect("preview");
    assert_eq!(preview.status, SkillUpdateStatus::UpstreamChanged);
    let managed_before = tree_listing(fixture.library.root().join(&record.id));
    let applied = fixture
        .apply(
            apply_from_preview(&preview, SkillUpdateStrategy::Keep),
            &installer,
        )
        .expect("keep");
    assert_eq!(applied.strategy, SkillUpdateStrategy::Keep);
    assert_eq!(applied.skill.revision, record.revision);
    assert_eq!(applied.skill.content_digest, record.content_digest);
    assert!(applied.deployments.is_empty());
    assert_eq!(
        tree_listing(fixture.library.root().join(&record.id)),
        managed_before
    );
}

#[test]
fn apply_replace_swaps_content_and_advances_source_revision() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    fixture.write_skill("demo", "demo", "# updated upstream");
    let new_tip = "c".repeat(40);
    let installer = FakeInstaller::new(fixture.root.join("upstream")).with_revision(&new_tip);
    let preview = fixture
        .preview(&record.id, record.revision, &installer)
        .expect("preview");
    let applied = fixture
        .apply(
            apply_from_preview(&preview, SkillUpdateStrategy::Replace),
            &installer,
        )
        .expect("replace");
    assert_eq!(applied.strategy, SkillUpdateStrategy::Replace);
    assert_eq!(applied.skill.revision, record.revision + 1);
    assert_eq!(
        applied.skill.content_digest,
        preview.candidate_content_digest
    );
    assert_eq!(
        applied.skill.source_revision.as_deref(),
        Some(new_tip.as_str())
    );
    assert_eq!(
        applied.skill.update_check_status,
        Some(SkillRevisionCheckStatus::UpToDate)
    );
    assert_eq!(
        applied.skill.remote_revision.as_deref(),
        Some(new_tip.as_str())
    );
    let live = fs::read_to_string(
        fixture
            .library
            .root()
            .join(&record.id)
            .join(MANIFEST_FILE_NAME),
    )
    .expect("manifest");
    assert!(live.contains("# updated upstream"));
}

#[test]
fn apply_refuses_stale_tokens_and_invalid_strategy() {
    let fixture = Fixture::new();
    let record = fixture.import_git(
        "demo",
        "# original",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    fixture.write_skill("demo", "demo", "# updated upstream");
    let installer = FakeInstaller::new(fixture.root.join("upstream"));
    let preview = fixture
        .preview(&record.id, record.revision, &installer)
        .expect("preview");
    let mut stale = apply_from_preview(&preview, SkillUpdateStrategy::Replace);
    stale.expected_candidate_digest = "deadbeef".to_string();
    assert_eq!(
        fixture.apply(stale, &installer).unwrap_err(),
        SkillError::UpdatePlanStale
    );

    let current = fixture.import_git(
        "other",
        "# same",
        "dddddddddddddddddddddddddddddddddddddddd",
    );
    // Upstream still matches the imported body for `other`.
    let current_installer = FakeInstaller::new(fixture.root.join("upstream"));
    // Re-write other to match its import for a Current preview.
    fixture.write_skill("other", "other", "# same");
    let current_preview = fixture
        .preview(&current.id, current.revision, &current_installer)
        .expect("current preview");
    assert_eq!(current_preview.status, SkillUpdateStatus::Current);
    assert_eq!(
        fixture
            .apply(
                apply_from_preview(&current_preview, SkillUpdateStrategy::Replace),
                &current_installer,
            )
            .unwrap_err(),
        SkillError::UpdateStrategyInvalid
    );
}

#[test]
fn revision_check_marks_update_available_and_up_to_date() {
    let fixture = Fixture::new();
    let recorded = "b".repeat(40);
    let record = fixture.import_git("demo", "# demo", &recorded);
    let newer = "c".repeat(40);
    let updated = check_revision_with(
        &fixture.storage,
        CheckSkillRevisionRequest {
            id: record.id.clone(),
        },
        NOW + 1,
        |_| Ok(newer.clone()),
    )
    .expect("check");
    assert_eq!(
        updated.update_check_status,
        Some(SkillRevisionCheckStatus::UpdateAvailable)
    );
    assert_eq!(updated.remote_revision.as_deref(), Some(newer.as_str()));
    assert_eq!(updated.last_checked_at_ms, Some(NOW + 1));
    assert_eq!(updated.revision, record.revision);

    let current = check_revision_with(
        &fixture.storage,
        CheckSkillRevisionRequest {
            id: record.id.clone(),
        },
        NOW + 2,
        |_| Ok(recorded.clone()),
    )
    .expect("recheck");
    assert_eq!(
        current.update_check_status,
        Some(SkillRevisionCheckStatus::UpToDate)
    );
    assert_eq!(current.remote_revision.as_deref(), Some(recorded.as_str()));
    assert!(current.last_check_error.is_none());
}

#[test]
fn revision_check_stores_errors_and_refuses_npx() {
    let fixture = Fixture::new();
    let record = fixture.import_git("demo", "# demo", &"b".repeat(40));
    let failed = check_revision_with(
        &fixture.storage,
        CheckSkillRevisionRequest {
            id: record.id.clone(),
        },
        NOW + 1,
        |_| Err(SkillError::InstallTimedOut),
    )
    .expect("stored error");
    assert_eq!(
        failed.update_check_status,
        Some(SkillRevisionCheckStatus::Error)
    );
    assert_eq!(
        failed.last_check_error.as_deref(),
        Some("install_timed_out")
    );
    assert!(failed.remote_revision.is_none());

    let local = fixture.root.join("npx-skill");
    fs::create_dir_all(&local).expect("dir");
    fs::write(
        local.join(MANIFEST_FILE_NAME),
        "---\nname: npx-skill\ndescription: Npx.\n---\n\n# npx\n",
    )
    .expect("manifest");
    let provenance = Provenance {
        origin_path: None,
        source_url: Some("https://github.com/example/skills"),
        source_revision: None,
        fetch_method: Some(FetchMethod::Npx),
        installer_version: Some("1.7.4"),
        installer_command: Some(
            "npx --yes skills@1.7.4 add https://github.com/example/skills --skill '*' --copy -g -a claude-code -y",
        ),
    };
    let npx =
        service::import_installed(&fixture.library, &fixture.storage, &local, &provenance, NOW)
            .expect("npx import");
    let error = check_revision_with(
        &fixture.storage,
        CheckSkillRevisionRequest { id: npx.id },
        NOW + 1,
        |_| Ok("a".repeat(40)),
    )
    .expect_err("npx");
    assert_eq!(error, SkillError::UpdateUnavailable);
}

fn tree_listing(root: PathBuf) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
        for entry in fs::read_dir(dir).expect("read") {
            let entry = entry.expect("entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let meta = fs::metadata(entry.path()).expect("meta");
            if meta.is_dir() {
                walk(&entry.path(), &relative, out);
            } else {
                out.push((relative, meta.len()));
            }
        }
    }
    walk(&root, "", &mut out);
    out.sort();
    out
}
