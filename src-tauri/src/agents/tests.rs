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
            },
        )
        .unwrap()
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
    assert_eq!(rows[0].config_directory, custom.to_str().map(str::to_owned));
    assert_eq!(rows[0].directory_source, DirectorySource::Environment);
    assert_eq!(rows[0].files[1].state, FileState::Present);
    assert_eq!(rows[1].config_directory, None);
    assert_eq!(rows[1].directory_source, DirectorySource::Unavailable);
    assert!(
        rows[1]
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
    assert_eq!(rows[0].source, EntrySource::Path);
    assert_eq!(
        rows[0].entry_path,
        fixture.0.join("first/pi").to_str().map(str::to_owned)
    );
    assert_eq!(rows[0].version.as_deref(), Some("1.1.0"));
    assert!(!marker.exists());
    fs::remove_file(fixture.0.join("first/pi")).unwrap();
    symlink(fixture.0.join("missing"), fixture.0.join("first/pi")).unwrap();
    let rows = discover_agents(&environment, &DiscoveryInput::default()).unwrap();
    assert_eq!(rows[0].state, EntryState::Unavailable);
    assert_eq!(rows[0].source, EntrySource::Path);
    assert_eq!(rows[0].version, None);
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
