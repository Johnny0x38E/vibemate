//! Behaviour tests for remote skill acquisition and import provenance.

use super::*;
use crate::skills::{SkillLibrary, SkillPage, service};
use crate::storage::Storage;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_700_000_000_000;

/// Temporary database, library and acquired-source directories.
struct Fixture {
    root: PathBuf,
    storage: Storage,
    library: SkillLibrary,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-skill-install-{}-{}",
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

    fn source_root(&self) -> PathBuf {
        self.root.join("source")
    }

    fn write_skill(&self, folder: &str, name: &str) -> PathBuf {
        let directory = self.source_root().join("skills").join(folder);
        fs::create_dir_all(&directory).expect("skill directory");
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: A remote test skill.\n---\n\n# {name}\n"),
        )
        .expect("manifest");
        directory
    }

    fn install(
        &self,
        request: SkillInstallSource,
        installer: &dyn SkillInstaller,
    ) -> Result<SkillInstallReport, SkillError> {
        install_with(&self.library, &self.storage, request, installer, NOW)
    }

    fn page(&self) -> SkillPage {
        crate::skills::list_skills(&self.storage, None, 100).expect("list")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Copy files and folders without following links; the test source is trusted.
fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target directory");
    for entry in fs::read_dir(source).expect("source directory") {
        let entry = entry.expect("entry");
        let destination = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(entry.path()).expect("metadata");
        if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(
                fs::read_link(entry.path()).expect("link target"),
                destination,
            )
            .expect("copy link");
        } else if metadata.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy file");
        }
    }
}

/// An acquisition step that copies a fixture tree and reports a fixed revision.
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
            revision: None,
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

    fn workspace(&self) -> PathBuf {
        self.workspace
            .lock()
            .expect("workspace lock")
            .clone()
            .expect("workspace recorded")
    }
}

impl SkillInstaller for FakeInstaller {
    fn acquire(&self, _: &ResolvedSource, workspace: &Path) -> Result<Acquired, SkillError> {
        *self.workspace.lock().expect("workspace lock") = Some(workspace.to_path_buf());
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

fn git_source(revision: &str, skill: Option<&str>) -> SkillInstallSource {
    SkillInstallSource::Git(GitInstallSource {
        repository: "example/skills".to_string(),
        revision: revision.to_string(),
        skill: skill.map(str::to_string),
    })
}

fn npx_source(version: Option<&str>, skill: Option<&str>) -> SkillInstallSource {
    SkillInstallSource::Npx(NpxInstallSource {
        source: "example/skills".to_string(),
        installer_version: version.map(str::to_string),
        skill: skill.map(str::to_string),
    })
}

#[test]
fn preview_git_canonicalizes_the_repository_and_shows_the_command() {
    let preview = preview_skill_install(git_source("v1.2.0", None)).expect("preview");
    assert_eq!(preview.method, FetchMethod::Git);
    assert_eq!(preview.source_url, "https://github.com/example/skills");
    assert_eq!(preview.revision.as_deref(), Some("v1.2.0"));
    assert_eq!(preview.installer, "git");
    assert_eq!(preview.installer_version, None);
    assert_eq!(
        preview.command,
        "git fetch --depth 1 https://github.com/example/skills v1.2.0 && git checkout --detach FETCH_HEAD"
    );
}

#[test]
fn preview_payload_matches_the_frontend_contract() {
    let preview = preview_skill_install(npx_source(None, None)).expect("preview");
    let json = serde_json::to_value(&preview).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({
            "method": "npx",
            "sourceUrl": "https://github.com/example/skills",
            "revision": null,
            "installer": "skills",
            "installerVersion": "1.7.4",
            "command": "npx --yes skills@1.7.4 add https://github.com/example/skills --skill '*' --copy -g -a claude-code -y",
        })
    );
}

#[test]
fn preview_npx_uses_the_reviewed_installer_and_selector() {
    let preview = preview_skill_install(npx_source(None, Some("alpha"))).expect("preview");
    assert_eq!(preview.method, FetchMethod::Npx);
    assert_eq!(preview.source_url, "https://github.com/example/skills");
    assert_eq!(preview.revision, None);
    assert_eq!(preview.installer, "skills");
    assert_eq!(preview.installer_version.as_deref(), Some("1.7.4"));
    assert_eq!(
        preview.command,
        "npx --yes skills@1.7.4 add https://github.com/example/skills --skill alpha --copy -g -a claude-code -y"
    );

    // A blank selector means every installed skill.
    let all = preview_skill_install(npx_source(Some("1.7.4"), None)).expect("preview");
    assert!(all.command.contains("--skill '*'"))
}

#[test]
fn unsupported_sources_and_versions_are_refused_before_any_work() {
    let cases = [
        SkillInstallSource::Git(GitInstallSource {
            repository: "https://user:token@github.com/example/skills".to_string(),
            revision: "v1".to_string(),
            skill: None,
        }),
        SkillInstallSource::Git(GitInstallSource {
            repository: "https://example.com/example/skills".to_string(),
            revision: "v1".to_string(),
            skill: None,
        }),
        SkillInstallSource::Git(GitInstallSource {
            repository: "example/skills".to_string(),
            revision: "--upload-pack=evil".to_string(),
            skill: None,
        }),
        SkillInstallSource::Git(GitInstallSource {
            repository: "example/skills".to_string(),
            revision: "main..evil".to_string(),
            skill: None,
        }),
        SkillInstallSource::Git(GitInstallSource {
            repository: "example/skills".to_string(),
            revision: "v1;rm".to_string(),
            skill: None,
        }),
        SkillInstallSource::Npx(NpxInstallSource {
            source: "example/skills".to_string(),
            installer_version: Some("^1.7.4".to_string()),
            skill: None,
        }),
        SkillInstallSource::Npx(NpxInstallSource {
            source: "example/skills".to_string(),
            installer_version: Some("1.7.3".to_string()),
            skill: None,
        }),
        SkillInstallSource::Npx(NpxInstallSource {
            source: "example/skills".to_string(),
            installer_version: None,
            skill: Some("../escape".to_string()),
        }),
    ];
    for source in cases {
        assert!(preview_skill_install(source).is_err());
    }
}

#[test]
fn git_install_records_the_resolved_commit_and_removes_the_workspace() {
    let fixture = Fixture::new();
    fixture.write_skill("alpha", "alpha-skill");
    fixture.write_skill("beta", "beta-skill");
    let commit = "a".repeat(40);
    let installer = FakeInstaller::new(fixture.source_root()).with_revision(&commit);
    let report = fixture
        .install(git_source("v1", None), &installer)
        .expect("install");

    assert_eq!(report.method, FetchMethod::Git);
    assert_eq!(report.source_url, "https://github.com/example/skills");
    assert_eq!(report.revision.as_deref(), Some(commit.as_str()));
    assert_eq!(report.installer, "git");
    assert_eq!(report.installer_version, None);
    assert_eq!(report.exit_status, 0);
    assert_eq!(report.imported.len(), 2);
    assert!(report.skipped.is_empty());
    assert_eq!(report.imported[0].origin_path, None);
    assert_eq!(report.imported[0].fetch_method, Some(FetchMethod::Git));
    assert_eq!(
        report.imported[0].source_revision.as_deref(),
        Some(commit.as_str())
    );
    assert!(
        report.imported[0]
            .installer_command
            .as_deref()
            .is_some_and(|command| command.starts_with("git fetch --depth 1"))
    );
    assert_eq!(fixture.page().total, 2);
    // The private acquisition tree is gone; the managed copies stay.
    assert!(!installer.workspace().exists());
}

#[test]
fn npx_install_records_installer_version_and_filters_one_skill() {
    let fixture = Fixture::new();
    fixture.write_skill("alpha", "alpha-skill");
    fixture.write_skill("beta", "beta-skill");
    let installer = FakeInstaller::new(fixture.source_root());
    let report = fixture
        .install(npx_source(None, Some("beta-skill")), &installer)
        .expect("install");

    assert_eq!(report.method, FetchMethod::Npx);
    assert_eq!(report.installer, "skills");
    assert_eq!(report.installer_version.as_deref(), Some("1.7.4"));
    assert_eq!(report.revision, None);
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].name, "beta-skill");
    assert_eq!(report.imported[0].fetch_method, Some(FetchMethod::Npx));
    assert_eq!(report.imported[0].source_revision, None);
    assert_eq!(
        report.imported[0].installer_version.as_deref(),
        Some("1.7.4")
    );
    assert!(
        report.imported[0]
            .installer_command
            .as_deref()
            .is_some_and(|command| command.contains("skills@1.7.4"))
    );
    assert!(!installer.workspace().exists());
}

#[test]
fn a_missing_selector_is_reported_without_importing_anything() {
    let fixture = Fixture::new();
    fixture.write_skill("alpha", "alpha-skill");
    let installer = FakeInstaller::new(fixture.source_root());
    assert_eq!(
        fixture
            .install(npx_source(None, Some("missing")), &installer)
            .unwrap_err(),
        SkillError::SkillNotFoundInSource
    );
    assert_eq!(fixture.page().total, 0);
    assert!(!installer.workspace().exists());
}

#[test]
fn one_refused_candidate_does_not_hide_the_others() {
    let fixture = Fixture::new();
    fixture.write_skill("alpha", "alpha-skill");
    fixture.write_skill("beta", "beta-skill");
    // A local skill owns the first name, so the remote alpha is refused.
    let local = fixture.root.join("local").join("alpha");
    fs::create_dir_all(&local).expect("local source");
    fs::write(
        local.join("SKILL.md"),
        "---\nname: alpha-skill\ndescription: A local skill.\n---\n",
    )
    .expect("manifest");
    service::import_one(&fixture.library, &fixture.storage, &local, NOW).expect("local");

    let installer = FakeInstaller::new(fixture.source_root());
    let report = fixture
        .install(npx_source(None, None), &installer)
        .expect("install");
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].name, "beta-skill");
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].code, SkillError::NameTaken);
    assert_eq!(fixture.page().total, 2);
}

#[test]
fn a_failed_acquisition_leaves_the_library_unchanged() {
    let fixture = Fixture::new();
    fixture.write_skill("alpha", "alpha-skill");
    let installer =
        FakeInstaller::new(fixture.source_root()).with_error(SkillError::InstallerFailed);
    assert_eq!(
        fixture
            .install(npx_source(None, None), &installer)
            .unwrap_err(),
        SkillError::InstallerFailed
    );
    assert_eq!(fixture.page().total, 0);
    assert!(!installer.workspace().exists());
}

#[test]
fn a_source_without_skills_is_refused() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.source_root()).expect("empty source");
    let installer = FakeInstaller::new(fixture.source_root());
    assert_eq!(
        fixture
            .install(npx_source(None, None), &installer)
            .unwrap_err(),
        SkillError::NoSkillsFound
    );
}

#[cfg(unix)]
#[test]
fn linked_candidates_are_skipped_instead_of_followed() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let real = fixture.write_skill("real", "real-skill");
    let acquired = fixture.source_root().join("skills");
    symlink(&real, acquired.join("linked")).expect("symlink");
    let installer = FakeInstaller::new(fixture.source_root());
    let report = fixture
        .install(npx_source(None, None), &installer)
        .expect("install");
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].name, "real-skill");
    assert!(report.skipped.is_empty());

    // A source made only of links has nothing safe to import.
    let only_links = Fixture::new();
    fs::create_dir_all(only_links.source_root().join("skills")).expect("skills");
    symlink(&real, only_links.source_root().join("skills/linked")).expect("symlink");
    let installer = FakeInstaller::new(only_links.source_root());
    assert_eq!(
        only_links
            .install(npx_source(None, None), &installer)
            .unwrap_err(),
        SkillError::NoSkillsFound
    );
}

#[test]
#[ignore = "network: runs the reviewed skills npx installer against a public source"]
fn real_npx_install_uses_the_reviewed_installer() {
    let fixture = Fixture::new();
    let report = fixture
        .install(
            SkillInstallSource::Npx(NpxInstallSource {
                source: "vercel-labs/agent-skills".to_string(),
                installer_version: None,
                skill: Some("vercel-composition-patterns".to_string()),
            }),
            &SystemSkillInstaller,
        )
        .expect("real npx install");
    assert_eq!(report.method, FetchMethod::Npx);
    assert_eq!(report.installer, "skills");
    assert_eq!(report.installer_version.as_deref(), Some("1.7.4"));
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].name, "vercel-composition-patterns");
    assert_eq!(
        report.imported[0].source_url.as_deref(),
        Some("https://github.com/vercel-labs/agent-skills")
    );
    assert_eq!(fixture.page().total, 1);
}

#[test]
#[ignore = "network: fetches a pinned public Git revision"]
fn real_git_install_records_the_pinned_commit() {
    let fixture = Fixture::new();
    let commit = "063bee94c3f4df8453406c830b0a7df0f2860278";
    let report = fixture
        .install(
            SkillInstallSource::Git(GitInstallSource {
                repository: "https://github.com/vercel-labs/agent-skills".to_string(),
                revision: commit.to_string(),
                skill: Some("vercel-composition-patterns".to_string()),
            }),
            &SystemSkillInstaller,
        )
        .expect("real git install");
    assert_eq!(report.method, FetchMethod::Git);
    assert_eq!(report.installer, "git");
    assert_eq!(report.revision.as_deref(), Some(commit));
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].name, "vercel-composition-patterns");
    assert_eq!(report.imported[0].source_revision.as_deref(), Some(commit));
    assert_eq!(fixture.page().total, 1);
}

#[test]
#[ignore = "network: checks a failed Git fetch leaves the library unchanged"]
fn real_failed_git_fetch_leaves_the_library_unchanged() {
    let fixture = Fixture::new();
    let error = fixture
        .install(
            SkillInstallSource::Git(GitInstallSource {
                repository: "https://github.com/vercel-labs/agent-skills".to_string(),
                revision: "0000000000000000000000000000000000000000".to_string(),
                skill: None,
            }),
            &SystemSkillInstaller,
        )
        .unwrap_err();
    assert_eq!(error, SkillError::InstallerFailed);
    assert_eq!(fixture.page().total, 0);
}

#[test]
fn reviewed_source_fields_build_the_same_update_metadata() {
    let (revision, version, command) = reviewed_source_fields(
        FetchMethod::Git,
        "https://github.com/a/b",
        Some("v2"),
        None,
        "x",
    )
    .expect("git fields");
    assert_eq!(revision.as_deref(), Some("v2"));
    assert_eq!(version, None);
    assert!(command.contains("git fetch --depth 1 https://github.com/a/b v2"));

    let (revision, version, command) = reviewed_source_fields(
        FetchMethod::Npx,
        "https://github.com/a/b",
        None,
        None,
        "demo",
    )
    .expect("npx fields");
    assert_eq!(revision, None);
    assert_eq!(version.as_deref(), Some("1.7.4"));
    assert!(command.contains("--skill demo"));

    assert_eq!(
        reviewed_source_fields(FetchMethod::Git, "https://github.com/a/b", None, None, "x")
            .unwrap_err(),
        SkillError::SourceRevisionRequired
    );
    assert_eq!(
        reviewed_source_fields(
            FetchMethod::Npx,
            "https://github.com/a/b",
            Some("v2"),
            None,
            "x"
        )
        .unwrap_err(),
        SkillError::InvalidRequest
    );
}
