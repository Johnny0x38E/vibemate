//! Behavior tests for the parent module, kept separate from runtime code.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::json;

use super::*;
use crate::credentials::CredentialError;
use crate::credentials::fake::FakeStore;
use crate::http_client::HttpError;
use crate::http_client::test_server::{MockServer, Reply, response};
use crate::model_catalog::RouteSupport;
use crate::models::{
    AddManualModelRequest, ListProviderModelsRequest, ModelSource, ProviderModel,
    SetModelsSelectedRequest, UpstreamState, add_manual_model, list_provider_models,
    set_models_selected,
};

const PROVIDER: &str = "0123456789abcdef0123456789abcdef";
/// A made-up key that is easy to search for in files and messages.
const KEY: &str = "sk-vibemate-SYNTHETIC-fetch-key-4b1d";
const COMMAND_CODE_FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/command-code-models-2026-10-10.json");

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vibemate-model-fetch-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn open(&self) -> Storage {
        Storage::open_in_directory(&self.0).expect("open storage")
    }

    fn database_bytes(&self) -> Vec<u8> {
        fs::read(self.0.join("vibemate.sqlite3")).expect("read database file")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn provider_id() -> ProviderId {
    ProviderId::parse(PROVIDER).unwrap()
}

fn reference() -> String {
    provider_id().credential_reference()
}

fn key_store() -> FakeStore {
    FakeStore::holding(&reference(), KEY)
}

/// Insert a provider (revision 1) and its key reference directly.
fn insert_provider(storage: &Storage, kind: &str, protocol: &str, base_url: &str) {
    let connection = storage.lock().unwrap();
    connection
        .execute(
            "INSERT INTO provider_instance
                 (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
             VALUES (?1, ?2, 'Test', ?3, ?4, 1, 1, 1)",
            params![PROVIDER, kind, base_url, protocol],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
             VALUES (?1, ?2, 1)",
            params![PROVIDER, reference()],
        )
        .unwrap();
}

fn ok_json(body: &serde_json::Value) -> Reply {
    Reply::Respond(response(
        200,
        &[("Content-Type", "application/json")],
        body.to_string().as_bytes(),
    ))
}

fn deepseek_body(ids: &[&str]) -> serde_json::Value {
    json!({
        "object": "list",
        "data": ids.iter().map(|id| json!({
            "id": id, "object": "model", "owned_by": "deepseek",
            "context_window": 1_000_000, "max_output_tokens": 384_000,
            "input_modalities": ["text"], "output_modalities": ["text"]
        })).collect::<Vec<_>>()
    })
}

fn openrouter_body(ids: &[&str], more: bool) -> serde_json::Value {
    json!({
        "data": ids.iter().map(|id| json!({
            "id": id, "name": format!("Name {id}"), "context_length": 128_000,
            "architecture": {"input_modalities": ["text", "image"], "output_modalities": ["text"]},
            "top_provider": {"max_completion_tokens": 4096}
        })).collect::<Vec<_>>(),
        "total_count": 5,
        "links": {"next": if more { json!("/api/v1/models?offset=x") } else { json!(null) }}
    })
}

fn fetcher() -> ModelFetcher {
    ModelFetcher::for_mock_server(Duration::from_secs(20))
}

async fn fetch(
    storage: &Storage,
    store: &FakeStore,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    fetch_provider_models(
        storage,
        store,
        &fetcher(),
        &ModelFetchRegistry::new(),
        PROVIDER,
        now_ms,
    )
    .await
}

fn all_models(storage: &Storage) -> Vec<ProviderModel> {
    list_provider_models(
        storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: None,
            limit: 200,
            filter: "all".to_owned(),
            query: None,
        },
    )
    .unwrap()
    .items
}

fn model<'a>(models: &'a [ProviderModel], id: &str) -> Option<&'a ProviderModel> {
    models.iter().find(|model| model.model_id == id)
}

fn count_rows(storage: &Storage, table: &str) -> i64 {
    storage
        .lock()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn assert_nothing_written(storage: &Storage) {
    assert_eq!(count_rows(storage, "provider_model"), 0);
    assert_eq!(count_rows(storage, "provider_model_fetch"), 0);
}

/// Whether the server's request heads carried the key as a Bearer token.
fn sent_bearer(server: &MockServer) -> bool {
    let expected = format!("authorization: bearer {KEY}").to_lowercase();
    server
        .requests()
        .iter()
        .all(|head| head.to_lowercase().contains(&expected))
}

fn select(storage: &Storage, ids: &[&str]) {
    set_models_selected(
        storage,
        &SetModelsSelectedRequest {
            provider_id: PROVIDER.to_owned(),
            model_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            selected: true,
        },
        50,
    )
    .unwrap();
}

async fn wait_for_requests(server: &MockServer, count: usize) {
    let start = Instant::now();
    while server.requests().len() < count {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "request never arrived"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_close(server: &MockServer) {
    let start = Instant::now();
    while server.closed.try_recv().is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "connection never closed"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn command_code_fixture_is_fetched_with_the_key_and_stored_unselected() {
    let server = MockServer::start(vec![Reply::Respond(response(
        200,
        &[("Content-Type", "application/json")],
        COMMAND_CODE_FIXTURE,
    ))]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    let base = server.url("/provider/v1").to_string();
    insert_provider(&storage, "command-code", "chat_completions", &base);

    let summary = fetch(&storage, &key_store(), 1_000).await.unwrap();
    assert_eq!(
        summary,
        ModelFetchSummary {
            complete: true,
            incomplete_reason: None,
            listed: 87,
            added: 87,
            updated: 0,
            marked_missing: 0,
            pruned: 0,
            skipped_invalid: 0,
            fetched_at_ms: 1_000,
        }
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /provider/v1/models HTTP/1.1"));
    assert!(sent_bearer(&server));

    let mut models = all_models(&storage);
    let selected_page = list_provider_models(
        &storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: None,
            limit: 200,
            filter: "selected".to_owned(),
            query: None,
        },
    )
    .unwrap();
    assert!(
        selected_page.items.is_empty(),
        "new models are not selected"
    );
    assert_eq!(models.len(), 87);
    assert!(
        models
            .iter()
            .all(|model| model.source == ModelSource::Fetched
                && model.upstream_state == UpstreamState::Listed
                && model.last_seen_at_ms == Some(1_000))
    );
    assert_eq!(
        models
            .iter()
            .filter(|model| model.model_id.contains('/'))
            .count(),
        65
    );
    let supported = models
        .iter()
        .filter(|model| model.route_support == RouteSupport::Supported)
        .count();
    assert_eq!(supported, 76);
    models.retain(|model| model.route_support == RouteSupport::Unsupported);
    assert_eq!(models.len(), 11);

    let status = fetch_status(&storage, &ModelFetchRegistry::new(), PROVIDER).unwrap();
    assert_eq!(
        status,
        ModelFetchStatus {
            running: false,
            last_fetch: Some(LastModelFetch {
                fetched_at_ms: 1_000,
                complete: true,
                listed_count: 87
            }),
        }
    );
}

#[tokio::test]
async fn deepseek_list_maps_metadata() {
    let server = MockServer::start(vec![ok_json(&deepseek_body(&[
        "deepseek-v4-flash",
        "deepseek-v4-pro",
    ]))]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );

    let summary = fetch(&storage, &key_store(), 10).await.unwrap();
    assert_eq!(
        (summary.listed, summary.added, summary.complete),
        (2, 2, true)
    );
    assert!(server.requests()[0].starts_with("GET /models HTTP/1.1"));
    assert!(sent_bearer(&server));
    let models = all_models(&storage);
    let flash = model(&models, "deepseek-v4-flash").unwrap();
    assert_eq!(flash.context_window, Some(1_000_000));
    assert_eq!(flash.max_output_tokens, Some(384_000));
    assert_eq!(
        flash.input_modalities.as_deref(),
        Some(&["text".to_owned()][..])
    );
    assert_eq!(flash.route_support, RouteSupport::NotApplicable);
}

#[tokio::test]
async fn openrouter_pages_are_requested_with_computed_offsets() {
    let server = MockServer::start(vec![
        ok_json(&openrouter_body(&["a/one", "a/two"], true)),
        ok_json(&openrouter_body(&["b/three", "a/one"], true)),
        ok_json(&openrouter_body(&["c/four"], false)),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "openrouter",
        "chat_completions",
        server.url("/api/v1").as_str(),
    );

    let summary = fetch(&storage, &key_store(), 10).await.unwrap();
    assert_eq!(
        (summary.listed, summary.added, summary.complete),
        (4, 4, true)
    );
    assert_eq!(summary.skipped_invalid, 1, "the repeated a/one is counted");
    let lines: Vec<String> = server
        .requests()
        .iter()
        .map(|head| head.lines().next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        lines,
        [
            "GET /api/v1/models?offset=0&limit=500 HTTP/1.1",
            "GET /api/v1/models?offset=2&limit=500 HTTP/1.1",
            "GET /api/v1/models?offset=4&limit=500 HTTP/1.1",
        ]
    );
    assert!(sent_bearer(&server));
    let models = all_models(&storage);
    let one = model(&models, "a/one").unwrap();
    assert_eq!(one.upstream_name.as_deref(), Some("Name a/one"));
    assert_eq!(one.max_output_tokens, Some(4096));
    assert_eq!(
        one.output_modalities.as_deref(),
        Some(&["text".to_owned()][..])
    );
}

#[tokio::test]
async fn openrouter_page_cap_makes_the_fetch_incomplete_and_prunes_nothing() {
    let replies = (0..10)
        .map(|page| ok_json(&openrouter_body(&[&format!("m/{page}")], true)))
        .collect();
    let server = MockServer::start(replies);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "openrouter",
        "chat_completions",
        server.url("/api/v1").as_str(),
    );
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model (provider_id, model_id, source, selected,
                 created_at, updated_at, last_seen_at)
             VALUES (?1, 'old/unselected', 'fetched', 0, 1, 1, 1)",
            [PROVIDER],
        )
        .unwrap();

    let summary = fetch(&storage, &key_store(), 10).await.unwrap();
    assert_eq!(server.requests().len(), 10);
    assert!(!summary.complete);
    assert_eq!(summary.incomplete_reason, Some(IncompleteReason::PageLimit));
    assert_eq!(
        (summary.added, summary.pruned, summary.marked_missing),
        (10, 0, 0)
    );
    let models = all_models(&storage);
    let old = model(&models, "old/unselected").expect("kept by an incomplete fetch");
    assert_eq!(old.upstream_state, UpstreamState::Listed);
    let status = fetch_status(&storage, &ModelFetchRegistry::new(), PROVIDER).unwrap();
    assert_eq!(status.last_fetch.map(|last| last.complete), Some(false));
}

#[tokio::test]
async fn merging_across_fetches_keeps_selected_and_manual_models() {
    let server = MockServer::start(vec![
        ok_json(&deepseek_body(&["a", "b", "c", "x"])),
        ok_json(&deepseek_body(&["a", "d"])),
        ok_json(&deepseek_body(&["a", "b", "d"])),
        ok_json(&deepseek_body(&[])),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let store = key_store();
    for (id, alias) in [("x", Some("Mine")), ("never", None)] {
        add_manual_model(
            &storage,
            &AddManualModelRequest {
                provider_id: PROVIDER.to_owned(),
                model_id: id.to_owned(),
                alias: alias.map(str::to_owned),
            },
            5,
        )
        .unwrap();
    }

    // Fetch 1: x is listed and gets metadata, but stays manual with its alias.
    let first = fetch(&storage, &store, 100).await.unwrap();
    assert_eq!((first.listed, first.added, first.updated), (4, 3, 1));
    let models = all_models(&storage);
    let x = model(&models, "x").unwrap();
    assert_eq!(x.source, ModelSource::Manual);
    assert_eq!(x.alias.as_deref(), Some("Mine"));
    assert!(x.selected);
    assert_eq!(x.context_window, Some(1_000_000));
    assert_eq!(x.upstream_state, UpstreamState::Listed);
    assert!(
        !model(&models, "a").unwrap().selected,
        "new models are not selected"
    );
    select(&storage, &["a", "b"]);

    // Fetch 2 lists only a and d.
    let second = fetch(&storage, &store, 200).await.unwrap();
    assert_eq!(
        (
            second.added,
            second.updated,
            second.marked_missing,
            second.pruned
        ),
        (1, 1, 2, 1)
    );
    let models = all_models(&storage);
    let ids: Vec<&str> = models.iter().map(|model| model.model_id.as_str()).collect();
    assert_eq!(
        ids,
        ["a", "b", "d", "never", "x"],
        "unselected c was pruned"
    );
    let b = model(&models, "b").unwrap();
    assert!(b.selected, "a selected model is never deleted");
    assert_eq!(b.upstream_state, UpstreamState::Missing);
    assert_eq!(b.missing_since_ms, Some(200));
    assert_eq!(
        model(&models, "x").unwrap().upstream_state,
        UpstreamState::Missing
    );
    let never = model(&models, "never").unwrap();
    assert_eq!(never.upstream_state, UpstreamState::NeverListed);
    assert_eq!(
        never.updated_at_ms, 5,
        "a never-listed manual row is untouched"
    );
    assert!(!model(&models, "d").unwrap().selected);

    // Fetch 3: b is back, x stays missing since its first absence.
    let third = fetch(&storage, &store, 300).await.unwrap();
    assert_eq!(
        (third.added, third.updated, third.marked_missing),
        (0, 3, 0)
    );
    let models = all_models(&storage);
    let b = model(&models, "b").unwrap();
    assert_eq!(
        (b.upstream_state, b.missing_since_ms, b.last_seen_at_ms),
        (UpstreamState::Listed, None, Some(300))
    );
    assert!(b.selected);
    assert_eq!(model(&models, "x").unwrap().missing_since_ms, Some(200));

    // Fetch 4: an empty list is incomplete and changes nothing.
    let fourth = fetch(&storage, &store, 400).await.unwrap();
    assert_eq!(fourth.incomplete_reason, Some(IncompleteReason::EmptyList));
    assert_eq!((fourth.pruned, fourth.marked_missing), (0, 0));
    assert_eq!(all_models(&storage), models);
}

#[tokio::test]
async fn cancelling_closes_the_request_and_writes_nothing() {
    let server = MockServer::start(vec![Reply::Hang]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let store = key_store();
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();

    let started = Instant::now();
    let (result, cancelled) = tokio::join!(
        fetch_provider_models(&storage, &store, &fetcher, &registry, PROVIDER, 10),
        async {
            wait_for_requests(&server, 1).await;
            assert!(registry.is_running(&provider_id()));
            // A second fetch for the same provider is refused meanwhile.
            let second =
                fetch_provider_models(&storage, &store, &fetcher, &registry, PROVIDER, 10).await;
            assert_eq!(second, Err(ModelError::ModelFetchInProgress));
            registry.cancel(&provider_id())
        }
    );
    assert!(cancelled, "the download was still running");
    assert_eq!(result, Err(ModelError::ModelFetchCancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
    wait_for_close(&server).await;
    assert_nothing_written(&storage);
    assert!(
        !registry.is_running(&provider_id()),
        "the registration was released"
    );
    assert!(!registry.cancel(&provider_id()), "nothing left to cancel");
}

#[tokio::test]
async fn the_overall_deadline_stops_a_hanging_fetch() {
    let server = MockServer::start(vec![Reply::Hang]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let started = Instant::now();
    let result = fetch_provider_models(
        &storage,
        &key_store(),
        &ModelFetcher::for_mock_server(Duration::from_millis(300)),
        &ModelFetchRegistry::new(),
        PROVIDER,
        10,
    )
    .await;
    assert_eq!(result, Err(ModelError::RequestTimedOut));
    assert!(started.elapsed() < Duration::from_secs(5));
    wait_for_close(&server).await;
    assert_nothing_written(&storage);
}

#[tokio::test]
async fn error_statuses_and_bad_bodies_map_to_codes_without_writes() {
    for (reply, expected) in [
        (
            Reply::Respond(response(401, &[], b"")),
            ModelError::AuthRejected,
        ),
        (
            Reply::Respond(response(403, &[], b"")),
            ModelError::AuthRejected,
        ),
        (
            Reply::Respond(response(402, &[], b"")),
            ModelError::InsufficientBalance,
        ),
        (
            Reply::Respond(response(429, &[], b"")),
            ModelError::RateLimited,
        ),
        (
            Reply::Respond(response(500, &[], b"")),
            ModelError::UpstreamUnavailable,
        ),
        (
            Reply::Respond(response(503, &[], b"")),
            ModelError::UpstreamUnavailable,
        ),
        (
            Reply::Respond(response(302, &[("Location", "http://127.0.0.1:9/")], b"")),
            ModelError::UpstreamResponseInvalid,
        ),
        (
            Reply::Respond(response(200, &[], b"<html>not json</html>")),
            ModelError::UpstreamResponseInvalid,
        ),
        (Reply::Close, ModelError::ConnectionFailed),
    ] {
        let server = MockServer::start(vec![reply]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let error = fetch(&storage, &key_store(), 10).await.unwrap_err();
        assert_eq!(error, expected);
        for text in [
            error.to_string(),
            format!("{error:?}"),
            serde_json::to_string(&error).unwrap(),
        ] {
            assert!(!text.contains(KEY), "{text}");
        }
        assert_nothing_written(&storage);
    }
}

#[tokio::test]
async fn settings_changed_during_the_fetch_discard_the_result() {
    for change in [
        "UPDATE provider_instance SET revision = 2",
        "UPDATE provider_instance SET base_url = 'https://other.example.com'",
        "DELETE FROM provider_instance",
    ] {
        let server = MockServer::start(vec![ok_json(&deepseek_body(&["a"]))]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
        let (_sender, cancel) = oneshot::channel();
        let catalog = download_catalog(&fetcher(), &prepared, cancel)
            .await
            .unwrap();
        storage.lock().unwrap().execute(change, []).unwrap();

        let result = merge_catalog(&storage, &prepared.snapshot, &catalog, 10);
        let expected = if change.starts_with("DELETE") {
            ModelError::NotFound
        } else {
            ModelError::ModelFetchStale
        };
        assert_eq!(result, Err(expected), "{change}");
        assert_nothing_written(&storage);
    }
}

#[tokio::test]
async fn a_missing_or_unreadable_key_stops_before_any_request() {
    let server = MockServer::start(vec![]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );

    // A reference without a stored key.
    assert_eq!(
        fetch(&storage, &FakeStore::default(), 10).await,
        Err(ModelError::SecretMissing)
    );
    let locked = key_store();
    locked.fail_loads(Some(CredentialError::AccessDenied));
    assert_eq!(
        fetch(&storage, &locked, 10).await,
        Err(ModelError::CredentialStoreAccessDenied)
    );
    locked.fail_loads(Some(CredentialError::Unavailable));
    assert_eq!(
        fetch(&storage, &locked, 10).await,
        Err(ModelError::CredentialStoreUnavailable)
    );
    // No reference at all (a provider saved before keys existed).
    storage
        .lock()
        .unwrap()
        .execute("DELETE FROM provider_credential", [])
        .unwrap();
    assert_eq!(
        fetch(&storage, &key_store(), 10).await,
        Err(ModelError::SecretMissing)
    );
    // Unknown provider and malformed ID.
    storage
        .lock()
        .unwrap()
        .execute("DELETE FROM provider_instance", [])
        .unwrap();
    assert_eq!(
        fetch(&storage, &key_store(), 10).await,
        Err(ModelError::NotFound)
    );
    let malformed = fetch_provider_models(
        &storage,
        &key_store(),
        &fetcher(),
        &ModelFetchRegistry::new(),
        "not-an-id",
        10,
    )
    .await;
    assert_eq!(malformed, Err(ModelError::InvalidRequest));
    assert!(server.requests().is_empty());
    assert_nothing_written(&storage);
}

#[tokio::test]
async fn an_unusable_stored_base_url_is_a_stored_data_error() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, "deepseek", "chat_completions", "not a url");
    assert_eq!(
        fetch(&storage, &key_store(), 10).await,
        Err(ModelError::InvalidStoredProvider)
    );
    assert_nothing_written(&storage);
}

#[tokio::test]
async fn the_key_never_reaches_the_database_or_any_output() {
    let server = MockServer::start(vec![
        ok_json(&deepseek_body(&["a"])),
        Reply::Respond(response(401, &[], KEY.as_bytes())),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let summary = fetch(&storage, &key_store(), 10).await.unwrap();
    // The second response echoes the key in its body; the body is never read.
    let error = fetch(&storage, &key_store(), 20).await.unwrap_err();
    assert_eq!(error, ModelError::AuthRejected);
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    drop(storage);

    let database = directory.database_bytes();
    assert!(
        !database
            .windows(KEY.len())
            .any(|window| window == KEY.as_bytes())
    );
    for text in [
        format!("{summary:?}"),
        serde_json::to_string(&summary).unwrap(),
        format!("{error:?} {error}"),
        format!("{prepared:?}"),
    ] {
        assert!(!text.contains(KEY), "{text}");
    }
}

#[test]
fn the_registry_allows_one_fetch_per_provider_and_stops_cancel_once_merging() {
    let registry = ModelFetchRegistry::new();
    let other = ProviderId::parse("fedcba9876543210fedcba9876543210").unwrap();
    assert!(!registry.cancel(&provider_id()));
    let (registration, mut cancel) = registry.begin(&provider_id()).unwrap();
    assert_eq!(
        registry.begin(&provider_id()).map(|_| ()),
        Err(ModelError::ModelFetchInProgress)
    );
    assert!(
        registry.begin(&other).is_ok(),
        "other providers are independent"
    );
    assert!(
        registry.clone().is_running(&provider_id()),
        "clones share state"
    );

    registration.enter_merge();
    assert!(!registry.cancel(&provider_id()), "too late once merging");
    assert!(cancel.try_recv().is_err());
    drop(registration);
    assert!(!registry.is_running(&provider_id()));

    let (_registration, mut cancel) = registry.begin(&provider_id()).unwrap();
    assert!(registry.cancel(&provider_id()));
    assert_eq!(cancel.try_recv(), Ok(()));
    assert!(
        !registry.cancel(&provider_id()),
        "a signal is sent only once"
    );
}

#[test]
fn network_errors_keep_their_http_codes() {
    for error in [
        HttpError::ClientUnavailable,
        HttpError::InvalidUrl,
        HttpError::SecretInvalid,
        HttpError::ConnectionFailed,
        HttpError::TlsFailed,
        HttpError::RequestTimedOut,
        HttpError::AuthRejected,
        HttpError::InsufficientBalance,
        HttpError::RateLimited,
        HttpError::UpstreamUnavailable,
        HttpError::UpstreamResponseInvalid,
        HttpError::ResponseTooLarge,
        HttpError::Cancelled,
    ] {
        let model_error = ModelError::from(error);
        assert_eq!(model_error.code(), error.code());
        assert_eq!(serde_json::to_value(model_error).unwrap(), error.code());
    }
}
