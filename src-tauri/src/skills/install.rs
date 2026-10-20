//! Install a skill from a reviewed GitHub source.
//!
//! Two acquisition paths are supported:
//!
//! - a pinned Git revision fetched with the installed `git` executable;
//! - the allowlisted `skills` npx installer at an exact package version.
//!
//! The caller shows [`preview_skill_install`] before running anything. The
//! install then acquires the source in a private temporary workspace, discovers
//! bounded `SKILL.md` folders, and copies each one through the same manifest and
//! digest path as a local import. The workspace is removed on every outcome.
//!
//! This module never runs a command from a skill. The only external programs are
//! the reviewed installer and Git, both with a cleared environment, a bounded
//! time limit, and arguments built from validated values rather than a shell.

use super::SkillLibrary;
use super::manifest;
use super::service::{self, Provenance};
use super::types::{
    FetchMethod, GitInstallSource, NpxInstallSource, SkillError, SkillInstallPreview,
    SkillInstallReport, SkillInstallSource, SkippedSkill,
};
use super::validation;
use crate::storage::Storage;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The only npx package P26 allows. The name is fixed in code; the interface
/// cannot substitute another installer.
pub(super) const SKILLS_INSTALLER: &str = "skills";
/// The installer version this build reviewed against its documentation.
pub(super) const SKILLS_INSTALLER_VERSION: &str = "1.7.4";
/// A Git fetch is bounded so a stalled network cannot block a worker forever.
const GIT_TIMEOUT: Duration = Duration::from_secs(120);
/// npm may need to download the installer and its dependencies.
const NPX_TIMEOUT: Duration = Duration::from_secs(300);
/// One fetched source may contribute at most this many candidates.
const MAX_REMOTE_CANDIDATES: usize = 64;
/// How deep the bounded `SKILL.md` search descends below the acquired root.
const MAX_REMOTE_DEPTH: usize = 6;
/// How much process output is retained for failure classification. The text is
/// never sent to the interface or written to a log.
const MAX_PROCESS_OUTPUT: usize = 64 * 1024;

/// A validated acquisition source with its reviewed command.
pub(super) enum ResolvedSource {
    Git {
        url: String,
        revision: String,
        skill: Option<String>,
    },
    Npx {
        url: String,
        installer_version: String,
        skill: Option<String>,
    },
}

impl ResolvedSource {
    fn method(&self) -> FetchMethod {
        match self {
            Self::Git { .. } => FetchMethod::Git,
            Self::Npx { .. } => FetchMethod::Npx,
        }
    }

    fn url(&self) -> &str {
        match self {
            Self::Git { url, .. } | Self::Npx { url, .. } => url,
        }
    }

    fn installer(&self) -> &'static str {
        match self {
            Self::Git { .. } => "git",
            Self::Npx { .. } => SKILLS_INSTALLER,
        }
    }

    fn installer_version(&self) -> Option<&str> {
        match self {
            Self::Git { .. } => None,
            Self::Npx {
                installer_version, ..
            } => Some(installer_version),
        }
    }

    fn skill(&self) -> Option<&str> {
        match self {
            Self::Git { skill, .. } | Self::Npx { skill, .. } => skill.as_deref(),
        }
    }

    fn command(&self) -> Result<String, SkillError> {
        let command = match self {
            Self::Git { url, revision, .. } => {
                format!("git fetch --depth 1 {url} {revision} && git checkout --detach FETCH_HEAD")
            }
            Self::Npx {
                url,
                installer_version,
                skill,
            } => npx_command(url, installer_version, skill.as_deref().unwrap_or("*")),
        };
        validation::installer_command(&command).map(str::to_string)
    }

    fn preview(&self) -> Result<SkillInstallPreview, SkillError> {
        Ok(SkillInstallPreview {
            method: self.method(),
            source_url: self.url().to_string(),
            revision: match self {
                Self::Git { revision, .. } => Some(revision.clone()),
                Self::Npx { .. } => None,
            },
            installer: self.installer().to_string(),
            installer_version: self.installer_version().map(str::to_string),
            command: self.command()?,
        })
    }
}

/// Validate a source and build the command the user must review.
///
/// No network request, process or file write happens here.
pub fn preview_skill_install(
    request: SkillInstallSource,
) -> Result<SkillInstallPreview, SkillError> {
    resolve_source(&request)?.preview()
}

/// Acquire a source and copy every discovered skill into the central library.
///
/// # Errors
/// Rejects an invalid source, a missing Git/npx executable, a failed or timed
/// out installer, and a source with no matching skill. A failure leaves the
/// library unchanged; a candidate that cannot be imported is reported in
/// `skipped` while the others are kept.
pub fn install_skill(
    library: &SkillLibrary,
    storage: &Storage,
    request: SkillInstallSource,
    now_ms: i64,
) -> Result<SkillInstallReport, SkillError> {
    install_with(library, storage, request, &SystemSkillInstaller, now_ms)
}

/// Test seam: the acquisition step can be replaced without a network or tools.
fn install_with(
    library: &SkillLibrary,
    storage: &Storage,
    request: SkillInstallSource,
    installer: &dyn SkillInstaller,
    now_ms: i64,
) -> Result<SkillInstallReport, SkillError> {
    if now_ms < 0 {
        return Err(SkillError::InvalidRequest);
    }
    let source = resolve_source(&request)?;
    let command = source.command()?;
    let workspace = Workspace::new()?;
    let acquired = installer.acquire(&source, workspace.path())?;
    let candidates = discover_remote(&acquired.root, source.skill())?;
    if candidates.is_empty() {
        return Err(SkillError::NoSkillsFound);
    }
    let revision = match &source {
        ResolvedSource::Git { revision, .. } => Some(
            acquired
                .revision
                .clone()
                .unwrap_or_else(|| revision.clone()),
        ),
        ResolvedSource::Npx { .. } => None,
    };
    let provenance = Provenance {
        origin_path: None,
        source_url: Some(source.url()),
        source_revision: revision.as_deref(),
        fetch_method: Some(source.method()),
        installer_version: source.installer_version(),
        installer_command: Some(&command),
    };
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        match service::import_installed(library, storage, &candidate, &provenance, now_ms) {
            Ok(record) => imported.push(record),
            Err(code) => skipped.push(SkippedSkill {
                folder: validation::folder_label(&candidate),
                code,
            }),
        }
    }
    Ok(SkillInstallReport {
        method: source.method(),
        source_url: source.url().to_string(),
        revision,
        installer: source.installer().to_string(),
        installer_version: source.installer_version().map(str::to_string),
        command,
        exit_status: acquired.exit_status,
        imported,
        skipped,
    })
}

/// Build the exact npx command used for both preview and execution.
///
/// `*` is quoted in the display text so copying it into a shell does not expand
/// it; the process receives the unquoted argument.
fn npx_command(url: &str, version: &str, selector: &str) -> String {
    let display_selector = if selector == "*" {
        "'*'".to_string()
    } else {
        selector.to_string()
    };
    format!(
        "npx --yes {SKILLS_INSTALLER}@{version} add {url} --skill {display_selector} --copy -g -a claude-code -y"
    )
}

/// Validate a fetch method and generate the fields a source record needs.
///
/// The source editor uses this so a hand-entered address can gain a method for
/// P29 without re-running the installer. The returned command is display text;
/// it is later rebuilt from the same validated fields by the installer.
pub(super) fn reviewed_source_fields(
    method: FetchMethod,
    url: &str,
    revision: Option<&str>,
    installer_version: Option<&str>,
    skill_name: &str,
) -> Result<(Option<String>, Option<String>, String), SkillError> {
    match method {
        FetchMethod::Git => {
            let revision = revision.ok_or(SkillError::SourceRevisionRequired)?;
            let revision = validation::git_revision(revision)?;
            if installer_version.is_some() {
                return Err(SkillError::InvalidRequest);
            }
            let command =
                format!("git fetch --depth 1 {url} {revision} && git checkout --detach FETCH_HEAD");
            let command = validation::installer_command(&command)?;
            Ok((Some(revision.to_string()), None, command.to_string()))
        }
        FetchMethod::Npx => {
            // The npx installer resolves its own source; a Git revision would not
            // be applied and must not be recorded as if it were.
            if revision.is_some() {
                return Err(SkillError::InvalidRequest);
            }
            let version = installer_version.unwrap_or(SKILLS_INSTALLER_VERSION);
            let version = validation::installer_version(version)?;
            // Only the version this build reviewed may run. A different package
            // version would execute code nobody in this release verified.
            if version != SKILLS_INSTALLER_VERSION {
                return Err(SkillError::InstallerVersionInvalid);
            }
            let selector = validation::skill_selector(Some(skill_name))?.unwrap_or("*");
            let command = npx_command(url, version, selector);
            let command = validation::installer_command(&command)?;
            Ok((None, Some(version.to_string()), command.to_string()))
        }
    }
}

/// Validate a request into the values execution needs.
fn resolve_source(request: &SkillInstallSource) -> Result<ResolvedSource, SkillError> {
    match request {
        SkillInstallSource::Git(GitInstallSource {
            repository,
            revision,
            skill,
        }) => Ok(ResolvedSource::Git {
            url: validation::github_repository(repository)?,
            revision: validation::git_revision(revision)?.to_string(),
            skill: validation::skill_selector(skill.as_deref())?.map(str::to_string),
        }),
        SkillInstallSource::Npx(NpxInstallSource {
            source,
            installer_version,
            skill,
        }) => {
            let version = installer_version
                .as_deref()
                .unwrap_or(SKILLS_INSTALLER_VERSION);
            let version = validation::installer_version(version)?;
            // The package name is fixed and only the reviewed version may run.
            if version != SKILLS_INSTALLER_VERSION {
                return Err(SkillError::InstallerVersionInvalid);
            }
            Ok(ResolvedSource::Npx {
                url: validation::github_repository(source)?,
                installer_version: version.to_string(),
                skill: validation::skill_selector(skill.as_deref())?.map(str::to_string),
            })
        }
    }
}

/// What one acquisition step produced.
struct Acquired {
    /// Directory that contains the acquired skill folders.
    root: PathBuf,
    /// Resolved Git commit, or `None` for the npx installer.
    revision: Option<String>,
    /// Exit code of the successful installer.
    exit_status: i32,
}

/// The acquisition step, replaceable in tests.
trait SkillInstaller {
    fn acquire(&self, source: &ResolvedSource, workspace: &Path) -> Result<Acquired, SkillError>;
}

/// Production acquisition through Git or the reviewed npx installer.
pub struct SystemSkillInstaller;

impl SkillInstaller for SystemSkillInstaller {
    fn acquire(&self, source: &ResolvedSource, workspace: &Path) -> Result<Acquired, SkillError> {
        match source {
            ResolvedSource::Git {
                url,
                revision,
                skill: _,
            } => acquire_git(workspace, url, revision),
            ResolvedSource::Npx {
                url,
                installer_version,
                skill,
            } => acquire_npx(workspace, url, installer_version, skill.as_deref()),
        }
    }
}

/// Fetch a pinned revision with Git and return the checkout root.
fn acquire_git(workspace: &Path, url: &str, revision: &str) -> Result<Acquired, SkillError> {
    let home = workspace.join("home");
    let repository = workspace.join("repository");
    let empty_config = workspace.join("gitconfig");
    fs::create_dir_all(&home).map_err(|_| SkillError::InstallWorkspaceFailed)?;
    fs::write(&empty_config, []).map_err(|_| SkillError::InstallWorkspaceFailed)?;
    fs::create_dir(&repository).map_err(|_| SkillError::InstallWorkspaceFailed)?;

    let mut environment = base_environment();
    environment.push(("HOME".into(), home.as_os_str().to_owned()));
    environment.push(("USERPROFILE".into(), home.as_os_str().to_owned()));
    environment.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    environment.push((
        "GIT_CONFIG_GLOBAL".into(),
        empty_config.as_os_str().to_owned(),
    ));
    environment.push(("GIT_TERMINAL_PROMPT".into(), "0".into()));
    environment.push(("GIT_LFS_SKIP_SMUDGE".into(), "1".into()));
    environment.push(("GIT_OPTIONAL_LOCKS".into(), "0".into()));

    let run = |args: &[&str]| -> Result<ProcessOutput, SkillError> {
        run_bounded(
            OsStr::new("git"),
            &args.iter().map(OsString::from).collect::<Vec<_>>(),
            &repository,
            &environment,
            GIT_TIMEOUT,
        )
    };
    run(&["init", "--quiet"])?;
    run(&["remote", "add", "origin", url])?;
    run(&[
        "fetch",
        "--quiet",
        "--depth",
        "1",
        "--no-tags",
        "origin",
        revision,
    ])?;
    run(&["checkout", "--quiet", "--detach", "FETCH_HEAD"])?;
    // A submodule checkout can fetch arbitrary nested repositories. P26 records
    // one source and refuses that extra dependency instead of silently missing it.
    if fs::symlink_metadata(repository.join(".gitmodules")).is_ok() {
        return Err(SkillError::SourceUnsupportedEntry);
    }
    let head = run(&["rev-parse", "HEAD"])?;
    let commit = String::from_utf8(head.stdout).map_err(|_| SkillError::InstallerOutputInvalid)?;
    let commit = commit.trim();
    let commit_is_sha =
        (40..=64).contains(&commit.len()) && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !commit_is_sha {
        return Err(SkillError::InstallerOutputInvalid);
    }
    Ok(Acquired {
        root: repository,
        revision: Some(commit.to_ascii_lowercase()),
        exit_status: 0,
    })
}

/// Run the allowlisted npx installer in an isolated home, then return its
/// canonical installed-skill directory.
fn acquire_npx(
    workspace: &Path,
    url: &str,
    version: &str,
    skill: Option<&str>,
) -> Result<Acquired, SkillError> {
    let home = workspace.join("home");
    let work = workspace.join("work");
    let cache = workspace.join("npm-cache");
    fs::create_dir_all(&home).map_err(|_| SkillError::InstallWorkspaceFailed)?;
    fs::create_dir_all(&work).map_err(|_| SkillError::InstallWorkspaceFailed)?;
    fs::create_dir_all(&cache).map_err(|_| SkillError::InstallWorkspaceFailed)?;

    let mut environment = base_environment();
    environment.push(("HOME".into(), home.as_os_str().to_owned()));
    environment.push(("USERPROFILE".into(), home.as_os_str().to_owned()));
    environment.push((
        "XDG_CONFIG_HOME".into(),
        home.join(".config").into_os_string(),
    ));
    environment.push(("npm_config_cache".into(), cache.into_os_string()));
    environment.push(("DISABLE_TELEMETRY".into(), "1".into()));
    environment.push(("DO_NOT_TRACK".into(), "1".into()));
    environment.push(("NO_COLOR".into(), "1".into()));
    environment.push(("CI".into(), "1".into()));

    let selector = skill.unwrap_or("*");
    let mut arguments = vec![
        OsString::from("--yes"),
        OsString::from(format!("{SKILLS_INSTALLER}@{version}")),
        OsString::from("add"),
        OsString::from(url),
        OsString::from("--skill"),
        OsString::from(selector),
        OsString::from("--copy"),
        OsString::from("-g"),
        OsString::from("-a"),
        OsString::from("claude-code"),
        OsString::from("-y"),
    ];
    // `npx` is a batch file on Windows. Running it through `cmd /C` keeps the
    // same validated arguments without asking the shell to parse user input.
    let program = if cfg!(windows) {
        arguments.insert(0, OsString::from("npx"));
        arguments.insert(0, OsString::from("/C"));
        OsString::from("cmd")
    } else {
        OsString::from("npx")
    };
    run_bounded(
        program.as_os_str(),
        &arguments,
        &work,
        &environment,
        NPX_TIMEOUT,
    )?;

    for relative in [".agents/skills", ".claude/skills"] {
        let root = home.join(relative);
        if !root.is_dir() {
            continue;
        }
        if !manifest::discover(&root)?.is_empty() {
            return Ok(Acquired {
                root,
                revision: None,
                exit_status: 0,
            });
        }
    }
    Err(SkillError::NoSkillsFound)
}

/// Environment retained for external tools. Everything else, including
/// `NODE_OPTIONS` and Git credential helpers, is cleared.
fn base_environment() -> Vec<(OsString, OsString)> {
    let mut environment = Vec::new();
    for name in [
        "PATH",
        "PATHEXT",
        "TEMP",
        "TMP",
        "SystemRoot",
        "WINDIR",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
        "all_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NODE_EXTRA_CA_CERTS",
    ] {
        if let Some(value) = std::env::var_os(name) {
            environment.push((OsString::from(name), value));
        }
    }
    environment
}

/// Output retained from one external command.
struct ProcessOutput {
    stdout: Vec<u8>,
}

/// Run one external command with a cleared environment and a hard deadline.
///
/// stdout and stderr are drained on separate threads so a noisy child cannot
/// block on a full pipe. Only a bounded prefix is retained.
fn run_bounded(
    program: &OsStr,
    arguments: &[OsString],
    directory: &Path,
    environment: &[(OsString, OsString)],
    timeout: Duration,
) -> Result<ProcessOutput, SkillError> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .current_dir(directory)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // Avoid a console window when a desktop command starts a console tool.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| SkillError::InstallerUnavailable)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(SkillError::InstallerOutputInvalid)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(SkillError::InstallerOutputInvalid)?;
    let stdout_reader = std::thread::spawn(move || read_bounded(stdout));
    let stderr_reader = std::thread::spawn(move || read_bounded(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                // Do not wait for the readers here: a killed child's descendants
                // may still hold a pipe, and the timeout must return promptly.
                let _ = child.kill();
                let _ = child.wait();
                return Err(SkillError::InstallTimedOut);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SkillError::InstallerFailed);
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| SkillError::InstallerOutputInvalid)?;
    let _stderr = stderr_reader
        .join()
        .map_err(|_| SkillError::InstallerOutputInvalid)?;
    let exit_status = status.code().ok_or(SkillError::InstallerFailed)?;
    if exit_status != 0 {
        return Err(SkillError::InstallerFailed);
    }
    Ok(ProcessOutput { stdout })
}

/// Read at most [`MAX_PROCESS_OUTPUT`] bytes, then drain the rest.
fn read_bounded(mut reader: impl Read) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                if kept.len() < MAX_PROCESS_OUTPUT {
                    let remaining = MAX_PROCESS_OUTPUT - kept.len();
                    kept.extend_from_slice(&buffer[..count.min(remaining)]);
                }
            }
        }
    }
    kept
}

/// Find bounded skill folders below an acquired root.
fn discover_remote(root: &Path, selector: Option<&str>) -> Result<Vec<PathBuf>, SkillError> {
    let mut candidates = Vec::new();
    collect_candidates(root, 0, &mut candidates)?;
    candidates.sort();
    candidates.dedup();
    let Some(selector) = selector.filter(|value| *value != "*") else {
        return Ok(candidates);
    };
    let candidates: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|path| candidate_matches(path, selector))
        .collect();
    if candidates.is_empty() {
        return Err(SkillError::SkillNotFoundInSource);
    }
    Ok(candidates)
}

/// Walk real directories only. A folder with `SKILL.md` is one candidate and is
/// not searched further, so bundled examples cannot become separate skills.
fn collect_candidates(
    directory: &Path,
    depth: usize,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), SkillError> {
    if depth > MAX_REMOTE_DEPTH {
        return Err(SkillError::SourceTooLarge);
    }
    let metadata = fs::symlink_metadata(directory).map_err(|_| SkillError::SourceNotFound)?;
    // The acquired root itself must be a real directory. Links below it are
    // skipped rather than followed: a repository may contain unrelated links,
    // and following one could leave the acquired tree.
    if metadata.file_type().is_symlink() {
        return Err(SkillError::SourceUnsupportedEntry);
    }
    if !metadata.is_dir() {
        return Err(SkillError::SourceNotDirectory);
    }
    if manifest::is_skill(directory) {
        if candidates.len() >= MAX_REMOTE_CANDIDATES {
            return Err(SkillError::SourceTooLarge);
        }
        candidates.push(directory.to_path_buf());
        return Ok(());
    }
    for entry in fs::read_dir(directory).map_err(|_| SkillError::SourceNotFound)? {
        let entry = entry.map_err(|_| SkillError::SourceNotFound)?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or(SkillError::SourceUnsupportedEntry)?
            .to_string();
        // Hidden folders are agent/tool bookkeeping, not skills.
        if name.starts_with('.') {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| SkillError::SourceNotFound)?;
        // A linked file or directory is not part of the acquired tree. Skip it;
        // a link inside a real skill is still refused by manifest inspection.
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_candidates(&entry.path(), depth + 1, candidates)?;
        }
    }
    Ok(())
}

/// Whether a candidate answers the reviewed one-skill selector.
fn candidate_matches(path: &Path, selector: &str) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name == selector)
        || manifest::inspect(path).is_ok_and(|plan| plan.name == selector)
}

/// A private temporary directory removed on every exit path.
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new() -> Result<Self, SkillError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        for _ in 0..8 {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            let root = base.join(format!(
                "vibemate-skill-install-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(SkillError::InstallWorkspaceFailed),
            }
        }
        Err(SkillError::InstallWorkspaceFailed)
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests;
