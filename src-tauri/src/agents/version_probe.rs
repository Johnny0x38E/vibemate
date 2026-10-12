//! User-confirmed CLI version execution, separate from static discovery.
//! Limits protect this request, not the filesystem/network or descendant processes.

use super::{AgentId, DiscoveryEnvironment, DiscoveryInput, EntryState, discover_agents};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, sync::Semaphore};

const RUN_LIMIT: Duration = Duration::from_secs(5);
const OUTPUT_LIMIT: usize = 8 * 1024;

/// A fixed version request for the exact entry and target approved in the UI.
/// No command arguments, shell strings or caller-controlled environment are accepted.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionProbeInput {
    /// Agent whose documented version command is used.
    pub id: AgentId,
    /// Observed absolute entry; checked again before execution.
    pub entry_path: String,
    /// Resolved target displayed during confirmation; changes reject execution.
    pub resolved_path: String,
    /// Explicit acknowledgment; this is not cryptographic proof of human consent.
    pub confirmed: bool,
}

/// Version returned by the executed program, not proof of authenticity or capability.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionResult {
    /// Agent identity from the confirmed request.
    pub id: AgentId,
    /// Rechecked entry path.
    pub entry_path: String,
    /// Rechecked regular-file target used for execution.
    pub resolved_path: String,
    /// Bounded version token only; raw output is discarded.
    pub version: String,
    /// Fixed provenance, kept separate from discovery and capability evidence.
    pub evidence: CliVersionEvidence,
}

/// Evidence for a version reported by a program, not an installation certificate.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CliVersionEvidence {
    /// Parsed from a successful user-triggered version command.
    CliOutput,
}

/// Sanitized failures; paths, process errors and output are never serialized here.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionProbeError {
    /// The caller did not acknowledge process execution.
    ConfirmationRequired,
    /// Another request already holds the application-wide probe permit.
    Busy,
    /// Entry input is unsafe, absent, non-regular or not executable.
    EntryUnavailable,
    /// The resolved target changed after the displayed observation.
    EntryChanged,
    /// Windows wrapping would require shell execution; only .exe is accepted.
    UnsupportedEntry,
    /// Process startup, I/O or cleanup failed; internal diagnostics are discarded.
    OperationFailed,
    /// Deadline expired; termination of the direct child was requested.
    TimedOut,
    /// A pipe exceeded its bounded byte allowance.
    OutputTooLarge,
    /// The command exited unsuccessfully, irrespective of its output.
    CommandFailed,
    /// Output was not one of the known safe version formats.
    UnrecognizedOutput,
}

/// One application-wide permit: requests fail busy rather than queue launches.
pub struct VersionProbeGate(Semaphore);

impl Default for VersionProbeGate {
    fn default() -> Self {
        Self(Semaphore::new(1))
    }
}

struct ScratchDirectory(PathBuf);
impl ScratchDirectory {
    fn create() -> Result<Self, VersionProbeError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..16 {
            let path = std::env::temp_dir().join(format!(
                "vibemate-version-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            if !path.is_absolute() {
                return Err(VersionProbeError::OperationFailed);
            }
            let builder = fs::DirBuilder::new();
            // Only Unix permission setup needs mutation; Windows keeps an immutable builder.
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = builder;
                builder.mode(0o700);
                builder
            };
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(VersionProbeError::OperationFailed),
            }
        }
        Err(VersionProbeError::OperationFailed)
    }
}
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        // Never recursively remove files written by the program or its descendants.
        // A nonempty scratch directory remains for inspection, not silent deletion.
        let _ = fs::remove_dir(&self.0);
    }
}

struct PreparedProbe {
    input: VersionProbeInput,
    scratch: ScratchDirectory,
}

fn windows_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

fn prepare(
    environment: DiscoveryEnvironment,
    input: VersionProbeInput,
) -> Result<PreparedProbe, VersionProbeError> {
    if !input.confirmed {
        return Err(VersionProbeError::ConfirmationRequired);
    }
    let mut discovery = DiscoveryInput::default();
    let path = Some(input.entry_path.clone());
    match input.id {
        AgentId::ClaudeCode => discovery.claude_path = path,
        AgentId::Codex => discovery.codex_path = path,
        AgentId::OpenCode => discovery.opencode_path = path,
        AgentId::Pi => discovery.pi_path = path,
        AgentId::GrokBuild => discovery.grok_path = path,
        AgentId::Hermes => discovery.hermes_path = path,
    }
    let row = discover_agents(&environment, &discovery)
        .map_err(|_| VersionProbeError::EntryUnavailable)?
        .into_iter()
        .find(|row| row.id == input.id)
        .ok_or(VersionProbeError::EntryUnavailable)?;
    if row.state != EntryState::Found {
        return Err(VersionProbeError::EntryUnavailable);
    }
    if row.resolved_path.as_deref() != Some(&input.resolved_path)
        || row.entry_path.as_deref() != Some(&input.entry_path)
    {
        return Err(VersionProbeError::EntryChanged);
    }
    if cfg!(windows)
        && (!windows_executable(Path::new(&input.entry_path))
            || !windows_executable(Path::new(&input.resolved_path)))
    {
        return Err(VersionProbeError::UnsupportedEntry);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(&input.resolved_path)
            .map_err(|_| VersionProbeError::EntryUnavailable)?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err(VersionProbeError::EntryUnavailable);
        }
    }
    Ok(PreparedProbe {
        input,
        scratch: ScratchDirectory::create()?,
    })
}

async fn bounded_output(
    mut stream: impl tokio::io::AsyncRead + Unpin,
) -> Result<Vec<u8>, VersionProbeError> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|_| VersionProbeError::OperationFailed)?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len() + count > OUTPUT_LIMIT {
            return Err(VersionProbeError::OutputTooLarge);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}

fn safe_version(value: &str) -> bool {
    if value.len() > 64 || !value.is_ascii() {
        return false;
    }
    let (base, build) = value
        .split_once('+')
        .map_or((value, None), |(base, build)| (base, Some(build)));
    let (core, prerelease) = base
        .split_once('-')
        .map_or((base, None), |(core, pre)| (core, Some(pre)));
    let parts: Vec<_> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 10 && part.bytes().all(|b| b.is_ascii_digit())
        })
        && [prerelease, build].into_iter().flatten().all(|part| {
            !part.is_empty()
                && part.split('.').all(|piece| {
                    !piece.is_empty()
                        && piece
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
        })
}

fn parse_version(id: AgentId, output: &[u8]) -> Result<String, VersionProbeError> {
    let text = std::str::from_utf8(output).map_err(|_| VersionProbeError::UnrecognizedOutput)?;
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or(VersionProbeError::UnrecognizedOutput)?
        .trim();
    let version = match id {
        AgentId::ClaudeCode => line.strip_suffix(" (Claude Code)").unwrap_or(line),
        AgentId::Codex => line
            .strip_prefix("codex-cli ")
            .or_else(|| line.strip_prefix("codex "))
            .ok_or(VersionProbeError::UnrecognizedOutput)?,
        AgentId::OpenCode | AgentId::Pi => line,
        AgentId::GrokBuild => {
            let line = line.strip_prefix("grok ").unwrap_or(line);
            match line.split_once(" (") {
                Some((version, commit))
                    if commit.strip_suffix(')').is_some_and(|hash| {
                        !hash.is_empty()
                            && hash.len() <= 64
                            && hash.bytes().all(|b| b.is_ascii_hexdigit())
                    }) =>
                {
                    version
                }
                _ => line,
            }
        }
        AgentId::Hermes => line
            .strip_prefix("Hermes Agent v")
            .and_then(|rest| rest.split_ascii_whitespace().next())
            .ok_or(VersionProbeError::UnrecognizedOutput)?,
    };
    if !safe_version(version) {
        return Err(VersionProbeError::UnrecognizedOutput);
    }
    Ok(version.to_owned())
}

async fn execute(
    prepared: &PreparedProbe,
    deadline: Duration,
) -> Result<String, VersionProbeError> {
    let mut command = Command::new(&prepared.input.resolved_path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&prepared.scratch.0)
        .env_clear()
        .kill_on_drop(true);
    // Avoid forwarding credentials and interpreter injection (NODE_OPTIONS,
    // BASH_ENV, PYTHONPATH, preload variables). This is not a sandbox: HOME and
    // config roots still allow the program to find its own user files.
    for name in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "LOCALAPPDATA",
        "APPDATA",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "OPENCODE_CONFIG_DIR",
        "XDG_CONFIG_HOME",
        "PI_CODING_AGENT_DIR",
        "GROK_HOME",
        "HERMES_HOME",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("TERM", "dumb");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW avoids a console flash.
    let mut child = command
        .spawn()
        .map_err(|_| VersionProbeError::OperationFailed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(VersionProbeError::OperationFailed)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(VersionProbeError::OperationFailed)?;
    let outcome = tokio::time::timeout(deadline, async {
        tokio::try_join!(
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| VersionProbeError::OperationFailed)
            },
            bounded_output(stdout),
            bounded_output(stderr),
        )
    })
    .await;
    let result = outcome.unwrap_or(Err(VersionProbeError::TimedOut));
    let (status, stdout, _stderr) = match result {
        Ok(output) => output,
        Err(error) => {
            // kill also waits/reaps. Bound cleanup; kill_on_drop remains a
            // best-effort fallback if cleanup fails or this future is cancelled.
            let _ = tokio::time::timeout(Duration::from_secs(1), child.kill()).await;
            return Err(error);
        }
    };
    if !status.success() {
        return Err(VersionProbeError::CommandFailed);
    }
    parse_version(prepared.input.id, &stdout)
}

/// Run a confirmed, fixed CLI version probe under the application's single permit.
/// Filesystem preparation uses a blocking task; only bounded version data returns.
/// Errors are sanitized. The selected program can still access files/network and
/// spawn descendants; termination targets the direct child, not a process tree.
pub async fn probe_agent_version(
    environment: DiscoveryEnvironment,
    input: VersionProbeInput,
    gate: &VersionProbeGate,
) -> Result<AgentVersionResult, VersionProbeError> {
    let _permit = gate.0.try_acquire().map_err(|_| VersionProbeError::Busy)?;
    let prepared = tokio::task::spawn_blocking(move || prepare(environment, input))
        .await
        .map_err(|_| VersionProbeError::OperationFailed)??;
    let version = execute(&prepared, RUN_LIMIT).await?;
    Ok(AgentVersionResult {
        id: prepared.input.id,
        entry_path: prepared.input.entry_path.clone(),
        resolved_path: prepared.input.resolved_path.clone(),
        version,
        evidence: CliVersionEvidence::CliOutput,
    })
}

#[cfg(test)]
#[path = "version_probe_tests.rs"]
mod tests;
