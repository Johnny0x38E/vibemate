//! Behavior tests for the parent module, kept separate from runtime code.

mod unified_save;
use super::validation::{parse_protocol_for, validate_extensions};
use super::*;
use crate::credentials::fake::FakeStore;
use crate::credentials::{CredentialError, InvalidSecret, Secret};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn no_extensions() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[test]
fn templates_use_the_documented_defaults_and_only_chat_completions() {
    let expected = [
        (
            ProviderKind::CommandCode,
            "https://api.commandcode.ai/provider/v1",
        ),
        (ProviderKind::DeepSeek, "https://api.deepseek.com"),
        (ProviderKind::OpenRouter, "https://openrouter.ai/api/v1"),
    ];
    assert_eq!(provider_templates().len(), expected.len());
    for (template, (kind, url)) in provider_templates().iter().zip(expected) {
        assert_eq!(template.kind, kind);
        assert_eq!(kind.template(), template);
        assert_eq!(template.default_base_url, url);
        assert_eq!(template.protocols, &[ProviderProtocol::ChatCompletions]);
        assert!(template.extension_fields.is_empty());
        // A default must survive normalization unchanged, or a fresh form would
        // save a different URL than the one it displayed.
        assert_eq!(normalize_base_url(url).as_deref(), Ok(url));
        assert_eq!(ProviderKind::from_value(kind.as_str()), Some(kind));
    }
}

#[test]
fn credential_failures_map_to_stable_codes() {
    for (error, code) in [
        (
            CredentialError::Unavailable,
            ProviderError::CredentialStoreUnavailable,
        ),
        (
            CredentialError::AccessDenied,
            ProviderError::CredentialStoreAccessDenied,
        ),
        (
            CredentialError::OperationFailed,
            ProviderError::CredentialStoreFailed,
        ),
        (
            CredentialError::InvalidReference,
            ProviderError::CredentialStoreFailed,
        ),
        (
            CredentialError::CorruptValue,
            ProviderError::CredentialStoreFailed,
        ),
    ] {
        assert_eq!(ProviderError::from(error), code);
    }
    assert_eq!(
        ProviderError::from(InvalidSecret),
        ProviderError::SecretInvalid
    );
}

#[test]
fn every_kind_picks_its_own_template() {
    // `template()` selects by array index; this catches a reordered TEMPLATES.
    for kind in [
        ProviderKind::CommandCode,
        ProviderKind::DeepSeek,
        ProviderKind::OpenRouter,
    ] {
        assert_eq!(kind.template().kind, kind);
    }
}

#[test]
fn unknown_kinds_receive_a_stable_code() {
    for value in [
        "",
        "custom",
        "DeepSeek",
        "command_code",
        "Command Code",
        " deepseek",
    ] {
        assert_eq!(parse_kind(value), Err(ProviderError::KindNotSupported));
    }
    assert_eq!(parse_kind("openrouter"), Ok(ProviderKind::OpenRouter));
}

#[test]
fn protocols_outside_the_kind_template_are_rejected() {
    for kind in [
        ProviderKind::CommandCode,
        ProviderKind::DeepSeek,
        ProviderKind::OpenRouter,
    ] {
        assert_eq!(
            parse_protocol_for(kind, "chat_completions"),
            Ok(ProviderProtocol::ChatCompletions)
        );
        for value in ["responses", "anthropic_messages", "openai-completions", ""] {
            assert_eq!(
                parse_protocol_for(kind, value),
                Err(ProviderError::ProtocolNotSupported)
            );
        }
    }
    for protocol in [
        ProviderProtocol::ChatCompletions,
        ProviderProtocol::Responses,
        ProviderProtocol::AnthropicMessages,
    ] {
        assert_eq!(
            ProviderProtocol::from_value(protocol.as_str()),
            Some(protocol)
        );
    }
}

#[test]
fn display_names_are_trimmed_and_limited_by_characters() {
    assert_eq!(validate_display_name("  Work  ").as_deref(), Ok("Work"));
    let chinese_limit = "名".repeat(MAX_DISPLAY_NAME_CHARS);
    assert_eq!(
        validate_display_name(&chinese_limit),
        Ok(chinese_limit.clone())
    );
    let too_long = format!("{chinese_limit}名");
    for invalid in ["", "   ", too_long.as_str(), "line\nbreak", "bell\u{7}"] {
        assert_eq!(
            validate_display_name(invalid),
            Err(ProviderError::DisplayNameInvalid)
        );
    }
}

#[test]
fn display_names_reject_invisible_and_bidi_characters() {
    for invalid in [
        "\u{200B}",
        "\u{200B}\u{200D}\u{200C}",
        " \u{200D} ",
        "\u{FEFF}Work",
        "Wo\u{2060}rk",
        "soft\u{00AD}hyphen",
        "a\u{202E}b",
        "a\u{202A}b",
        "a\u{2066}b\u{2069}",
        "a\u{200E}b",
        "a\u{200F}b",
        "a\u{061C}b",
    ] {
        assert_eq!(
            validate_display_name(invalid),
            Err(ProviderError::DisplayNameInvalid),
            "input: {invalid:?}"
        );
    }
    // Joiners that real text needs stay allowed next to visible characters.
    for valid in [
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} Family",
        "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
        "工作 DeepSeek",
    ] {
        assert_eq!(validate_display_name(valid).as_deref(), Ok(valid));
    }
}

#[test]
fn base_urls_are_normalized_without_adding_paths() {
    for (input, expected) in [
        ("HTTPS://API.DeepSeek.com/", "https://api.deepseek.com"),
        (
            "  https://openrouter.ai/api/v1/  ",
            "https://openrouter.ai/api/v1",
        ),
        (
            "https://example.com:8443/base",
            "https://example.com:8443/base",
        ),
        ("https://例子.测试/v1", "https://xn--fsqu00a.xn--0zwm56d/v1"),
        // Only one trailing slash is removed.
        ("https://a.com/v1//", "https://a.com/v1/"),
        // Loopback and private hosts stay allowed (decided in P12).
        ("https://127.0.0.1:8443/v1", "https://127.0.0.1:8443/v1"),
        ("https://192.168.1.10/v1/", "https://192.168.1.10/v1"),
        ("\t https://example.com/v1 \n", "https://example.com/v1"),
    ] {
        assert_eq!(normalize_base_url(input).as_deref(), Ok(expected));
    }
}

#[test]
fn base_urls_with_unsafe_or_unsupported_parts_are_rejected() {
    let too_long = format!("https://example.com/{}", "a".repeat(MAX_BASE_URL_LEN));
    for (input, error) in [
        ("", ProviderError::BaseUrlInvalid),
        ("not a url", ProviderError::BaseUrlInvalid),
        ("api.deepseek.com", ProviderError::BaseUrlInvalid),
        // `url` would silently delete these; vibemate rejects them instead.
        (
            "https://api.example\n.com/v1",
            ProviderError::BaseUrlInvalid,
        ),
        (
            "https://api.example.com/v\t1",
            ProviderError::BaseUrlInvalid,
        ),
        (
            "https://api.example.com/v1\r/x",
            ProviderError::BaseUrlInvalid,
        ),
        (
            "https://api.example.com/my path",
            ProviderError::BaseUrlInvalid,
        ),
        (
            "https://api.example.com/v1\u{7F}",
            ProviderError::BaseUrlInvalid,
        ),
        (too_long.as_str(), ProviderError::BaseUrlInvalid),
        (
            "https://example.com/v1?key=x",
            ProviderError::BaseUrlInvalid,
        ),
        ("https://example.com/v1#part", ProviderError::BaseUrlInvalid),
        ("http://api.deepseek.com", ProviderError::BaseUrlNotHttps),
        ("ftp://example.com", ProviderError::BaseUrlNotHttps),
        (
            "https://user:secret@example.com",
            ProviderError::BaseUrlHasCredentials,
        ),
        (
            "http://token@example.com",
            ProviderError::BaseUrlHasCredentials,
        ),
    ] {
        assert_eq!(normalize_base_url(input), Err(error), "input: {input}");
    }
}

#[test]
fn any_extension_key_is_rejected_until_evidence_adds_one() {
    let mut extensions = no_extensions();
    assert_eq!(
        validate_extensions(ProviderKind::DeepSeek, &extensions),
        Ok(no_extensions())
    );
    extensions.insert("zdr".to_string(), "true".to_string());
    for kind in [
        ProviderKind::CommandCode,
        ProviderKind::DeepSeek,
        ProviderKind::OpenRouter,
    ] {
        assert_eq!(
            validate_extensions(kind, &extensions),
            Err(ProviderError::ExtensionFieldNotSupported)
        );
    }
}

#[test]
fn settings_validation_reports_the_first_invalid_field_in_order() {
    let valid = validate_settings(
        ProviderKind::DeepSeek,
        " Personal ",
        "https://api.deepseek.com/",
        "chat_completions",
        &no_extensions(),
    );
    assert_eq!(
        valid,
        Ok(ProviderSettings {
            display_name: "Personal".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            protocol: ProviderProtocol::ChatCompletions,
            extensions: no_extensions(),
        })
    );
    // Both the name and the URL are invalid; the name is reported first.
    assert_eq!(
        validate_settings(
            ProviderKind::DeepSeek,
            "",
            "http://x",
            "chat_completions",
            &no_extensions()
        ),
        Err(ProviderError::DisplayNameInvalid)
    );
    assert_eq!(
        validate_settings(
            ProviderKind::DeepSeek,
            "Name",
            "https://api.deepseek.com",
            "responses",
            &no_extensions()
        ),
        Err(ProviderError::ProtocolNotSupported)
    );
}

#[test]
fn provider_ids_accept_only_the_generated_format() {
    let valid = "0123456789abcdef0123456789abcdef";
    assert_eq!(
        ProviderId::parse(valid).map(|id| id.as_str().to_string()),
        Some(valid.to_string())
    );
    for invalid in [
        "",
        "0123456789ABCDEF0123456789ABCDEF",
        "0123456789abcdef0123456789abcde",
        "0123456789abcdef0123456789abcdef0",
        "0123456789abcdef0123456789abcdeg",
        "Personal",
    ] {
        assert_eq!(ProviderId::parse(invalid), None, "input: {invalid}");
    }
}

/// A unique temporary app-data folder, removed when the test ends.
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vibemate-providers-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create unique test directory");
        Self(path)
    }

    fn open(&self) -> Storage {
        Storage::open_in_directory(&self.0).expect("open storage")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_request(kind: &str, name: &str) -> CreateProviderRequest {
    CreateProviderRequest {
        kind: kind.to_string(),
        display_name: name.to_string(),
        base_url: kind_default_url(kind).to_string(),
        protocol: "chat_completions".to_string(),
        extensions: no_extensions(),
        secret: Secret::new(SYNTHETIC_KEY.to_string()),
    }
}

/// A made-up key that is easy to search for in files and messages.
const SYNTHETIC_KEY: &str = "vibemate-synthetic-key-7f3a9c";

/// Create with a throwaway in-memory credential store, for tests about SQLite.
fn create(
    storage: &Storage,
    request: CreateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    create_provider(storage, &FakeStore::default(), request, now_ms)
}

/// Count rows in `table` (a fixed name from the test, never user input).
fn table_rows(storage: &Storage, table: &str) -> u32 {
    storage
        .lock()
        .expect("lock")
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count rows")
}

fn kind_default_url(kind: &str) -> &'static str {
    ProviderKind::from_value(kind).map_or("https://example.com", |kind| {
        kind.template().default_base_url
    })
}

fn update_request(record: &ProviderRecord, name: &str, url: &str) -> UpdateProviderRequest {
    UpdateProviderRequest {
        id: record.id.as_str().to_string(),
        expected_revision: record.revision,
        display_name: name.to_string(),
        base_url: url.to_string(),
        protocol: "chat_completions".to_string(),
        extensions: no_extensions(),
    }
}

fn page(storage: &Storage, after: Option<String>, limit: u32) -> ProviderPage {
    list_providers(storage, &ListProvidersRequest { after, limit }).expect("list page")
}

fn row_count(storage: &Storage) -> u32 {
    storage
        .lock()
        .expect("lock")
        .query_row("SELECT COUNT(*) FROM provider_instance", [], |row| {
            row.get(0)
        })
        .expect("count rows")
}

#[test]
fn providers_created_instance_is_read_back_after_restart() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let mut request = create_request("deepseek", "  Personal  ");
    request.base_url = "https://API.deepseek.com/".to_string();
    let created = create(&storage, request, 1_000).expect("create");
    assert_eq!(created.display_name, "Personal");
    assert_eq!(created.base_url, "https://api.deepseek.com");
    assert_eq!(created.kind, ProviderKind::DeepSeek);
    assert_eq!(
        (
            created.revision,
            created.created_at_ms,
            created.updated_at_ms
        ),
        (1, 1_000, 1_000)
    );
    drop(storage);

    let reopened = directory.open();
    assert_eq!(
        get_provider(&reopened, created.id.as_str()),
        Ok(created.clone())
    );
    assert_eq!(page(&reopened, None, 10).items, vec![created]);
}

#[test]
fn providers_with_the_same_kind_and_name_are_separate_instances() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let first = create(&storage, create_request("openrouter", "Work"), 5).expect("first");
    let second = create(&storage, create_request("openrouter", "Work"), 5).expect("second");
    assert_ne!(first.id, second.id);
    assert_eq!(first.id.as_str().len(), 32);
    assert_eq!(row_count(&storage), 2);
    // Editing one instance must not touch the other, even with the same name.
    let edited = update_provider(
        &storage,
        &update_request(&first, "Work", "https://openrouter.ai/api/v2"),
        6,
    )
    .expect("edit first");
    assert_eq!(get_provider(&storage, second.id.as_str()), Ok(second));
    assert_eq!(edited.id, first.id);
}

#[test]
fn providers_edit_keeps_identity_and_increments_the_revision() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("command-code", "Team"), 100).expect("create");
    let edited = update_provider(
        &storage,
        &update_request(
            &created,
            "Renamed",
            "https://api.commandcode.ai/provider/v1/",
        ),
        200,
    )
    .expect("edit");
    assert_eq!(edited.id, created.id);
    assert_eq!(edited.kind, ProviderKind::CommandCode);
    assert_eq!(edited.display_name, "Renamed");
    assert_eq!(edited.base_url, "https://api.commandcode.ai/provider/v1");
    assert_eq!(
        (edited.revision, edited.created_at_ms, edited.updated_at_ms),
        (2, 100, 200)
    );
    drop(storage);
    assert_eq!(
        get_provider(&directory.open(), created.id.as_str()),
        Ok(edited)
    );
}

#[test]
fn providers_edit_with_a_clock_before_creation_keeps_timestamps_ordered() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "A"), 500).expect("create");
    let edited = update_provider(
        &storage,
        &update_request(&created, "B", "https://api.deepseek.com"),
        10,
    )
    .expect("edit despite clock change");
    assert_eq!((edited.created_at_ms, edited.updated_at_ms), (500, 500));
}

#[test]
fn providers_stale_missing_or_malformed_edits_change_nothing() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "Original"), 1).expect("create");
    let current = update_provider(
        &storage,
        &update_request(&created, "Newer", "https://api.deepseek.com"),
        2,
    )
    .expect("first edit");

    // `created` still carries revision 1, as a form opened before the first edit would.
    let stale = update_request(&created, "Stale", "https://api.deepseek.com");
    assert_eq!(
        update_provider(&storage, &stale, 3),
        Err(ProviderError::RevisionConflict)
    );

    let mut missing = update_request(&current, "Missing", "https://api.deepseek.com");
    missing.id = "f".repeat(32);
    assert_eq!(
        update_provider(&storage, &missing, 3),
        Err(ProviderError::NotFound)
    );
    assert_eq!(
        get_provider(&storage, &"f".repeat(32)),
        Err(ProviderError::NotFound)
    );

    let mut by_name = update_request(&current, "Missing", "https://api.deepseek.com");
    by_name.id = "Newer".to_string();
    assert_eq!(
        update_provider(&storage, &by_name, 3),
        Err(ProviderError::InvalidRequest)
    );
    assert_eq!(
        get_provider(&storage, "Newer"),
        Err(ProviderError::InvalidRequest)
    );

    let mut zero_revision = update_request(&current, "Zero", "https://api.deepseek.com");
    zero_revision.expected_revision = 0;
    assert_eq!(
        update_provider(&storage, &zero_revision, 3),
        Err(ProviderError::InvalidRequest)
    );

    let invalid_url = update_request(&current, "Bad URL", "http://api.deepseek.com");
    assert_eq!(
        update_provider(&storage, &invalid_url, 3),
        Err(ProviderError::BaseUrlNotHttps)
    );
    let mut wrong_protocol = update_request(&current, "Bad protocol", "https://api.deepseek.com");
    wrong_protocol.protocol = "responses".to_string();
    assert_eq!(
        update_provider(&storage, &wrong_protocol, 3),
        Err(ProviderError::ProtocolNotSupported)
    );
    let mut extension = update_request(&current, "Extension", "https://api.deepseek.com");
    extension
        .extensions
        .insert("zdr".to_string(), "true".to_string());
    assert_eq!(
        update_provider(&storage, &extension, 3),
        Err(ProviderError::ExtensionFieldNotSupported)
    );

    assert_eq!(get_provider(&storage, created.id.as_str()), Ok(current));
}

#[test]
fn providers_edit_that_updates_no_row_is_a_conflict() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "Original"), 1).expect("create");
    // Simulate another writer winning the race between the revision check and
    // the UPDATE: this test-only trigger makes SQLite skip the row, so the
    // UPDATE reports zero changed rows just as `WHERE revision = ?` would.
    storage
        .lock()
        .expect("lock")
        .execute_batch(
            "CREATE TEMP TRIGGER skip_update BEFORE UPDATE ON provider_instance
             BEGIN SELECT RAISE(IGNORE); END;",
        )
        .expect("trigger");
    assert_eq!(
        update_provider(
            &storage,
            &update_request(&created, "Lost", "https://api.deepseek.com"),
            2
        ),
        Err(ProviderError::RevisionConflict)
    );
    assert_eq!(get_provider(&storage, created.id.as_str()), Ok(created));
}

#[test]
fn providers_create_saves_the_key_and_its_reference_together() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    let mut request = create_request("deepseek", "Keyed");
    request.secret = Secret::new(format!("  {SYNTHETIC_KEY}\n"));

    let created = create_provider(&storage, &store, request, 42).expect("create");

    let reference = format!("provider-{}", created.id.as_str());
    assert_eq!(created.id.credential_reference(), reference);
    // The key is trimmed and stored only under the ID-based entry name.
    assert_eq!(store.stored(&reference).as_deref(), Some(SYNTHETIC_KEY));
    let (stored_ref, updated_at): (String, i64) = storage
        .lock()
        .expect("lock")
        .query_row(
            "SELECT credential_ref, updated_at FROM provider_credential WHERE provider_id = ?1",
            [created.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("reference row");
    assert_eq!((stored_ref, updated_at), (reference, 42));
}

#[test]
fn providers_invalid_keys_are_rejected_before_any_store_is_touched() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    for key in ["", "   ", "line\nbreak", "tab\tinside"] {
        let mut request = create_request("deepseek", "Keyed");
        request.secret = Secret::new(key.to_string());
        assert_eq!(
            create_provider(&storage, &store, request, 1),
            Err(ProviderError::SecretInvalid),
            "key: {key:?}"
        );
    }
    // Other fields are checked first, so the form shows their error first.
    let mut both_invalid = create_request("deepseek", " ");
    both_invalid.secret = Secret::new(String::new());
    assert_eq!(
        create_provider(&storage, &store, both_invalid, 1),
        Err(ProviderError::DisplayNameInvalid)
    );
    assert_eq!(store.call_count(), 0);
    assert_eq!(table_rows(&storage, "provider_instance"), 0);
}

#[test]
fn providers_credential_store_failure_saves_nothing() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    for (failure, code) in [
        (
            CredentialError::Unavailable,
            ProviderError::CredentialStoreUnavailable,
        ),
        (
            CredentialError::AccessDenied,
            ProviderError::CredentialStoreAccessDenied,
        ),
        (
            CredentialError::OperationFailed,
            ProviderError::CredentialStoreFailed,
        ),
    ] {
        let store = FakeStore::default();
        store.fail_saves(Some(failure));
        assert_eq!(
            create_provider(&storage, &store, create_request("deepseek", "A"), 1),
            Err(code)
        );
    }
    assert_eq!(table_rows(&storage, "provider_instance"), 0);
    assert_eq!(table_rows(&storage, "provider_credential"), 0);
}

/// Make the reference insert fail, the second write in the create transaction.
fn fail_reference_inserts(storage: &Storage) {
    storage
        .lock()
        .expect("lock")
        .execute_batch(
            "CREATE TEMP TRIGGER fail_reference BEFORE INSERT ON provider_credential
             BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;",
        )
        .expect("trigger");
}

#[test]
fn providers_database_failure_after_the_key_write_removes_the_key() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    fail_reference_inserts(&storage);

    assert_eq!(
        create_provider(&storage, &store, create_request("deepseek", "A"), 1),
        Err(ProviderError::WriteFailed)
    );
    // The instance row was inserted first; one transaction rolled both back.
    assert_eq!(table_rows(&storage, "provider_instance"), 0);
    assert_eq!(table_rows(&storage, "provider_credential"), 0);
    // save, then the compensating delete.
    assert_eq!(store.call_count(), 2);
    assert!(!store.holds_value(SYNTHETIC_KEY));
}

#[test]
fn providers_failed_cleanup_after_a_database_failure_is_an_unknown_outcome() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    fail_reference_inserts(&storage);
    store.fail_deletes(Some(CredentialError::OperationFailed));

    assert_eq!(
        create_provider(&storage, &store, create_request("deepseek", "A"), 1),
        Err(ProviderError::CreateOutcomeUnknown)
    );
    assert_eq!(table_rows(&storage, "provider_instance"), 0);
    // The unreferenced entry remains; no row points at it, so it is never read.
    // The generated ID is not returned on failure, so look for the value.
    assert!(store.holds_value(SYNTHETIC_KEY));
}

#[test]
fn providers_failed_commit_keeps_the_key_and_reports_an_unknown_outcome() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let store = FakeStore::default();
    fail_commits_after_reference_writes(&storage);

    assert_eq!(
        create_provider(&storage, &store, create_request("deepseek", "A"), 1),
        Err(ProviderError::CreateOutcomeUnknown)
    );
    // COMMIT failed, so no compensating delete ran: save is the only call.
    assert_eq!(store.call_count(), 1);
    assert!(store.holds_value(SYNTHETIC_KEY));
    // In this simulation SQLite rolled back; the kept entry is unreferenced.
    assert_eq!(table_rows(&storage, "provider_instance"), 0);
}

#[test]
fn providers_key_never_reaches_the_database_file_or_debug_output() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let request = create_request("deepseek", "Searchable-name");
    assert!(!format!("{request:?}").contains(SYNTHETIC_KEY));
    create(&storage, request, 1).expect("create");
    drop(storage);

    let mut saw_name = false;
    for entry in fs::read_dir(&directory.0).expect("list app-data folder") {
        let bytes = fs::read(entry.expect("entry").path()).expect("read file");
        assert!(
            !bytes
                .windows(SYNTHETIC_KEY.len())
                .any(|window| window == SYNTHETIC_KEY.as_bytes()),
            "the key was written to a database file"
        );
        saw_name |= bytes
            .windows("Searchable-name".len())
            .any(|window| window == b"Searchable-name");
    }
    // Proves the scan read the real database contents.
    assert!(saw_name);
}

#[test]
fn providers_invalid_creations_store_nothing() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let mut unknown_kind = create_request("custom", "Custom");
    unknown_kind.base_url = "https://example.com".to_string();
    assert_eq!(
        create(&storage, unknown_kind, 1),
        Err(ProviderError::KindNotSupported)
    );
    let mut credentials = create_request("deepseek", "Leaky");
    credentials.base_url = "https://key:secret@api.deepseek.com".to_string();
    assert_eq!(
        create(&storage, credentials, 1),
        Err(ProviderError::BaseUrlHasCredentials)
    );
    assert_eq!(
        create(&storage, create_request("deepseek", " "), 1),
        Err(ProviderError::DisplayNameInvalid)
    );
    assert_eq!(row_count(&storage), 0);
}

#[test]
fn providers_pages_are_bounded_and_ordered_by_creation_then_id() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    assert_eq!(
        page(&storage, None, 3),
        ProviderPage {
            items: Vec::new(),
            next_cursor: None
        }
    );
    // Three instances share one millisecond, so only the ID can order them.
    let mut expected: Vec<ProviderRecord> = [20, 10, 10, 10, 30]
        .into_iter()
        .map(|now| create(&storage, create_request("deepseek", "Same"), now).expect("create"))
        .collect();
    expected.sort_by(|left, right| {
        (left.created_at_ms, left.id.as_str()).cmp(&(right.created_at_ms, right.id.as_str()))
    });

    let mut seen = Vec::new();
    let mut cursor = None;
    let mut page_sizes = Vec::new();
    loop {
        let current = page(&storage, cursor, 2);
        page_sizes.push(current.items.len());
        seen.extend(current.items);
        match current.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(page_sizes, vec![2, 2, 1]);
    assert_eq!(seen, expected);

    // A page that exactly fits the remaining rows reports no further page.
    assert_eq!(page(&storage, None, 5).next_cursor, None);
    assert_eq!(page(&storage, None, MAX_PAGE_SIZE).items.len(), 5);
}

#[test]
fn providers_instances_added_between_pages_are_not_repeated() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    for now in [1, 2, 3] {
        create(&storage, create_request("openrouter", "Item"), now).expect("create");
    }
    let first = page(&storage, None, 2);
    // An older instance (another window with a slow clock) and a newer one.
    create(&storage, create_request("openrouter", "Older"), 0).expect("older");
    let newer = create(&storage, create_request("openrouter", "Newer"), 9).expect("newer");
    let second = page(&storage, first.next_cursor, 2);
    let first_ids: Vec<_> = first.items.iter().map(|record| record.id.clone()).collect();
    assert!(
        second
            .items
            .iter()
            .all(|record| !first_ids.contains(&record.id))
    );
    assert_eq!(second.items.last(), Some(&newer));
    assert_eq!(second.items.len(), 2);
}

#[test]
fn providers_page_size_and_cursor_are_validated() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let valid_id = "a".repeat(32);
    for (after, limit) in [
        (None, 0),
        (None, MAX_PAGE_SIZE + 1),
        (Some(String::new()), 10),
        (Some("12".to_string()), 10),
        (Some(format!("x.{valid_id}")), 10),
        (Some("12.Personal".to_string()), 10),
        (Some(format!("+5.{valid_id}")), 10),
        (Some(format!("-5.{valid_id}")), 10),
        (Some(format!(" 5.{valid_id}")), 10),
        (Some(format!("５.{valid_id}")), 10),
        (Some(format!(".{valid_id}")), 10),
        (Some(format!("99999999999999999999.{valid_id}")), 10),
    ] {
        assert_eq!(
            list_providers(&storage, &ListProvidersRequest { after, limit }),
            Err(ProviderError::InvalidRequest)
        );
    }
    assert!(
        list_providers(
            &storage,
            &ListProvidersRequest {
                after: Some(format!("12.{valid_id}")),
                limit: 1
            }
        )
        .is_ok()
    );
}

#[test]
fn providers_invalid_stored_rows_are_reported_and_left_unchanged() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "Kept"), 1).expect("create");
    storage
        .lock()
        .expect("lock")
        .execute_batch("PRAGMA ignore_check_constraints = ON; UPDATE provider_instance SET kind = 'future-kind'; PRAGMA ignore_check_constraints = OFF;")
        .expect("simulate a row from a newer build");
    assert_eq!(
        get_provider(&storage, created.id.as_str()),
        Err(ProviderError::InvalidStoredProvider)
    );
    assert_eq!(
        list_providers(
            &storage,
            &ListProvidersRequest {
                after: None,
                limit: 10
            }
        ),
        Err(ProviderError::InvalidStoredProvider)
    );
    assert_eq!(
        update_provider(
            &storage,
            &update_request(&created, "Overwrite", "https://api.deepseek.com"),
            2
        ),
        Err(ProviderError::InvalidStoredProvider)
    );
    let kind: String = storage
        .lock()
        .expect("lock")
        .query_row("SELECT kind FROM provider_instance", [], |row| row.get(0))
        .expect("stored kind");
    assert_eq!(kind, "future-kind");
}

#[test]
fn providers_failed_writes_keep_the_previous_data() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "Before"), 1).expect("create");
    // query_only makes SQLite reject writes without platform-specific permissions.
    storage
        .lock()
        .expect("lock")
        .pragma_update(None, "query_only", true)
        .expect("read-only");
    assert_eq!(
        create(&storage, create_request("deepseek", "Second"), 2),
        Err(ProviderError::WriteFailed)
    );
    assert_eq!(
        update_provider(
            &storage,
            &update_request(&created, "After", "https://api.deepseek.com"),
            2
        ),
        Err(ProviderError::WriteFailed)
    );
    assert_eq!(get_provider(&storage, created.id.as_str()), Ok(created));
    assert_eq!(row_count(&storage), 1);
}

#[test]
fn providers_poisoned_storage_rejects_every_operation() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let created = create(&storage, create_request("deepseek", "A"), 1).expect("create");
    std::thread::scope(|scope| {
        let result = scope
            .spawn(|| {
                let _connection = storage.lock().expect("lock");
                panic!("synthetic interrupted operation");
            })
            .join();
        assert!(result.is_err());
    });
    let unavailable = Some(ProviderError::StorageUnavailable);
    assert_eq!(
        create(&storage, create_request("deepseek", "B"), 2).err(),
        unavailable
    );
    assert_eq!(
        update_provider(
            &storage,
            &update_request(&created, "C", "https://api.deepseek.com"),
            2
        )
        .err(),
        unavailable
    );
    assert_eq!(
        get_provider(&storage, created.id.as_str()).err(),
        unavailable
    );
    assert_eq!(
        list_providers(
            &storage,
            &ListProvidersRequest {
                after: None,
                limit: 1
            }
        )
        .err(),
        unavailable
    );
}

#[test]
fn providers_schema_rejects_rows_that_bypass_validation() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    let connection = storage.lock().expect("lock");
    let valid_id = "b".repeat(32);
    for (id, kind, protocol, revision) in [
        ("Personal", "deepseek", "chat_completions", 1),
        (valid_id.as_str(), "custom", "chat_completions", 1),
        (valid_id.as_str(), "deepseek", "openai-completions", 1),
        (valid_id.as_str(), "deepseek", "chat_completions", 0),
    ] {
        assert!(
            connection
                .execute(
                    "INSERT INTO provider_instance VALUES (?1, ?2, 'Name', 'https://x', ?3, ?4, 1, 1)",
                    params![id, kind, protocol, revision],
                )
                .is_err(),
            "row: {id} {kind} {protocol} {revision}"
        );
    }
}

#[test]
fn providers_update_request_rejects_a_kind_field() {
    use serde::de::value::{Error, MapDeserializer};
    // A kind sent with an edit must fail loudly instead of being ignored.
    let input = MapDeserializer::<_, Error>::new(vec![("kind", "openrouter")].into_iter());
    let error = UpdateProviderRequest::deserialize(input).expect_err("kind is not editable");
    assert!(
        error.to_string().contains("unknown field `kind`"),
        "{error}"
    );
}
