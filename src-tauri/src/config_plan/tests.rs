//! Isolated preview-plan tests. Fixtures live under the system temp directory
//! and are removed when the test ends; no user settings file is touched.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

/// SHA-256 of an empty byte string, computed with `shasum -a 256`.
const EMPTY_DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// SHA-256 of [`THEME_DOCUMENT`], computed with `shasum -a 256`.
const THEME_DIGEST: &str = "27dafb2742d0da69a49cc8d206fc9cc429feff09cc3738addcf590d9c4358f97";
const THEME_DOCUMENT: &str = "{\n  \"theme\": \"dark\"\n}\n";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vibemate-config-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn file(&self, name: &str, content: &str) -> PathBuf {
        let target = self.path(name);
        fs::write(&target, content).unwrap();
        target
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn literal(key_path: &'static str, value: &str) -> PlannedWrite {
    PlannedWrite {
        key_path,
        value: PlannedValue::Literal(value.to_owned()),
    }
}

#[test]
fn a_missing_file_reads_as_an_empty_document_without_a_digest() {
    let fixture = Fixture::new();
    let read = read_target_document(&fixture.path("settings.json")).unwrap();
    assert!(!read.target.exists);
    assert_eq!(read.target.content_digest, None);
    assert!(!read.target.symlink);
    assert!(read.document.is_empty());
}

#[test]
fn an_existing_document_reports_the_digest_of_its_bytes() {
    let fixture = Fixture::new();
    let read = read_target_document(&fixture.file("settings.json", THEME_DOCUMENT)).unwrap();
    assert!(read.target.exists);
    assert_eq!(read.target.content_digest.as_deref(), Some(THEME_DIGEST));
    assert_eq!(read.document["theme"], serde_json::json!("dark"));
}

#[test]
fn a_blank_file_reads_as_an_empty_document_with_the_digest_of_nothing() {
    let fixture = Fixture::new();
    let empty = read_target_document(&fixture.file("empty.json", "")).unwrap();
    assert!(empty.target.exists);
    assert_eq!(empty.target.content_digest.as_deref(), Some(EMPTY_DIGEST));
    assert!(empty.document.is_empty());

    // Whitespace-only content is still blank rather than a parse failure.
    let blank = read_target_document(&fixture.file("blank.json", "  \n")).unwrap();
    assert!(blank.document.is_empty());
    assert_ne!(blank.target.content_digest.as_deref(), Some(EMPTY_DIGEST));
}

#[test]
fn invalid_json_and_non_objects_are_refused() {
    let fixture = Fixture::new();
    for (index, content) in ["[]", "[1]", "null", "{\"a\":1", "\"text\"", "7"]
        .iter()
        .enumerate()
    {
        let path = fixture.file(&format!("broken-{index}.json"), content);
        assert_eq!(
            read_target_document(&path).unwrap_err(),
            PlanError::TargetUnreadable,
            "content {content} must be refused"
        );
    }
}

#[test]
fn a_directory_in_place_of_the_file_is_refused() {
    let fixture = Fixture::new();
    let directory = fixture.path("settings.json");
    fs::create_dir_all(&directory).unwrap();
    assert_eq!(
        read_target_document(&directory).unwrap_err(),
        PlanError::TargetUnreadable
    );
}

#[test]
fn a_relative_path_is_refused() {
    assert_eq!(
        read_target_document(Path::new("settings.json")).unwrap_err(),
        PlanError::TargetPathInvalid
    );
}

#[test]
fn an_oversized_document_is_refused() {
    let fixture = Fixture::new();
    let content = " ".repeat(usize::try_from(MAX_TARGET_BYTES).unwrap() + 1);
    let path = fixture.file("settings.json", &content);
    assert_eq!(
        read_target_document(&path).unwrap_err(),
        PlanError::TargetTooLarge
    );
}

#[test]
fn nested_writes_create_missing_objects_and_keep_unrelated_keys() {
    let mut document = serde_json::json!({
        "theme": "dark",
        "permissions": { "defaultMode": "acceptEdits" }
    })
    .as_object()
    .unwrap()
    .clone();
    let changes = planned_changes(
        &mut document,
        &[
            literal("env.ANTHROPIC_MODEL", "deepseek-flash"),
            literal(
                "env.ANTHROPIC_BASE_URL",
                "https://api.deepseek.com/anthropic",
            ),
        ],
    )
    .unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].key_path, "env.ANTHROPIC_MODEL");
    assert_eq!(changes[0].before, None);
    assert_eq!(changes[0].after, "deepseek-flash");
    assert!(!changes[0].secret);
    assert_eq!(document["theme"], serde_json::json!("dark"));
    assert_eq!(
        document["permissions"]["defaultMode"],
        serde_json::json!("acceptEdits")
    );
    assert_eq!(
        document["env"]["ANTHROPIC_MODEL"],
        serde_json::json!("deepseek-flash")
    );
    assert_eq!(
        document["env"]["ANTHROPIC_BASE_URL"],
        serde_json::json!("https://api.deepseek.com/anthropic")
    );
}

#[test]
fn a_non_object_parent_is_a_conflict() {
    let mut document = serde_json::json!({ "env": "not-an-object" })
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        planned_changes(&mut document, &[literal("env.ANTHROPIC_MODEL", "m")]).unwrap_err(),
        PlanError::TargetConflict
    );
    assert_eq!(document["env"], serde_json::json!("not-an-object"));
}

#[test]
fn a_credential_write_shows_a_placeholder_and_marks_the_change() {
    let mut document = Map::new();
    let changes = planned_changes(
        &mut document,
        &[PlannedWrite {
            key_path: "env.ANTHROPIC_AUTH_TOKEN",
            value: PlannedValue::Credential,
        }],
    )
    .unwrap();
    assert_eq!(changes[0].after, SECRET_PLACEHOLDER);
    assert!(changes[0].secret);
    assert_eq!(
        document["env"]["ANTHROPIC_AUTH_TOKEN"],
        serde_json::json!(SECRET_PLACEHOLDER)
    );
}

#[test]
fn existing_credentials_are_masked_before_the_plan_crosses_ipc() {
    let mut document = serde_json::json!({"env":{
        "ANTHROPIC_AUTH_TOKEN":"synthetic-old-token",
        "ANTHROPIC_API_KEY":"synthetic-old-direct-key"
    }})
    .as_object()
    .unwrap()
    .clone();
    let changes = planned_changes(
        &mut document,
        &[
            PlannedWrite {
                key_path: "env.ANTHROPIC_AUTH_TOKEN",
                value: PlannedValue::Credential,
            },
            literal("env.ANTHROPIC_API_KEY", ""),
        ],
    )
    .unwrap();
    assert_eq!(changes[0].before.as_deref(), Some(SECRET_PLACEHOLDER));
    assert_eq!(changes[1].before.as_deref(), Some(SECRET_PLACEHOLDER));
    assert_eq!(changes[1].after, "");
    let payload = serde_json::to_string(&changes).unwrap();
    assert!(!payload.contains("synthetic-old-token"));
    assert!(!payload.contains("synthetic-old-direct-key"));
    assert_eq!(document["env"]["ANTHROPIC_API_KEY"], "");
}

#[test]
fn before_values_describe_existing_values() {
    let mut document = serde_json::json!({
        "env": { "ANTHROPIC_MODEL": "opus", "ANTHROPIC_API_KEY": "" },
        "fallbackModel": ["a", "b"],
        "alwaysThinkingEnabled": true,
        "autoCompactWindow": 200000
    })
    .as_object()
    .unwrap()
    .clone();
    let changes = planned_changes(
        &mut document,
        &[
            literal("env.ANTHROPIC_MODEL", "deepseek-flash"),
            literal("env.ANTHROPIC_API_KEY", ""),
            literal("alwaysThinkingEnabled", "false"),
        ],
    )
    .unwrap();
    assert_eq!(changes[0].before.as_deref(), Some("opus"));
    assert_eq!(changes[1].before.as_deref(), Some(""));
    assert_eq!(changes[2].before.as_deref(), Some("true"));
    assert_eq!(document["fallbackModel"], serde_json::json!(["a", "b"]));
    assert_eq!(document["autoCompactWindow"], serde_json::json!(200000));
}

#[test]
fn an_empty_key_path_is_refused() {
    let mut document = Map::new();
    assert_eq!(
        planned_changes(&mut document, &[literal("env.", "x")]).unwrap_err(),
        PlanError::InvalidRequest
    );
    assert_eq!(
        planned_changes(&mut document, &[literal("", "x")]).unwrap_err(),
        PlanError::InvalidRequest
    );
    assert!(document.is_empty());
}
