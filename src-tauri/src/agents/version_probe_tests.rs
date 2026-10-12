//! Isolated process fixtures: never launch an installed Agent or read its files.

use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-version-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn environment(&self) -> DiscoveryEnvironment {
        DiscoveryEnvironment {
            home: self.0.clone(),
            ..DiscoveryEnvironment::default()
        }
    }
    fn input(&self, id: AgentId, entry: &Path) -> VersionProbeInput {
        VersionProbeInput {
            id,
            entry_path: entry.to_str().unwrap().to_owned(),
            resolved_path: fs::canonicalize(entry)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            confirmed: true,
        }
    }
    #[cfg(unix)]
    fn script(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.0.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn version_probe_parses_only_agent_specific_version_tokens() {
    for (id, text, version) in [
        (AgentId::ClaudeCode, "2.1.119 (Claude Code)\n", "2.1.119"),
        (AgentId::Codex, "codex-cli 0.98.0\n", "0.98.0"),
        (AgentId::OpenCode, "1.2.28\n", "1.2.28"),
        (AgentId::Pi, "1.1.0\n", "1.1.0"),
        (AgentId::GrokBuild, "1.0.50 (c58f321264ba)\n", "1.0.50"),
        (
            AgentId::Hermes,
            "Hermes Agent v0.13.0 (2026-10-01) · upstream abc123\nInstall directory: /synthetic/private\nUpdate available\n",
            "0.13.0",
        ),
        (
            AgentId::Hermes,
            "Hermes Agent v0.13.0-dev.1+abc · stable\n",
            "0.13.0-dev.1+abc",
        ),
    ] {
        assert_eq!(parse_version(id, text.as_bytes()).unwrap(), version);
    }
    for text in [
        "private-error",
        "token=1.2.3",
        "1.2.3 private-token",
        "\x1b[31m1.2.3",
        "1.2.3+",
        "1.2.3-",
        "1.2.3+../secret",
        "1.2.3\0",
        "1.2.3/credential",
    ] {
        assert_eq!(
            parse_version(AgentId::Pi, text.as_bytes()),
            Err(VersionProbeError::UnrecognizedOutput)
        );
    }
    assert_eq!(
        parse_version(AgentId::Codex, b"Python 3.12.1"),
        Err(VersionProbeError::UnrecognizedOutput)
    );
    assert_eq!(
        parse_version(AgentId::Pi, &[255]),
        Err(VersionProbeError::UnrecognizedOutput)
    );
    assert!(!safe_version(&format!("1.2.3-{}", "a".repeat(64))));
}

#[test]
fn version_probe_request_cannot_supply_arguments_or_environment() {
    let base = serde_json::json!({"id":"pi","entryPath":"/entry","resolvedPath":"/entry","confirmed":true});
    for extra in ["args", "command", "env", "cwd", "timeout"] {
        let mut input = base.clone();
        input[extra] = serde_json::json!("synthetic-private-value");
        assert!(serde_json::from_value::<VersionProbeInput>(input).is_err());
    }
    assert!(windows_executable(Path::new("C:/entry/PI.EXE")));
    assert!(!windows_executable(Path::new("C:/entry/pi.cmd")));
    assert!(!windows_executable(Path::new("C:/entry/pi.bat")));
}

#[tokio::test]
async fn version_probe_confirmation_and_invalid_paths_fail_before_execution() {
    let fixture = Fixture::new();
    let gate = VersionProbeGate::default();
    let input = VersionProbeInput {
        id: AgentId::Pi,
        entry_path: "relative".into(),
        resolved_path: "relative".into(),
        confirmed: false,
    };
    assert_eq!(
        probe_agent_version(fixture.environment(), input, &gate).await,
        Err(VersionProbeError::ConfirmationRequired)
    );
    let input = VersionProbeInput {
        id: AgentId::Pi,
        entry_path: "relative".into(),
        resolved_path: "relative".into(),
        confirmed: true,
    };
    assert_eq!(
        probe_agent_version(fixture.environment(), input, &gate).await,
        Err(VersionProbeError::EntryUnavailable)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_all_six_use_fixed_arguments_closed_stdin_and_neutral_cwd() {
    let fixture = Fixture::new();
    let gate = VersionProbeGate::default();
    for (id, output) in [
        (AgentId::ClaudeCode, "2.1.119 (Claude Code)"),
        (AgentId::Codex, "codex-cli 0.98.0"),
        (AgentId::OpenCode, "1.2.28"),
        (AgentId::Pi, "1.1.0"),
        (AgentId::GrokBuild, "1.0.50 (c58f321264ba)"),
        (AgentId::Hermes, "Hermes Agent v0.13.0 (2026-10-01)"),
    ] {
        let entry = fixture.script("entry with spaces ; literal", &format!(
            "[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 10\nif read input; then exit 11; fi\ncase \"$PWD\" in */vibemate-version-*) ;; *) exit 12 ;; esac\n[ -z \"${{OPENAI_API_KEY+x}}\" ] && [ -z \"${{NODE_OPTIONS+x}}\" ] && [ -z \"${{BASH_ENV+x}}\" ] || exit 13\nprintf '%s\\n' '{output}'"
        ));
        let result = probe_agent_version(fixture.environment(), fixture.input(id, &entry), &gate)
            .await
            .unwrap();
        assert_eq!(result.evidence, CliVersionEvidence::CliOutput);
        assert_eq!(result.id, id);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("Install directory"));
        assert!(!fixture.0.join("version").exists());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_changed_symlink_and_nonexecutables_do_not_launch() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let fixture = Fixture::new();
    let entry = fixture.script("first", "printf '1.1.0\\n'");
    let next = fixture.script("second", "exit 55");
    let alias = fixture.0.join("alias");
    symlink(&entry, &alias).unwrap();
    let input = fixture.input(AgentId::Pi, &alias);
    fs::remove_file(&alias).unwrap();
    symlink(&next, &alias).unwrap();
    let gate = VersionProbeGate::default();
    assert_eq!(
        probe_agent_version(fixture.environment(), input, &gate).await,
        Err(VersionProbeError::EntryChanged)
    );
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        probe_agent_version(
            fixture.environment(),
            fixture.input(AgentId::Pi, &entry),
            &gate
        )
        .await,
        Err(VersionProbeError::EntryUnavailable)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_drains_both_pipes_and_bounds_each_output() {
    let fixture = Fixture::new();
    let gate = VersionProbeGate::default();
    let entry = fixture.script("stderr-success", "i=0; while [ $i -lt 500 ]; do printf 'warning\\n' >&2; i=$((i+1)); done; printf '1.1.0\\n'");
    assert_eq!(
        probe_agent_version(
            fixture.environment(),
            fixture.input(AgentId::Pi, &entry),
            &gate
        )
        .await
        .unwrap()
        .version,
        "1.1.0"
    );
    for redirect in ["", ">&2"] {
        let entry = fixture.script(
            "overflow",
            &format!(
                "while :; do printf 'synthetic-private-output-0123456789\\n' {redirect}; done"
            ),
        );
        assert_eq!(
            probe_agent_version(
                fixture.environment(),
                fixture.input(AgentId::Pi, &entry),
                &gate
            )
            .await,
            Err(VersionProbeError::OutputTooLarge)
        );
    }
    let entry = fixture.script(
        "nonzero",
        "printf '1.1.0\\n'; printf 'synthetic-secret' >&2; exit 7",
    );
    assert_eq!(
        probe_agent_version(
            fixture.environment(),
            fixture.input(AgentId::Pi, &entry),
            &gate
        )
        .await,
        Err(VersionProbeError::CommandFailed)
    );
    let entry = fixture.script("unexpected", "printf 'synthetic-private-output\\n'");
    let error = probe_agent_version(
        fixture.environment(),
        fixture.input(AgentId::Pi, &entry),
        &gate,
    )
    .await
    .unwrap_err();
    assert_eq!(
        serde_json::to_string(&error).unwrap(),
        "\"unrecognized_output\""
    );
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_timeout_reaps_direct_child_and_cleans_empty_cwd() {
    let fixture = Fixture::new();
    let pid_file = fixture.0.join("pid");
    let entry = fixture.script(
        "endless",
        &format!(
            "printf '%s' \"$$\" > '{}'\nwhile :; do :; done",
            pid_file.display()
        ),
    );
    let prepared = prepare(fixture.environment(), fixture.input(AgentId::Pi, &entry)).unwrap();
    let cwd = prepared.scratch.0.clone();
    assert_eq!(
        execute(&prepared, Duration::from_secs(2)).await,
        Err(VersionProbeError::TimedOut)
    );
    let pid = fs::read_to_string(pid_file).unwrap();
    // kill -0 tests process presence without signaling it. Only our fixture PID
    // is checked; installed Agents are never started or inspected by this test.
    let status = std::process::Command::new("/bin/kill")
        .args(["-0", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    drop(prepared);
    assert!(!cwd.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_cancellation_requests_child_termination_and_releases_gate() {
    let fixture = Fixture::new();
    let pid_file = fixture.0.join("cancel-pid");
    let entry = fixture.script(
        "cancelled",
        &format!(
            "printf '%s' \"$$\" > '{}'\nwhile :; do :; done",
            pid_file.display()
        ),
    );
    let environment = fixture.environment();
    let input = fixture.input(AgentId::Pi, &entry);
    let gate = std::sync::Arc::new(VersionProbeGate::default());
    let running_gate = gate.clone();
    let request =
        tokio::spawn(async move { probe_agent_version(environment, input, &running_gate).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !pid_file.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let pid = fs::read_to_string(&pid_file).unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let alive = std::process::Command::new("/bin/kill")
                .args(["-0", &pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            if !alive {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(gate.0.try_acquire().is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn version_probe_does_not_delete_files_written_by_program() {
    let fixture = Fixture::new();
    let entry = fixture.script(
        "writes-file",
        "printf 'synthetic-owned-by-program' > keep; printf '1.1.0\\n'",
    );
    let prepared = prepare(fixture.environment(), fixture.input(AgentId::Pi, &entry)).unwrap();
    let cwd = prepared.scratch.0.clone();
    assert_eq!(execute(&prepared, RUN_LIMIT).await.unwrap(), "1.1.0");
    drop(prepared);
    assert_eq!(
        fs::read_to_string(cwd.join("keep")).unwrap(),
        "synthetic-owned-by-program"
    );
    fs::remove_file(cwd.join("keep")).unwrap();
    fs::remove_dir(cwd).unwrap();
}

#[tokio::test]
async fn version_probe_busy_requests_fail_instead_of_queueing() {
    let fixture = Fixture::new();
    let gate = VersionProbeGate::default();
    let permit = gate.0.try_acquire().unwrap();
    let input = VersionProbeInput {
        id: AgentId::Pi,
        entry_path: "relative".into(),
        resolved_path: "relative".into(),
        confirmed: true,
    };
    assert_eq!(
        probe_agent_version(fixture.environment(), input, &gate).await,
        Err(VersionProbeError::Busy)
    );
    drop(permit);
    assert!(gate.0.try_acquire().is_ok());
}

#[cfg(windows)]
#[test]
fn version_probe_windows_batch_entries_are_rejected_without_shell() {
    let fixture = Fixture::new();
    for name in ["pi.cmd", "pi.bat", "pi"] {
        let entry = fixture.0.join(name);
        fs::write(&entry, "@echo synthetic-must-not-execute").unwrap();
        assert!(matches!(
            prepare(fixture.environment(), fixture.input(AgentId::Pi, &entry)),
            Err(VersionProbeError::UnsupportedEntry)
        ));
    }
}
