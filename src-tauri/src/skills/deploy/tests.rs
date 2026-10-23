//! Temporary-directory tests for Pi skill deploy: symlink, copy-sync, conflicts.

use super::{
    apply_pi_skill_deploy, disable_pi_skill_deploy, list_pi_skill_deployments,
    preview_pi_skill_deploy,
};
use crate::agents::DiscoveryEnvironment;
use crate::skills::manifest::MANIFEST_FILE_NAME;
use crate::skills::service::import_one;
use crate::skills::types::{
    DeleteSkillRequest, DisablePiSkillDeployRequest, ReviewedPiSkillDeploy, SkillDeployMode,
    SkillDeployTargetState, SkillError,
};
use crate::skills::{SkillLibrary, delete_skill};
use crate::storage::Storage;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_700_000_000_000;

struct Fixture {
    root: PathBuf,
    storage: Storage,
    library: SkillLibrary,
    home: PathBuf,
    pi_root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-skill-deploy-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let storage = Storage::open_in_directory(&root).expect("storage");
        let library = SkillLibrary::new(root.join("skills"));
        let home = root.join("home");
        let pi_root = home.join(".pi/agent");
        fs::create_dir_all(&pi_root).expect("pi root");
        Self {
            root,
            storage,
            library,
            home,
            pi_root,
        }
    }

    fn environment(&self) -> DiscoveryEnvironment {
        DiscoveryEnvironment {
            home: self.home.clone(),
            ..DiscoveryEnvironment::default()
        }
    }

    fn import(&self, name: &str) -> crate::skills::SkillRecord {
        let source = self.root.join("sources").join(name);
        fs::create_dir_all(&source).expect("source");
        fs::write(
            source.join(MANIFEST_FILE_NAME),
            format!("---\nname: {name}\ndescription: A test skill.\n---\n\n# {name}\n"),
        )
        .expect("manifest");
        import_one(&self.library, &self.storage, &source, NOW).expect("import")
    }

    fn preview(&self, skill_id: &str) -> Result<crate::skills::PiSkillDeployPreview, SkillError> {
        preview_pi_skill_deploy(&self.library, &self.storage, &self.environment(), skill_id)
    }

    fn apply(
        &self,
        reviewed: ReviewedPiSkillDeploy,
    ) -> Result<crate::skills::PiSkillDeployed, SkillError> {
        apply_pi_skill_deploy(
            &self.library,
            &self.storage,
            &self.environment(),
            reviewed,
            NOW,
        )
    }

    fn disable(
        &self,
        skill_id: &str,
        config_root: &str,
    ) -> Result<crate::skills::PiSkillDeployDisabled, SkillError> {
        disable_pi_skill_deploy(
            &self.library,
            &self.storage,
            &self.environment(),
            DisablePiSkillDeployRequest {
                skill_id: skill_id.to_string(),
                config_root: config_root.to_string(),
            },
        )
    }

    fn target(&self, name: &str) -> PathBuf {
        self.pi_root.join("skills").join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn reviewed_from_preview(
    preview: &crate::skills::PiSkillDeployPreview,
    mode: SkillDeployMode,
) -> ReviewedPiSkillDeploy {
    ReviewedPiSkillDeploy {
        skill_id: preview.skill_id.clone(),
        expected_skill_revision: preview.skill_revision,
        config_root: preview.config_root.clone(),
        mode,
        target_state: preview.target_state,
    }
}

#[test]
fn symlink_deploy_creates_owned_link_and_records_revision() {
    let fixture = Fixture::new();
    let record = fixture.import("demo-skill");
    let preview = fixture.preview(&record.id).expect("preview");
    assert_eq!(preview.target_state, SkillDeployTargetState::Absent);
    assert!(preview.can_apply);
    assert!(preview.symlink_available);
    assert_eq!(preview.suggested_mode, SkillDeployMode::Symlink);

    let applied = fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect("apply");
    assert_eq!(applied.skill_revision, record.revision);
    assert_eq!(applied.mode, SkillDeployMode::Symlink);

    let target = fixture.target("demo-skill");
    let meta = fs::symlink_metadata(&target).expect("target meta");
    assert!(meta.file_type().is_symlink());
    let resolved = fs::canonicalize(&target).expect("canonicalize");
    let managed = fs::canonicalize(fixture.library.root().join(&record.id)).expect("managed");
    assert_eq!(resolved, managed);

    let listed =
        list_pi_skill_deployments(&fixture.library, &fixture.storage, &fixture.environment())
            .expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].in_sync);
    assert_eq!(listed[0].skill_revision, record.revision);
}

#[test]
fn copy_deploy_writes_independent_tree() {
    let fixture = Fixture::new();
    let record = fixture.import("copy-skill");
    let preview = fixture.preview(&record.id).expect("preview");
    let applied = fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Copy))
        .expect("apply");
    assert_eq!(applied.mode, SkillDeployMode::Copy);

    let target = fixture.target("copy-skill");
    let meta = fs::symlink_metadata(&target).expect("meta");
    assert!(meta.is_dir());
    assert!(!meta.file_type().is_symlink());
    assert!(target.join(MANIFEST_FILE_NAME).is_file());
    // Deleting the managed copy later must not remove the deployed files; for
    // this test, only prove the trees are distinct paths.
    assert_ne!(
        fs::canonicalize(&target).unwrap(),
        fs::canonicalize(fixture.library.root().join(&record.id)).unwrap()
    );
}

#[test]
fn foreign_target_is_previewed_but_not_applied() {
    let fixture = Fixture::new();
    let record = fixture.import("taken");
    let target = fixture.target("taken");
    fs::create_dir_all(&target).expect("foreign");
    fs::write(target.join(MANIFEST_FILE_NAME), "user content").expect("file");

    let preview = fixture.preview(&record.id).expect("preview");
    assert_eq!(preview.target_state, SkillDeployTargetState::Foreign);
    assert!(!preview.can_apply);

    let error = fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect_err("foreign refused");
    assert_eq!(error, SkillError::DeployModeInvalid);
    assert!(Path::new(&target.join(MANIFEST_FILE_NAME)).exists());
}

#[test]
fn disable_removes_owned_symlink_only() {
    let fixture = Fixture::new();
    let record = fixture.import("revoke-me");
    let preview = fixture.preview(&record.id).expect("preview");
    fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect("apply");

    // User-owned neighbour must survive.
    let neighbour = fixture.target("user-own");
    fs::create_dir_all(&neighbour).expect("neighbour");
    fs::write(neighbour.join("keep.txt"), "keep").expect("keep");

    let disabled = fixture
        .disable(&record.id, &preview.config_root)
        .expect("disable");
    assert!(disabled.target_removed);
    assert!(!disabled.left_foreign);
    assert!(!fixture.target("revoke-me").exists());
    assert!(neighbour.join("keep.txt").exists());
    assert!(fixture.library.root().join(&record.id).exists());

    let listed =
        list_pi_skill_deployments(&fixture.library, &fixture.storage, &fixture.environment())
            .expect("list");
    assert!(listed.is_empty());
}

#[test]
fn disable_leaves_replaced_foreign_content() {
    let fixture = Fixture::new();
    let record = fixture.import("replaced");
    let preview = fixture.preview(&record.id).expect("preview");
    fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect("apply");

    let target = fixture.target("replaced");
    fs::remove_file(&target)
        .or_else(|_| fs::remove_dir_all(&target))
        .expect("remove link");
    fs::create_dir_all(&target).expect("user dir");
    fs::write(target.join("user.md"), "mine").expect("user file");

    let disabled = fixture
        .disable(&record.id, &preview.config_root)
        .expect("disable");
    assert!(!disabled.target_removed);
    assert!(disabled.left_foreign);
    assert!(target.join("user.md").exists());
}

#[test]
fn stale_target_state_is_rejected() {
    let fixture = Fixture::new();
    let record = fixture.import("stale");
    let preview = fixture.preview(&record.id).expect("preview");
    // Create a foreign path after the preview claimed absent.
    fs::create_dir_all(fixture.target("stale")).expect("foreign");
    let error = fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect_err("stale");
    assert_eq!(error, SkillError::TargetChanged);
}

#[test]
fn forged_revision_is_rejected() {
    let fixture = Fixture::new();
    let record = fixture.import("forged");
    let preview = fixture.preview(&record.id).expect("preview");
    let mut reviewed = reviewed_from_preview(&preview, SkillDeployMode::Symlink);
    reviewed.expected_skill_revision = record.revision + 1;
    let error = fixture.apply(reviewed).expect_err("forged revision");
    assert_eq!(error, SkillError::RevisionConflict);
}

#[test]
fn delete_skill_refuses_while_deployed() {
    let fixture = Fixture::new();
    let record = fixture.import("in-use");
    let preview = fixture.preview(&record.id).expect("preview");
    fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect("apply");

    let error = delete_skill(
        &fixture.library,
        &fixture.storage,
        DeleteSkillRequest {
            id: record.id.clone(),
            expected_revision: record.revision,
        },
    )
    .expect_err("in use");
    assert_eq!(error, SkillError::SkillInUse);
    assert!(fixture.library.root().join(&record.id).exists());

    fixture
        .disable(&record.id, &preview.config_root)
        .expect("disable");
    delete_skill(
        &fixture.library,
        &fixture.storage,
        DeleteSkillRequest {
            id: record.id.clone(),
            expected_revision: record.revision,
        },
    )
    .expect("delete after disable");
}

#[test]
fn redeploy_updates_owned_target() {
    let fixture = Fixture::new();
    let record = fixture.import("again");
    let preview = fixture.preview(&record.id).expect("preview");
    fixture
        .apply(reviewed_from_preview(&preview, SkillDeployMode::Symlink))
        .expect("first");

    let again = fixture.preview(&record.id).expect("second preview");
    assert_eq!(again.target_state, SkillDeployTargetState::OwnedSymlink);
    fixture
        .apply(reviewed_from_preview(&again, SkillDeployMode::Copy))
        .expect("switch to copy");
    let meta = fs::symlink_metadata(fixture.target("again")).expect("meta");
    assert!(meta.is_dir());
    assert!(!meta.file_type().is_symlink());
}
