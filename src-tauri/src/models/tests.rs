//! Behavior tests for the parent module, kept separate from runtime code.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// A unique temporary app-data folder, removed when the test ends.
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vibemate-models-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test directory");
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

const PROVIDER: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210";

/// Insert a provider row directly, so tests choose kind and protocol freely.
fn insert_provider(storage: &Storage, id: &str, kind: &str, protocol: &str) {
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_instance
                 (id, kind, display_name, base_url, protocol, revision, created_at, updated_at)
             VALUES (?1, ?2, 'Test', 'https://example.com', ?3, 1, 1, 1)",
            params![id, kind, protocol],
        )
        .unwrap();
}

/// Insert a fetched model row as a later merge (P12.a.4) would.
fn insert_fetched(storage: &Storage, provider: &str, model_id: &str, endpoints: Option<&str>) {
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, upstream_name, context_window,
                  supported_endpoints, created_at, updated_at, last_seen_at)
             VALUES (?1, ?2, 'fetched', 0, 'Upstream', 1000, ?3, 5, 5, 5)",
            params![provider, model_id, endpoints],
        )
        .unwrap();
}

fn list(storage: &Storage, after: Option<String>, limit: u32, filter: &str) -> ProviderModelPage {
    list_provider_models(
        storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after,
            limit,
            filter: filter.to_owned(),
            query: None,
        },
    )
    .expect("list")
}

fn add(
    storage: &Storage,
    model_id: &str,
    alias: Option<&str>,
) -> Result<ProviderModel, ModelError> {
    add_manual_model(
        storage,
        &AddManualModelRequest {
            provider_id: PROVIDER.to_owned(),
            model_id: model_id.to_owned(),
            alias: alias.map(str::to_owned),
        },
        100,
    )
}

fn select(
    storage: &Storage,
    ids: &[&str],
    selected: bool,
) -> Result<Vec<ProviderModel>, ModelError> {
    set_models_selected(
        storage,
        &SetModelsSelectedRequest {
            provider_id: PROVIDER.to_owned(),
            model_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            selected,
        },
        200,
    )
}

#[test]
fn save_selections_deletes_removed_rows_and_inserts_new_ones() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    insert_fetched(&storage, PROVIDER, "keep-me", None);
    select(&storage, &["keep-me"], true).expect("select");
    insert_fetched(&storage, PROVIDER, "drop-me", None);
    select(&storage, &["drop-me"], true).expect("select");

    save_provider_model_selections(
        &storage,
        &SaveProviderModelSelectionsRequest {
            provider_id: PROVIDER.to_owned(),
            remove_model_ids: vec!["drop-me".to_owned()],
            add: vec![SaveProviderModelEntry {
                model_id: "new-model".to_owned(),
                upstream_name: Some("New".to_owned()),
                context_window: Some(1000),
                max_output_tokens: None,
                input_modalities: None,
                output_modalities: None,
                supported_endpoints: None,
            }],
        },
        300,
    )
    .expect("save");

    let page = list(&storage, None, 50, "selected");
    let ids: Vec<_> = page.items.iter().map(|m| m.model_id.as_str()).collect();
    assert_eq!(ids, ["keep-me", "new-model"]);
}

#[test]
fn cursor_round_trips_ids_with_separators() {
    for id in [
        "mistral/mistral-large-4",
        "ling-3.1-flash:free",
        "模型/甲",
        "a",
    ] {
        let cursor = encode_cursor(id);
        assert!(cursor.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(decode_cursor(&cursor).as_deref(), Ok(id));
    }
    assert_eq!(encode_cursor("a/b"), "612f62");
}

#[test]
fn malformed_cursors_are_rejected() {
    let too_long = "61".repeat(MAX_CURSOR_LEN);
    for cursor in [
        "",
        "6",
        "zz",
        "612F62", // uppercase is not what `encode_cursor` makes
        "ff",     // not UTF-8
        "6120",   // "a " contains whitespace
        "07",     // control character
        too_long.as_str(),
        "1722.abc", // the provider cursor format
    ] {
        assert_eq!(
            decode_cursor(cursor),
            Err(ModelError::InvalidRequest),
            "{cursor}"
        );
    }
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    let result = list_provider_models(
        &storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: Some("not-a-cursor".to_owned()),
            limit: 10,
            filter: "all".to_owned(),
            query: None,
        },
    );
    assert_eq!(result, Err(ModelError::InvalidRequest));
}

#[test]
fn pages_are_ordered_by_model_id_and_cross_ids_with_separators() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
    insert_provider(&storage, OTHER, "openrouter", "chat_completions");
    let ids = [
        "a.b",
        "a/b",
        "a:b",
        "a",
        "ab",
        "mistral/mistral-large-4",
        "openai/gpt-4",
        "z",
    ];
    for id in ids {
        insert_fetched(&storage, PROVIDER, id, None);
    }
    insert_fetched(&storage, OTHER, "only-other", None);

    let mut seen = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = list(&storage, cursor, 3, "all");
        pages += 1;
        seen.extend(page.items.iter().map(|model| model.model_id.clone()));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    let mut expected: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    expected.sort(); // byte order, the same as SQLite's BINARY collation
    assert_eq!(seen, expected);
    assert_eq!(pages, 3);

    // A row inserted before the cursor is not repeated; one after it appears.
    let first = list(&storage, None, 3, "all");
    insert_fetched(&storage, PROVIDER, "0-early", None);
    insert_fetched(&storage, PROVIDER, "zz-late", None);
    let mut rest = Vec::new();
    let mut cursor = first.next_cursor;
    while let Some(next) = cursor {
        let page = list(&storage, Some(next), 3, "all");
        rest.extend(page.items.into_iter().map(|model| model.model_id));
        cursor = page.next_cursor;
    }
    assert!(!rest.contains(&"0-early".to_owned()));
    assert_eq!(rest.last().map(String::as_str), Some("zz-late"));
}

#[test]
fn list_validates_the_request_and_the_provider() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    let request = |provider_id: &str, limit: u32, filter: &str| ListProviderModelsRequest {
        provider_id: provider_id.to_owned(),
        after: None,
        limit,
        filter: filter.to_owned(),
        query: None,
    };
    for (bad, expected) in [
        (request("nope", 10, "all"), ModelError::InvalidRequest),
        (request(PROVIDER, 0, "all"), ModelError::InvalidRequest),
        (
            request(PROVIDER, MAX_MODEL_PAGE_SIZE + 1, "all"),
            ModelError::InvalidRequest,
        ),
        (request(PROVIDER, 10, "ticked"), ModelError::InvalidRequest),
        (request(OTHER, 10, "all"), ModelError::NotFound),
    ] {
        assert_eq!(list_provider_models(&storage, &bad), Err(expected));
    }
    let empty = list(&storage, None, MAX_MODEL_PAGE_SIZE, "all");
    assert!(empty.items.is_empty());
    assert_eq!(empty.next_cursor, None);
}

#[test]
fn manual_models_are_validated_selected_and_unique() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "command-code", "chat_completions");

    let added = add(&storage, "  mistral/mistral-large-4  ", Some("  Large  ")).unwrap();
    assert_eq!(added.model_id, "mistral/mistral-large-4");
    assert_eq!(added.alias.as_deref(), Some("Large"));
    assert_eq!(added.source, ModelSource::Manual);
    assert!(added.selected);
    assert_eq!(added.upstream_state, UpstreamState::NeverListed);
    // A manual Command Code model has no routes: selectable, but unverified.
    assert_eq!(added.route_support, RouteSupport::Unknown);
    assert_eq!((added.created_at_ms, added.updated_at_ms), (100, 100));

    assert_eq!(
        add(&storage, "mistral/mistral-large-4", None),
        Err(ModelError::ModelAlreadyExists)
    );
    // IDs are case-sensitive.
    assert!(add(&storage, "Mistral/Mistral-Large-4", None).is_ok());
    for invalid in [
        "",
        "   ",
        "has space",
        "tab\tin",
        "zero\u{200b}width",
        &"m".repeat(257),
    ] {
        assert_eq!(
            add(&storage, invalid, None),
            Err(ModelError::ModelIdInvalid),
            "{invalid:?}"
        );
    }
    for alias in ["   ", "line\nbreak", &"a".repeat(65)] {
        assert_eq!(
            add(&storage, "fresh", Some(alias)),
            Err(ModelError::ModelAliasInvalid)
        );
    }
    let missing_provider = add_manual_model(
        &storage,
        &AddManualModelRequest {
            provider_id: OTHER.to_owned(),
            model_id: "x".to_owned(),
            alias: None,
        },
        100,
    );
    assert_eq!(missing_provider, Err(ModelError::NotFound));
}

#[test]
fn selection_is_all_or_nothing_and_respects_routes() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "command-code", "chat_completions");
    insert_fetched(
        &storage,
        PROVIDER,
        "deepseek/deepseek-v4-flash",
        Some(r#"["/chat/completions","/responses"]"#),
    );
    insert_fetched(
        &storage,
        PROVIDER,
        "claude-sonnet-5",
        Some(r#"["/messages"]"#),
    );
    insert_fetched(&storage, PROVIDER, "no-routes", None);
    add(&storage, "manual-model", None).unwrap();

    let changed = select(&storage, &["deepseek/deepseek-v4-flash"], true).unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed[0].selected);
    assert_eq!(changed[0].route_support, RouteSupport::Supported);
    assert_eq!(changed[0].updated_at_ms, 200);

    // Fetched models that do not list the protocol's route cannot be selected,
    // and the whole batch is rolled back.
    assert_eq!(
        select(&storage, &["manual-model", "claude-sonnet-5"], false).map(|models| models.len()),
        Ok(2)
    );
    assert_eq!(
        select(&storage, &["manual-model", "claude-sonnet-5"], true),
        Err(ModelError::ModelRouteNotSupported)
    );
    assert_eq!(
        select(&storage, &["no-routes"], true),
        Err(ModelError::ModelRouteNotSupported)
    );
    let selected = list(&storage, None, 10, "selected");
    assert_eq!(
        selected
            .items
            .iter()
            .map(|model| model.model_id.as_str())
            .collect::<Vec<_>>(),
        ["deepseek/deepseek-v4-flash"],
        "the failed batch left manual-model unselected"
    );

    // Unselecting is always allowed; an unknown ID fails the whole batch.
    assert_eq!(
        select(&storage, &["deepseek/deepseek-v4-flash", "gone"], false),
        Err(ModelError::ModelNotFound)
    );
    assert_eq!(list(&storage, None, 10, "selected").items.len(), 1);
    assert_eq!(select(&storage, &[], true), Err(ModelError::InvalidRequest));
    let too_many: Vec<String> = (0..=MAX_SELECTION_BATCH).map(|i| format!("m{i}")).collect();
    let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();
    assert_eq!(
        select(&storage, &too_many, true),
        Err(ModelError::InvalidRequest)
    );
}

#[test]
fn other_vendors_do_not_use_routes() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
    insert_fetched(&storage, PROVIDER, "openai/gpt-4", None);
    let changed = select(&storage, &["openai/gpt-4"], true).unwrap();
    assert_eq!(changed[0].route_support, RouteSupport::NotApplicable);
    assert_eq!(changed[0].upstream_state, UpstreamState::Listed);
    assert_eq!(changed[0].upstream_name.as_deref(), Some("Upstream"));
    assert_eq!(changed[0].context_window, Some(1000));
}

#[test]
fn only_manual_models_can_be_deleted() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    insert_fetched(&storage, PROVIDER, "deepseek-flash", None);
    add(&storage, "my/model", None).unwrap();
    let delete = |model_id: &str| {
        delete_manual_model(
            &storage,
            &DeleteManualModelRequest {
                provider_id: PROVIDER.to_owned(),
                model_id: model_id.to_owned(),
            },
        )
    };
    assert_eq!(delete("deepseek-flash"), Err(ModelError::InvalidRequest));
    assert_eq!(delete("missing"), Err(ModelError::ModelNotFound));
    assert_eq!(delete("bad id"), Err(ModelError::InvalidRequest));
    assert_eq!(delete("my/model"), Ok(()));
    let remaining: Vec<String> = list(&storage, None, 10, "all")
        .items
        .into_iter()
        .map(|model| model.model_id)
        .collect();
    assert_eq!(remaining, ["deepseek-flash"]);
}

#[test]
fn corrupt_stored_rows_are_reported_not_guessed() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    // Valid JSON, but not a string array.
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, input_modalities, created_at, updated_at)
             VALUES (?1, 'odd', 'fetched', 0, '{\"a\":1}', 1, 1)",
            [PROVIDER],
        )
        .unwrap();
    let result = list_provider_models(
        &storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: None,
            limit: 10,
            filter: "all".to_owned(),
            query: None,
        },
    );
    assert_eq!(result, Err(ModelError::InvalidStoredModel));
}

/// Insert a fetched row with a chosen upstream name and selection.
fn insert_named(storage: &Storage, model_id: &str, name: Option<&str>, selected: bool) {
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, upstream_name,
                  created_at, updated_at, last_seen_at)
             VALUES (?1, ?2, 'fetched', ?3, ?4, 5, 5, 5)",
            params![PROVIDER, model_id, selected, name],
        )
        .unwrap();
}

fn search(
    storage: &Storage,
    query: &str,
    limit: u32,
    filter: &str,
) -> Result<ProviderModelPage, ModelError> {
    list_provider_models(
        storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: None,
            limit,
            filter: filter.to_owned(),
            query: Some(query.to_owned()),
        },
    )
}

fn ids(page: &ProviderModelPage) -> Vec<&str> {
    page.items
        .iter()
        .map(|model| model.model_id.as_str())
        .collect()
}

#[test]
fn search_ranks_across_id_name_and_alias() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
    insert_named(&storage, "deepseek-chat", Some("深度求索 Chat"), false);
    insert_named(
        &storage,
        "deepseek-reasoner",
        Some("深度求索 Reasoner"),
        false,
    );
    insert_named(
        &storage,
        "mistral/mistral-large-4",
        Some("Mistral Large"),
        false,
    );
    add(&storage, "my-local", Some("我的模型")).unwrap();

    let page = search(&storage, "ds chat", 10, "all").unwrap();
    assert_eq!(ids(&page), ["deepseek-chat"]);
    assert_eq!(
        (page.total_matches, page.next_cursor.as_deref()),
        (Some(1), None)
    );
    // Chinese words in the upstream name: contains, subsequence, exact.
    for query in ["求索", "深求", "深度求索"] {
        let page = search(&storage, query, 10, "all").unwrap();
        assert_eq!(
            ids(&page),
            ["deepseek-chat", "deepseek-reasoner"],
            "{query}"
        );
    }
    assert_eq!(
        ids(&search(&storage, "我的", 10, "all").unwrap()),
        ["my-local"]
    );
    assert_eq!(
        ids(&search(&storage, "MISTRAL large", 10, "all").unwrap()),
        ["mistral/mistral-large-4"]
    );
    // `%` and `_` are not wildcards here: nothing contains `%`.
    assert_eq!(
        search(&storage, "%", 10, "all").unwrap().total_matches,
        Some(0)
    );
    assert_eq!(
        search(&storage, "'; DROP TABLE provider_model; --", 10, "all")
            .unwrap()
            .total_matches,
        Some(0)
    );
    assert_eq!(
        list(&storage, None, 10, "all").items.len(),
        4,
        "the table is intact"
    );
}

#[test]
fn search_truncates_to_the_limit_and_reports_the_total() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
    for id in ["b/chat", "a/chat", "chat", "c/chatty", "d/other"] {
        insert_named(&storage, id, None, false);
    }
    let page = search(&storage, "chat", 2, "all").unwrap();
    assert_eq!(
        ids(&page),
        ["chat", "a/chat"],
        "full match first, then ties by ID"
    );
    assert_eq!(page.total_matches, Some(4));
    assert_eq!(page.next_cursor, None);
    let all = search(&storage, "chat", MAX_MODEL_PAGE_SIZE, "all").unwrap();
    assert_eq!(ids(&all), ["chat", "a/chat", "b/chat", "c/chatty"]);
}

#[test]
fn search_combines_with_the_selected_filter() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    insert_named(&storage, "deepseek-chat", None, true);
    insert_named(&storage, "deepseek-reasoner", None, false);
    let selected = search(&storage, "deepseek", 10, "selected").unwrap();
    assert_eq!(
        (ids(&selected), selected.total_matches),
        (vec!["deepseek-chat"], Some(1))
    );
    assert_eq!(
        search(&storage, "deepseek", 10, "all")
            .unwrap()
            .total_matches,
        Some(2)
    );
}

#[test]
fn empty_queries_list_normally_and_bad_queries_are_rejected() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "deepseek", "chat_completions");
    for id in ["a", "b", "c"] {
        insert_named(&storage, id, None, false);
    }
    for blank in ["", "   ", " - _ / . "] {
        let page = search(&storage, blank, 2, "all").unwrap();
        assert_eq!(ids(&page), ["a", "b"], "{blank:?} is no query");
        assert_eq!(page.total_matches, None);
        assert!(page.next_cursor.is_some(), "paging still works");
    }
    assert!(search(&storage, &"深".repeat(200), 10, "all").is_ok());
    assert_eq!(
        search(&storage, &"深".repeat(201), 10, "all"),
        Err(ModelError::InvalidRequest)
    );
    let with_cursor = list_provider_models(
        &storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: Some(encode_cursor("a")),
            limit: 10,
            filter: "all".to_owned(),
            query: Some("b".to_owned()),
        },
    );
    assert_eq!(with_cursor, Err(ModelError::InvalidRequest));
    // A blank query with a cursor is just the next page.
    let blank_with_cursor = list_provider_models(
        &storage,
        &ListProviderModelsRequest {
            provider_id: PROVIDER.to_owned(),
            after: Some(encode_cursor("a")),
            limit: 10,
            filter: "all".to_owned(),
            query: Some("  ".to_owned()),
        },
    )
    .unwrap();
    assert_eq!(ids(&blank_with_cursor), ["b", "c"]);
    // Limits and filters are still checked for searches.
    assert_eq!(
        search(&storage, "a", 0, "all"),
        Err(ModelError::InvalidRequest)
    );
    assert_eq!(
        search(&storage, "a", 10, "ticked"),
        Err(ModelError::InvalidRequest)
    );
}

#[test]
fn the_request_accepts_an_absent_query_but_no_unknown_fields() {
    let without: ListProviderModelsRequest = serde_json::from_value(serde_json::json!({
        "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all"
    }))
    .unwrap();
    assert_eq!(without.query, None);
    let with: ListProviderModelsRequest = serde_json::from_value(serde_json::json!({
        "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all", "query": "ds"
    }))
    .unwrap();
    assert_eq!(with.query.as_deref(), Some("ds"));
    assert!(
        serde_json::from_value::<ListProviderModelsRequest>(serde_json::json!({
            "providerId": PROVIDER, "after": null, "limit": 10, "filter": "all", "search": "ds"
        }))
        .is_err()
    );
}

#[test]
fn searching_five_thousand_models_is_fast_enough() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "openrouter", "chat_completions");
    {
        let mut connection = storage.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..5000 {
            transaction
                .execute(
                    "INSERT INTO provider_model
                         (provider_id, model_id, source, selected, upstream_name,
                          created_at, updated_at, last_seen_at)
                     VALUES (?1, ?2, 'fetched', ?3, ?4, 5, 5, 5)",
                    params![
                        PROVIDER,
                        format!("vendor-{}/model-{index:04}-chat", index % 50),
                        index % 7 == 0,
                        format!("Vendor {} Model {index} 深度求索", index % 50)
                    ],
                )
                .unwrap();
        }
        transaction.commit().unwrap();
    }
    let started = std::time::Instant::now();
    let broad = search(&storage, "chat", MAX_MODEL_PAGE_SIZE, "all").unwrap();
    let narrow = search(&storage, "vendor 7 model 0007", 10, "all").unwrap();
    let none = search(&storage, "zzzz-not-there", 10, "all").unwrap();
    let elapsed = started.elapsed();
    eprintln!("5000-row search, three queries: {elapsed:?}");
    assert_eq!(broad.total_matches, Some(5000));
    assert_eq!(broad.items.len(), 200);
    assert_eq!(narrow.items[0].model_id, "vendor-7/model-0007-chat");
    assert_eq!(none.total_matches, Some(0));
    // A loose bound for slow, shared CI machines in debug builds; this guards
    // against an accidental quadratic algorithm, not against small slowdowns.
    assert!(elapsed < std::time::Duration::from_secs(10), "{elapsed:?}");
}

#[test]
fn models_serialize_with_camel_case_fields_and_snake_case_values() {
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(&storage, PROVIDER, "command-code", "chat_completions");
    let model = add(&storage, "a/b", None).unwrap();
    let json = serde_json::to_value(&model).unwrap();
    assert_eq!(json["modelId"], "a/b");
    assert_eq!(json["routeSupport"], "unknown");
    assert_eq!(json["upstreamState"], "never_listed");
    assert_eq!(json["source"], "manual");
    assert_eq!(
        serde_json::to_value(ModelError::ModelRouteNotSupported).unwrap(),
        "model_route_not_supported"
    );
    assert_eq!(
        ModelError::InvalidStoredModel.to_string(),
        "invalid_stored_model"
    );
}
