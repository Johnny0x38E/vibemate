//! Preview and apply updating one central skill from its recorded fetch method,
//! plus a lightweight Git revision check that only runs `ls-remote`.
//!
//! Only skills with an explicit Git or npx fetch method can be digest-previewed
//! or applied. The digest preview re-acquires into a temporary workspace,
//! digests the live managed copy and the matching candidate, then reports
//! status and strategies. Git update acquisition always fetches `HEAD` so a tip
//! move is visible; the recorded pin remains the digest baseline. Preview never
//! writes the library or any Agent skills directory.
//!
//! Apply re-acquires, re-checks the digest tokens the interface reviewed, then
//! either leaves the library unchanged (`keep`) or replaces the managed copy and
//! refreshes owned deployments (`replace`). A partial deploy refresh does not
//! roll back the central library change.
//!
//! The revision check is Git-only: it resolves remote `HEAD`, compares it to
//! the recorded `source_revision`, and stores the outcome without downloading
//! the tree. npx-recorded skills stay on the digest-preview path.

#[cfg(test)]
mod tests;

use super::SkillLibrary;
use super::deploy;
use super::install::{
    SkillInstaller, SystemSkillInstaller, Workspace, discover_remote, ls_remote_head,
    resolve_source, reviewed_source_fields,
};
use super::manifest;
use super::repository;
use super::service::{self, digest_tree};
use super::types::{
    ApplySkillUpdateRequest, CheckSkillRevisionRequest, FetchMethod, GitInstallSource,
    NpxInstallSource, PreviewSkillUpdateRequest, SkillError, SkillInstallSource, SkillRecord,
    SkillRevisionCheckStatus, SkillUpdateApplied, SkillUpdateDiff, SkillUpdatePreview,
    SkillUpdateStatus, SkillUpdateStrategy,
};
use super::validation;
use crate::storage::Storage;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// How many path names each diff list may carry across IPC.
const MAX_DIFF_PATHS: usize = 64;

/// Compare one Git skill's recorded revision to remote `HEAD` via `ls-remote`.
///
/// Network work runs before the database write. The skill metadata `revision`
/// is not bumped. npx and method-less sources are refused.
///
/// # Errors
/// Rejects non-Git sources, missing identities, and storage write failures. A
/// network failure is stored as `update_check_status = error` and still returns
/// the updated record when the row can be written.
pub fn check_skill_revision(
    storage: &Storage,
    request: CheckSkillRevisionRequest,
    now_ms: i64,
) -> Result<SkillRecord, SkillError> {
    check_revision_with(storage, request, now_ms, ls_remote_head)
}

/// Test seam: replace remote resolution without contacting the network.
pub(super) fn check_revision_with(
    storage: &Storage,
    request: CheckSkillRevisionRequest,
    now_ms: i64,
    resolve: impl FnOnce(&str) -> Result<String, SkillError>,
) -> Result<SkillRecord, SkillError> {
    validation::identity(&request.id)?;
    if now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    let record = repository::get_skill(storage, &request.id)?;
    if record.fetch_method != Some(FetchMethod::Git) {
        return Err(SkillError::UpdateUnavailable);
    }
    let url = record
        .source_url
        .as_deref()
        .ok_or(SkillError::UpdateUnavailable)?;
    let recorded = record
        .source_revision
        .as_deref()
        .ok_or(SkillError::SourceRevisionRequired)?;
    match resolve(url) {
        Ok(remote) => {
            let status = if remote.eq_ignore_ascii_case(recorded) {
                SkillRevisionCheckStatus::UpToDate
            } else {
                SkillRevisionCheckStatus::UpdateAvailable
            };
            repository::store_revision_check(
                storage,
                &request.id,
                Some(&remote),
                status,
                None,
                now_ms,
            )?;
        }
        Err(error) => {
            let code = skill_error_code(error);
            repository::store_revision_check(
                storage,
                &request.id,
                None,
                SkillRevisionCheckStatus::Error,
                Some(&code),
                now_ms,
            )?;
        }
    }
    repository::get_skill(storage, &request.id)
}

fn skill_error_code(error: SkillError) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "operation_failed".to_string())
}

/// Preview an update using the skill's recorded Git or npx source.
///
/// # Errors
/// Rejects skills without a fetch method, a stale library revision, a missing
/// managed copy, acquisition failures, and a source that no longer contains the
/// skill. On every outcome the temporary workspace is removed and the library
/// is left unchanged.
pub fn preview_skill_update(
    library: &SkillLibrary,
    storage: &Storage,
    request: PreviewSkillUpdateRequest,
) -> Result<SkillUpdatePreview, SkillError> {
    preview_with(library, storage, request, &SystemSkillInstaller)
}

/// Test seam: replace acquisition without contacting the network.
pub(super) fn preview_with(
    library: &SkillLibrary,
    storage: &Storage,
    request: PreviewSkillUpdateRequest,
    installer: &dyn SkillInstaller,
) -> Result<SkillUpdatePreview, SkillError> {
    validation::identity(&request.id)?;
    if request.expected_revision < 1 {
        return Err(SkillError::InvalidRequest);
    }
    let record = super::repository::get_skill(storage, &request.id)?;
    if record.revision != request.expected_revision {
        return Err(SkillError::RevisionConflict);
    }
    let install_source = source_from_record(&record)?;
    let source = resolve_source(&install_source)?;
    let command = source.command()?;
    let workspace = Workspace::new()?;
    let acquired = installer.acquire(&source, workspace.path())?;
    let candidates = discover_remote(&acquired.root, source.skill())?;
    let candidate_path = candidates
        .into_iter()
        .next()
        .ok_or(SkillError::SkillNotFoundInSource)?;
    let candidate_plan = manifest::inspect(&candidate_path)?;
    if candidate_plan.name != record.name {
        return Err(SkillError::SkillNotFoundInSource);
    }
    let candidate_digest = digest_tree(&candidate_path, &candidate_plan)?;
    let managed = library.root().join(&record.id);
    let live_plan = manifest::inspect(&managed)?;
    let live_digest = digest_tree(&managed, &live_plan)?;
    let status = classify_status(&record.content_digest, &live_digest, &candidate_digest);
    let strategies = strategies_for(status);
    let diff = build_diff(&live_plan, &managed, &candidate_plan, &candidate_path)?;
    Ok(SkillUpdatePreview {
        skill_id: record.id,
        skill_name: record.name,
        skill_revision: record.revision,
        fetch_method: source.method(),
        source_url: source.url().to_string(),
        command,
        recorded_source_revision: record.source_revision,
        candidate_source_revision: acquired.revision,
        recorded_content_digest: record.content_digest,
        live_content_digest: live_digest,
        candidate_content_digest: candidate_digest,
        candidate_file_count: candidate_plan.files.len() as i64,
        candidate_byte_size: candidate_plan.total_bytes as i64,
        status,
        strategies,
        diff,
    })
}

/// Apply a reviewed keep/replace strategy after a digest preview.
///
/// Re-acquires the candidate and refuses when digests, status or the candidate
/// revision no longer match the tokens the interface reviewed. `keep` never
/// writes. `replace` swaps the managed copy, updates source metadata, then
/// refreshes owned Agent deployments; a per-target deploy failure is reported
/// without rolling back the library.
///
/// # Errors
/// Rejects a stale library revision, a strategy not offered for the live
/// status, mismatched preview tokens, acquisition failures, and replace write
/// failures.
pub fn apply_skill_update(
    library: &SkillLibrary,
    storage: &Storage,
    request: ApplySkillUpdateRequest,
    now_ms: i64,
) -> Result<SkillUpdateApplied, SkillError> {
    apply_with(library, storage, request, now_ms, &SystemSkillInstaller)
}

/// Test seam: replace acquisition without contacting the network.
pub(super) fn apply_with(
    library: &SkillLibrary,
    storage: &Storage,
    request: ApplySkillUpdateRequest,
    now_ms: i64,
    installer: &dyn SkillInstaller,
) -> Result<SkillUpdateApplied, SkillError> {
    validation::identity(&request.id)?;
    if request.expected_revision < 1 || now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    if request.expected_recorded_digest.is_empty()
        || request.expected_live_digest.is_empty()
        || request.expected_candidate_digest.is_empty()
    {
        return Err(SkillError::InvalidRequest);
    }
    let record = repository::get_skill(storage, &request.id)?;
    if record.revision != request.expected_revision {
        return Err(SkillError::RevisionConflict);
    }
    let install_source = source_from_record(&record)?;
    let source = resolve_source(&install_source)?;
    let workspace = Workspace::new()?;
    let acquired = installer.acquire(&source, workspace.path())?;
    let candidates = discover_remote(&acquired.root, source.skill())?;
    let candidate_path = candidates
        .into_iter()
        .next()
        .ok_or(SkillError::SkillNotFoundInSource)?;
    let candidate_plan = manifest::inspect(&candidate_path)?;
    if candidate_plan.name != record.name {
        return Err(SkillError::SkillNotFoundInSource);
    }
    let candidate_digest = digest_tree(&candidate_path, &candidate_plan)?;
    let managed = library.root().join(&record.id);
    let live_plan = manifest::inspect(&managed)?;
    let live_digest = digest_tree(&managed, &live_plan)?;
    let status = classify_status(&record.content_digest, &live_digest, &candidate_digest);
    if status != request.expected_status
        || record.content_digest != request.expected_recorded_digest
        || live_digest != request.expected_live_digest
        || candidate_digest != request.expected_candidate_digest
        || acquired.revision.as_deref() != request.expected_candidate_source_revision.as_deref()
    {
        return Err(SkillError::UpdatePlanStale);
    }
    let allowed = strategies_for(status);
    if !allowed.contains(&request.strategy) {
        return Err(SkillError::UpdateStrategyInvalid);
    }
    match request.strategy {
        SkillUpdateStrategy::Keep => Ok(SkillUpdateApplied {
            skill: record,
            strategy: SkillUpdateStrategy::Keep,
            status,
            deployments: Vec::new(),
        }),
        SkillUpdateStrategy::Replace => {
            let (source_revision, installer_command) = match source.method() {
                FetchMethod::Git => {
                    let revision = acquired
                        .revision
                        .as_deref()
                        .ok_or(SkillError::InstallerOutputInvalid)?;
                    let (_, _, command) = reviewed_source_fields(
                        FetchMethod::Git,
                        source.url(),
                        Some(revision),
                        None,
                        &record.name,
                    )?;
                    (Some(revision.to_string()), Some(command))
                }
                FetchMethod::Npx => (None, record.installer_command.clone()),
            };
            let skill = service::replace_managed_content(
                library,
                storage,
                &service::ManagedReplace {
                    id: &request.id,
                    expected_revision: request.expected_revision,
                    candidate: &candidate_path,
                    source_revision: source_revision.as_deref(),
                    installer_command: installer_command.as_deref(),
                    now_ms,
                },
            )?;
            let deployments =
                deploy::refresh_skill_deployments_after_update(library, storage, &skill, now_ms)?;
            Ok(SkillUpdateApplied {
                skill,
                strategy: SkillUpdateStrategy::Replace,
                status,
                deployments,
            })
        }
    }
}

/// Rebuild the acquisition request an update preview or apply uses.
///
/// Git always fetches `HEAD` so tip movement is visible; the recorded pin stays
/// the digest baseline. npx reuses the reviewed installer version.
pub(super) fn source_from_record(record: &SkillRecord) -> Result<SkillInstallSource, SkillError> {
    let url = record
        .source_url
        .as_deref()
        .ok_or(SkillError::UpdateUnavailable)?;
    let method = record.fetch_method.ok_or(SkillError::UpdateUnavailable)?;
    match method {
        FetchMethod::Git => {
            // A recorded pin proves Git provenance even though the candidate is tip.
            let _ = record
                .source_revision
                .as_deref()
                .ok_or(SkillError::SourceRevisionRequired)?;
            Ok(SkillInstallSource::Git(GitInstallSource {
                repository: url.to_string(),
                revision: "HEAD".to_string(),
                skill: Some(record.name.clone()),
            }))
        }
        FetchMethod::Npx => Ok(SkillInstallSource::Npx(NpxInstallSource {
            source: url.to_string(),
            installer_version: record.installer_version.clone(),
            skill: Some(record.name.clone()),
        })),
    }
}

fn classify_status(recorded: &str, live: &str, candidate: &str) -> SkillUpdateStatus {
    let local_modified = live != recorded;
    let upstream_changed = candidate != recorded;
    match (local_modified, upstream_changed) {
        (false, false) => SkillUpdateStatus::Current,
        (true, false) => SkillUpdateStatus::LocalModified,
        (false, true) => SkillUpdateStatus::UpstreamChanged,
        (true, true) => SkillUpdateStatus::Diverged,
    }
}

fn strategies_for(status: SkillUpdateStatus) -> Vec<SkillUpdateStrategy> {
    match status {
        SkillUpdateStatus::Current => Vec::new(),
        SkillUpdateStatus::LocalModified
        | SkillUpdateStatus::UpstreamChanged
        | SkillUpdateStatus::Diverged => {
            vec![SkillUpdateStrategy::Keep, SkillUpdateStrategy::Replace]
        }
    }
}

fn build_diff(
    live_plan: &manifest::SourcePlan,
    managed: &Path,
    candidate_plan: &manifest::SourcePlan,
    candidate: &Path,
) -> Result<SkillUpdateDiff, SkillError> {
    let live_digests = file_digests(managed, live_plan)?;
    let candidate_digests = file_digests(candidate, candidate_plan)?;
    let live_paths: BTreeSet<&str> = live_digests.keys().map(String::as_str).collect();
    let candidate_paths: BTreeSet<&str> = candidate_digests.keys().map(String::as_str).collect();
    let mut added = Vec::new();
    for path in candidate_paths.difference(&live_paths) {
        if added.len() >= MAX_DIFF_PATHS {
            break;
        }
        added.push((*path).to_string());
    }
    let mut removed = Vec::new();
    for path in live_paths.difference(&candidate_paths) {
        if removed.len() >= MAX_DIFF_PATHS {
            break;
        }
        removed.push((*path).to_string());
    }
    let mut changed = Vec::new();
    for path in live_paths.intersection(&candidate_paths) {
        if changed.len() >= MAX_DIFF_PATHS {
            break;
        }
        if live_digests.get(*path) != candidate_digests.get(*path) {
            changed.push((*path).to_string());
        }
    }
    Ok(SkillUpdateDiff {
        added_files: added,
        removed_files: removed,
        changed_files: changed,
        description_changed: live_plan.description != candidate_plan.description,
        live_description: live_plan.description.clone(),
        candidate_description: candidate_plan.description.clone(),
    })
}

fn file_digests(
    root: &Path,
    plan: &manifest::SourcePlan,
) -> Result<BTreeMap<String, String>, SkillError> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeMap::new();
    for file in &plan.files {
        let bytes =
            std::fs::read(root.join(&file.relative)).map_err(|_| SkillError::SourceNotFound)?;
        if bytes.len() as u64 != file.size {
            return Err(SkillError::SourceChanged);
        }
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        out.insert(file.relative.clone(), format!("{:x}", hasher.finalize()));
    }
    Ok(out)
}
