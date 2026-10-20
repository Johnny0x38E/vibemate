//! Central skill library facade. A skill is a directory with a `SKILL.md` file;
//! this module owns the managed copy, its content digest and its source record.
//! Importing never runs a skill command and never changes the source directory,
//! and deleting removes only the managed copy and its record.

mod install;
mod manifest;
mod repository;
mod service;
mod types;
mod validation;

pub use install::{install_skill, preview_skill_install};
pub use repository::{get_skill, list_skills};
pub use service::{cleanup_deleted_skill_copies, delete_skill, import_folder, set_skill_source};
pub use types::{
    DeleteSkillRequest, DeletedSkill, FetchMethod, GitInstallSource, ImportFolderReport,
    ImportFolderRequest, NpxInstallSource, PickedFolder, SetSkillSourceRequest, SkillError,
    SkillInstallPreview, SkillInstallReport, SkillInstallSource, SkillPage, SkillRecord,
    SkippedSkill,
};

use std::path::PathBuf;

#[cfg(test)]
mod tests;

/// Root directory that holds the managed copies, `<app data>/skills/<id>`.
///
/// The library keeps its own copy of every skill so later Agent deployment can
/// link to a stable path while the user's original directory stays untouched.
#[derive(Debug)]
pub struct SkillLibrary {
    root: PathBuf,
}

impl SkillLibrary {
    /// Build a library inside the private application data directory.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Directory that contains one managed copy per skill ID.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}
