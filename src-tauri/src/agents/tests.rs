//! Isolated agent_discovery fixtures. Never use a user's home or launch an Agent.
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vibemate-agent-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn environment(&self) -> DiscoveryEnvironment {
        DiscoveryEnvironment {
            home: self.0.clone(),
            path: None,
            pi_directory: None,
            grok_directory: None,
            local_app_data: Some(self.0.join("local-app-data").into_os_string()),
            ..DiscoveryEnvironment::default()
        }
    }
    fn file(&self, path: &str, content: &str) -> PathBuf {
        let target = self.0.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, content).unwrap();
        target
    }
    fn pi(&self, version: &str) -> PathBuf {
        let entry = self.file(
            "package/dist/bundle/cli.js",
            "throw new Error('must not run');",
        );
        self.file(
            "package/package.json",
            &serde_json::json!({
                "name":"@earendil-works/pi-coding-agent", "version":version,
                "bin":{"pi":"dist/bundle/cli.js"}, "privateField":"synthetic-secret-never-return"
            })
            .to_string(),
        );
        entry
    }
    fn scan(&self, pi: Option<&Path>, grok: Option<&Path>) -> Vec<AgentObservation> {
        discover_agents(
            &self.environment(),
            &DiscoveryInput {
                pi_path: pi.map(|p| p.to_str().unwrap().to_owned()),
                grok_path: grok.map(|p| p.to_str().unwrap().to_owned()),
                ..DiscoveryInput::default()
            },
        )
        .unwrap()
        .into_iter()
        // Preserve the original Pi/Grok test focus; new tests observe all six.
        .filter(|row| matches!(row.id, AgentId::Pi | AgentId::GrokBuild))
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn agent_discovery_absent_entries_do_not_create_config_or_claim_installation() {
    let fixture = Fixture::new();
    let rows = fixture.scan(None, None);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, AgentId::Pi);
    assert_eq!(rows[1].id, AgentId::GrokBuild);
    for row in rows {
        assert_eq!(row.state, EntryState::NotFound);
        assert_eq!(row.source, EntrySource::None);
        assert_eq!(row.version, None);
        assert!(row.files.iter().all(|f| f.state == FileState::Missing));
    }
    assert!(!fixture.0.join(".pi").exists());
    assert!(!fixture.0.join(".grok").exists());
}

#[test]
fn agent_discovery_pi_metadata_is_tied_to_entry_and_never_returns_other_fields() {
    let fixture = Fixture::new();
    let entry = fixture.pi("1.1.0");
    fixture.file(".pi/agent/models.json", "synthetic-config-secret");
    let rows = fixture.scan(Some(&entry), None);
    let row = &rows[0];
    assert_eq!(row.state, EntryState::Found);
    assert_eq!(row.version.as_deref(), Some("1.1.0"));
    assert_eq!(row.version_evidence, VersionEvidence::PackageMetadata);
    assert_eq!(row.capability_evidence, CapabilityEvidence::Documented);
    assert_eq!(row.files[1].state, FileState::Present);
    let json = serde_json::to_string(&rows).unwrap();
    assert!(!json.contains("synthetic"));
    assert!(!json.contains("privateField"));
    assert_eq!(
        fs::read_to_string(fixture.0.join(".pi/agent/models.json")).unwrap(),
        "synthetic-config-secret"
    );
}

#[test]
fn agent_discovery_unknown_versions_are_not_unsupported_or_documented() {
    let fixture = Fixture::new();
    let entry = fixture.pi("1.2.0-beta.1");
    let rows = fixture.scan(Some(&entry), None);
    assert_eq!(rows[0].version.as_deref(), Some("1.2.0-beta.1"));
    assert_eq!(rows[0].capability_evidence, CapabilityEvidence::Unverified);
}

#[test]
fn agent_discovery_unrelated_bin_and_legacy_package_are_not_identified_as_pi() {
    let fixture = Fixture::new();
    let entry = fixture.pi("1.1.0");
    let other = fixture.file("package/dist/other.js", "not pi");
    assert_eq!(fixture.scan(Some(&other), None)[0].version, None);
    fixture.file("package/package.json", r#"{"name":"@mariozechner/pi-coding-agent","version":"1.1.0","bin":{"pi":"dist/bundle/cli.js"}}"#);
    assert_eq!(fixture.scan(Some(&entry), None)[0].version, None);
}

#[test]
fn agent_discovery_metadata_rejects_oversized_malformed_and_unsafe_versions() {
    let fixture = Fixture::new();
    let entry = fixture.pi("1.1.0");
    for content in ["x".repeat(65_537), "{invalid json".to_owned(), r#"{"name":"@earendil-works/pi-coding-agent","version":"synthetic-secret","bin":{"pi":"dist/bundle/cli.js"}}"#.to_owned()] {
        fixture.file("package/package.json", &content);
        assert_eq!(fixture.scan(Some(&entry), None)[0].version, None);
    }
}

#[test]
fn agent_discovery_grok_filename_is_only_an_unverified_version_hint() {
    let fixture = Fixture::new();
    for platform in [
        "macos-aarch64",
        "macos-x86_64",
        "linux-aarch64",
        "linux-x86_64",
    ] {
        let entry = fixture.file(
            &format!(".grok/downloads/grok-1.0.50-{platform}"),
            "not executed",
        );
        let rows = fixture.scan(None, Some(&entry));
        assert_eq!(rows[1].version.as_deref(), Some("1.0.50"));
        assert_eq!(rows[1].version_evidence, VersionEvidence::Filename);
        assert_eq!(rows[1].capability_evidence, CapabilityEvidence::Unverified);
    }
    let entry = fixture.file("grok.exe", "not executed");
    assert_eq!(fixture.scan(None, Some(&entry))[1].version, None);
}

#[test]
fn agent_discovery_manual_paths_validate_before_scanning() {
    let fixture = Fixture::new();
    for value in [
        "relative/pi".to_owned(),
        "".to_owned(),
        fixture.0.join("bad\npi").to_str().unwrap().to_owned(),
        format!("/{}", "x".repeat(4096)),
    ] {
        let input = DiscoveryInput {
            pi_path: Some(value),
            grok_path: None,
            ..DiscoveryInput::default()
        };
        assert_eq!(
            discover_agents(&fixture.environment(), &input).unwrap_err(),
            DiscoveryError::InvalidPath
        );
    }
    let missing = fixture.0.join("absent");
    let rows = fixture.scan(Some(&missing), None);
    assert_eq!(rows[0].state, EntryState::NotFound);
    assert_eq!(rows[0].source, EntrySource::Manual);
    assert!(rows[0].entry_path.is_some());
    assert!(
        serde_json::from_str::<DiscoveryInput>(r#"{"piPath":null,"grokPath":null,"execute":true}"#)
            .is_err()
    );
}

#[test]
fn agent_discovery_environment_overrides_do_not_fall_back_when_invalid() {
    let fixture = Fixture::new();
    let custom = fixture.0.join("custom");
    fixture.file("custom/models.json", "not read");
    let mut environment = fixture.environment();
    environment.pi_directory = Some(custom.clone().into_os_string());
    environment.grok_directory = Some("relative-directory".into());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows[3].config_directory, custom.to_str().map(str::to_owned));
    assert_eq!(rows[3].directory_source, DirectorySource::Environment);
    assert_eq!(rows[3].files[1].state, FileState::Present);
    assert_eq!(rows[4].config_directory, None);
    assert_eq!(rows[4].directory_source, DirectorySource::Unavailable);
    assert!(
        rows[4]
            .files
            .iter()
            .all(|f| f.state == FileState::Unavailable)
    );
}

#[test]
fn agent_discovery_directory_entry_is_unavailable_and_configs_are_not_parsed() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.0.join("directory")).unwrap();
    fixture.file(".grok/config.toml", "broken TOML synthetic-secret");
    let rows = fixture.scan(None, Some(&fixture.0.join("directory")));
    assert_eq!(rows[1].state, EntryState::Unavailable);
    assert_eq!(rows[1].resolved_path, None);
    assert_eq!(rows[1].files[0].state, FileState::Present);
    assert!(
        !serde_json::to_string(&rows)
            .unwrap()
            .contains("synthetic-secret")
    );
}

#[cfg(unix)]
#[test]
fn agent_discovery_symlinks_follow_entry_without_executing_and_keep_path_order() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    let entry = fixture.pi("1.1.0");
    let marker = fixture.0.join("SHOULD_NOT_RUN");
    fs::write(&entry, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(fixture.0.join("first")).unwrap();
    fs::create_dir_all(fixture.0.join("second")).unwrap();
    symlink(&entry, fixture.0.join("first/pi")).unwrap();
    symlink(&entry, fixture.0.join("second/pi")).unwrap();
    let mut environment = fixture.environment();
    environment.path = Some(
        std::env::join_paths([
            PathBuf::from("."),
            fixture.0.join("first"),
            fixture.0.join("second"),
        ])
        .unwrap(),
    );
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows[3].source, EntrySource::Path);
    assert_eq!(
        rows[3].entry_path,
        fixture.0.join("first/pi").to_str().map(str::to_owned)
    );
    assert_eq!(rows[3].version.as_deref(), Some("1.1.0"));
    assert!(!marker.exists());
    fs::remove_file(fixture.0.join("first/pi")).unwrap();
    symlink(fixture.0.join("missing"), fixture.0.join("first/pi")).unwrap();
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows[3].state, EntryState::Unavailable);
    assert_eq!(rows[3].source, EntrySource::Path);
    assert_eq!(rows[3].version, None);
}

#[cfg(unix)]
#[test]
fn agent_discovery_symlinked_metadata_is_not_read_and_launcher_does_not_pick_release() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let entry = fixture.pi("1.1.0");
    fs::rename(
        fixture.0.join("package/package.json"),
        fixture.0.join("metadata.json"),
    )
    .unwrap();
    symlink(
        fixture.0.join("metadata.json"),
        fixture.0.join("package/package.json"),
    )
    .unwrap();
    assert_eq!(fixture.scan(Some(&entry), None)[0].version, None);
    let launcher = fixture.file(".pi/agent/bin/pi", "not read or executed");
    fixture.file(
        ".pi/agent/install/releases/1.1.0/package.json",
        r#"{"version":"1.1.0"}"#,
    );
    let rows = fixture.scan(None, None);
    assert_eq!(rows[0].entry_path, launcher.to_str().map(str::to_owned));
    assert_eq!(rows[0].state, EntryState::Found);
    assert_eq!(rows[0].version, None);
}

#[test]
fn agent_discovery_six_path_entries_are_observed_without_execution_or_config_reads() {
    let fixture = Fixture::new();
    let marker = fixture.0.join("SHOULD_NOT_RUN");
    let names = ["claude", "codex", "opencode", "pi", "grok", "hermes"];
    let ids = [
        AgentId::ClaudeCode,
        AgentId::Codex,
        AgentId::OpenCode,
        AgentId::Pi,
        AgentId::GrokBuild,
        AgentId::Hermes,
    ];
    for name in names {
        let filename = if cfg!(windows) {
            format!("{name}.cmd")
        } else {
            name.to_owned()
        };
        fixture.file(
            &format!("bin/{filename}"),
            &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        );
    }
    let configs = [
        ".claude/settings.json",
        ".codex/config.toml",
        ".config/opencode/opencode.jsonc",
        ".pi/agent/models.json",
        ".grok/config.toml",
    ];
    for path in configs {
        fixture.file(path, "invalid-config synthetic-secret-do-not-read");
    }
    let hermes_root = if cfg!(windows) {
        "local-app-data/hermes"
    } else {
        ".hermes"
    };
    let hermes_config = format!("{hermes_root}/config.yaml");
    fixture.file(
        &hermes_config,
        "invalid-config synthetic-secret-do-not-read",
    );
    let mut environment = fixture.environment();
    environment.path = Some(std::env::join_paths([fixture.0.join("bin")]).unwrap());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows.len(), 6);
    for (row, id) in rows.iter().zip(ids) {
        assert_eq!(row.id, id);
        assert_eq!(row.state, EntryState::Found);
        assert_eq!(row.source, EntrySource::Path);
        assert!(row.files.iter().any(|f| f.state == FileState::Present));
        assert_eq!(row.version, None);
        assert_eq!(row.capability_evidence, CapabilityEvidence::Unverified);
    }
    assert!(
        !serde_json::to_string(&rows)
            .unwrap()
            .contains("synthetic-secret")
    );
    for path in configs.into_iter().chain([hermes_config.as_str()]) {
        assert_eq!(
            fs::read_to_string(fixture.0.join(path)).unwrap(),
            "invalid-config synthetic-secret-do-not-read"
        );
    }
    assert!(!marker.exists());
}

#[test]
fn agent_discovery_config_residue_does_not_claim_any_installed_entry() {
    let fixture = Fixture::new();
    for path in [
        ".claude/settings.json",
        ".codex/config.toml",
        ".config/opencode/opencode.json",
        ".pi/agent/settings.json",
        ".grok/config.toml",
        ".hermes/config.yaml",
    ] {
        fixture.file(path, "unopened");
    }
    let rows = discover_agents(&fixture.environment(), &DiscoveryInput::default()).unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        assert_eq!(row.state, EntryState::NotFound);
        assert_eq!(row.entry_path, None);
        assert_eq!(row.version, None);
    }
}

#[test]
fn agent_discovery_every_manual_path_is_independent_and_validated() {
    let fixture = Fixture::new();
    let fields = [
        "claudePath",
        "codexPath",
        "opencodePath",
        "piPath",
        "grokPath",
        "hermesPath",
    ];
    let bin = fixture.file("manual-entry", "not executed");
    let mut environment = fixture.environment();
    fixture.file("bin/codex", "should be shadowed by explicit path");
    environment.path = Some(std::env::join_paths([fixture.0.join("bin")]).unwrap());
    for (index, field) in fields.iter().enumerate() {
        let input: DiscoveryInput =
            serde_json::from_value(serde_json::json!({field.to_string(): bin})).unwrap();
        let rows = discover_agents(&environment, &input).unwrap();
        assert_eq!(rows[index].source, EntrySource::Manual);
        assert_eq!(
            rows[index].resolved_path,
            fs::canonicalize(&bin).unwrap().to_str().map(str::to_owned)
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.source == EntrySource::Manual)
                .count(),
            1
        );
        for invalid in ["relative", "", "/bad\npath"] {
            let input: DiscoveryInput =
                serde_json::from_value(serde_json::json!({field.to_string(): invalid})).unwrap();
            assert_eq!(
                discover_agents(&environment, &input).unwrap_err(),
                DiscoveryError::InvalidPath
            );
        }
        let missing = fixture.0.join("missing");
        let input: DiscoveryInput =
            serde_json::from_value(serde_json::json!({field.to_string(): missing})).unwrap();
        let rows = discover_agents(&environment, &input).unwrap();
        assert_eq!(rows[index].state, EntryState::NotFound);
        assert_eq!(rows[index].source, EntrySource::Manual);
    }
}

#[test]
fn agent_discovery_new_directory_overrides_are_observations_not_installations() {
    let fixture = Fixture::new();
    let root = fixture.0.join("custom");
    fixture.file("custom/settings.json", "unopened");
    fixture.file("custom/config.toml", "unopened");
    fixture.file("custom/opencode.json", "unopened");
    fixture.file("custom/config.yaml", "unopened");
    let mut environment = fixture.environment();
    environment.claude_directory = Some(root.clone().into_os_string());
    environment.codex_directory = Some(root.clone().into_os_string());
    environment.opencode_directory = Some(root.clone().into_os_string());
    environment.hermes_directory = Some(root.clone().into_os_string());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    for index in [0, 1, 2, 5] {
        let expected = if index == 1 {
            fs::canonicalize(&root).unwrap()
        } else {
            root.clone()
        };
        assert_eq!(
            rows[index].config_directory,
            expected.to_str().map(str::to_owned)
        );
        assert_eq!(rows[index].directory_source, DirectorySource::Environment);
        assert_eq!(rows[index].files[0].state, FileState::Present);
        assert_eq!(rows[index].state, EntryState::NotFound);
    }
    environment.claude_directory = Some("relative".into());
    environment.codex_directory = Some(fixture.0.join("missing").into_os_string());
    environment.opencode_directory = Some("relative".into());
    environment.hermes_directory = Some("relative".into());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    for index in [0, 1, 2, 5] {
        assert_eq!(rows[index].directory_source, DirectorySource::Unavailable);
        assert_eq!(rows[index].config_directory, None);
        assert!(
            rows[index]
                .files
                .iter()
                .all(|f| f.state == FileState::Unavailable)
        );
    }
}

#[test]
fn agent_discovery_opencode_xdg_and_codex_empty_override_keep_documented_distinctions() {
    let fixture = Fixture::new();
    fixture.file("xdg/opencode/opencode.json", "unopened");
    let mut environment = fixture.environment();
    environment.xdg_config_directory = Some(fixture.0.join("xdg").into_os_string());
    environment.codex_directory = Some("".into());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(
        rows[1].config_directory,
        fixture.0.join(".codex").to_str().map(str::to_owned)
    );
    assert_eq!(rows[1].directory_source, DirectorySource::Default);
    assert_eq!(
        rows[2].config_directory,
        fixture.0.join("xdg/opencode").to_str().map(str::to_owned)
    );
    assert_eq!(rows[2].files[0].state, FileState::Present);
    environment.xdg_config_directory = Some("relative".into());
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows[2].directory_source, DirectorySource::Unavailable);
}

#[test]
fn agent_discovery_documented_user_entries_are_detected_without_guessing_codex_prefix() {
    let fixture = Fixture::new();
    let filename = |name: &str| {
        if cfg!(windows) {
            format!("{name}.cmd")
        } else {
            name.to_owned()
        }
    };
    fixture.file(
        &format!(".local/bin/{}", filename("claude")),
        "not executed",
    );
    fixture.file(
        &format!(".opencode/bin/{}", filename("opencode")),
        "not executed",
    );
    let hermes_root = if cfg!(windows) {
        "local-app-data/hermes/bin"
    } else {
        ".local/bin"
    };
    fixture.file(
        &format!("{hermes_root}/{}", filename("hermes")),
        "not executed",
    );
    let rows = discover_agents(&fixture.environment(), &DiscoveryInput::default()).unwrap();
    let known_indices: &[usize] = if cfg!(windows) { &[5] } else { &[0, 2, 5] };
    for &index in known_indices {
        assert_eq!(rows[index].state, EntryState::Found);
        assert_eq!(rows[index].source, EntrySource::UserEntry);
    }
    assert_eq!(rows[1].state, EntryState::NotFound);
    if cfg!(windows) {
        assert_eq!(rows[0].state, EntryState::NotFound);
        assert_eq!(rows[2].state, EntryState::NotFound);
    }
}
