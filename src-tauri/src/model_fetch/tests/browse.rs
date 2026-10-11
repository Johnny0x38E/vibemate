//! Browse regression tests use the public fetch API and local HTTP fixtures.
//! They protect session caching and result/API pagination without merging rows.

use super::*;
use crate::model_search::SearchQuery;

/// Mirror the command's registration lifetime without invoking the desktop runtime.
async fn browse_once(
    fetcher: &ModelFetcher,
    registry: &ModelFetchRegistry,
    prepared: &PreparedFetch,
    offset: usize,
    query: Option<&SearchQuery>,
) -> Result<UpstreamBrowsePage, ModelError> {
    let (_registration, cancel) = registry.begin(&prepared.snapshot.provider_id)?;
    browse_upstream_page(fetcher, registry, prepared, cancel, Some(offset), query).await
}

#[tokio::test]
async fn single_page_vendor_cache_is_shared_and_searchable_without_model_writes() {
    let ids: Vec<String> = (0..75).map(|i| format!("model-{i:03}")).collect();
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let server = MockServer::start(vec![ok_json(&deepseek_body(&names))]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let before = directory.database_bytes();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();

    let first = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    let second = browse_once(&fetcher, &registry.clone(), &prepared, 50, None)
        .await
        .unwrap();
    assert_eq!(second.items.len(), 25);
    assert_eq!(second.items[0].model_id, "model-050");
    assert!(!second.more_pages);
    assert_eq!(second.next_offset, None);

    let query = SearchQuery::parse("model-074").unwrap().unwrap();
    let found = browse_once(&fetcher, &registry, &prepared, 0, Some(&query))
        .await
        .unwrap();
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].model_id, "model-074");
    assert!(!found.more_pages);
    let exhausted = browse_once(&fetcher, &registry, &prepared, usize::MAX, None)
        .await
        .unwrap();
    assert!(exhausted.items.is_empty());
    assert_eq!(exhausted.next_offset, None);
    assert_eq!(server.requests().len(), 1);
    assert!(all_models(&storage).is_empty());
    assert_eq!(
        fetch_status(&storage, &registry, PROVIDER)
            .unwrap()
            .last_fetch,
        None
    );
    assert_eq!(directory.database_bytes(), before);
}

#[tokio::test]
async fn sparse_search_advances_raw_api_offset_once_per_call_and_deduplicates() {
    let server = MockServer::start(vec![
        ok_json(&openrouter_body(&["other", "other"], true)),
        ok_json(&openrouter_body(&["other", "target"], false)),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "openrouter",
        "chat_completions",
        server.url("/api/v1").as_str(),
    );
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();
    let query = SearchQuery::parse("target").unwrap().unwrap();

    let first = browse_once(&fetcher, &registry, &prepared, 0, Some(&query))
        .await
        .unwrap();
    assert!(first.items.is_empty());
    assert!(first.more_pages);
    assert_eq!(first.next_offset, Some(0));
    assert_eq!(
        server.requests().len(),
        1,
        "one API page per browse invocation"
    );
    let second = browse_once(&fetcher, &registry, &prepared, 0, Some(&query))
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].model_id, "target");
    assert_eq!(second.next_offset, None);
    let unfiltered = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(
        unfiltered
            .items
            .iter()
            .map(|m| m.model_id.as_str())
            .collect::<Vec<_>>(),
        ["other", "target"]
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].lines().next(),
        Some("GET /api/v1/models?offset=0&limit=50 HTTP/1.1")
    );
    assert_eq!(
        requests[1].lines().next(),
        Some("GET /api/v1/models?offset=2&limit=50 HTTP/1.1")
    );
    assert!(all_models(&storage).is_empty());
}

#[tokio::test]
async fn changing_revision_or_base_url_discards_the_previous_browse_session() {
    let server = MockServer::start(vec![
        ok_json(&deepseek_body(&["old"])),
        ok_json(&deepseek_body(&["revision-new"])),
        ok_json(&deepseek_body(&["url-new"])),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let first = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(first.items[0].model_id, "old");
    storage
        .lock()
        .unwrap()
        .execute(
            "UPDATE provider_instance SET revision=2 WHERE id=?1",
            [PROVIDER],
        )
        .unwrap();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let second = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(second.items[0].model_id, "revision-new");
    storage
        .lock()
        .unwrap()
        .execute(
            "UPDATE provider_instance SET base_url=?1 WHERE id=?2",
            params![server.url("/changed").as_str(), PROVIDER],
        )
        .unwrap();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let third = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(third.items[0].model_id, "url-new");
    assert_eq!(server.requests().len(), 3);
    assert_eq!(
        server.requests()[2].lines().next(),
        Some("GET /changed/models HTTP/1.1")
    );
    assert!(all_models(&storage).is_empty());
}

#[tokio::test]
async fn failed_page_keeps_cached_rows_and_retries_the_same_api_offset() {
    let server = MockServer::start(vec![
        ok_json(&openrouter_body(&["base"], true)),
        Reply::Close,
        ok_json(&openrouter_body(&["base", "next"], false)),
    ]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "openrouter",
        "chat_completions",
        server.url("/api/v1").as_str(),
    );
    let before = directory.database_bytes();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();
    assert_eq!(
        browse_once(&fetcher, &registry, &prepared, 0, None)
            .await
            .unwrap()
            .items[0]
            .model_id,
        "base"
    );
    assert_eq!(
        browse_once(&fetcher, &registry, &prepared, 1, None)
            .await
            .unwrap_err(),
        ModelError::ConnectionFailed
    );
    assert!(!registry.is_running(&provider_id()));
    let retried = browse_once(&fetcher, &registry, &prepared, 1, None)
        .await
        .unwrap();
    assert_eq!(retried.items[0].model_id, "next");
    assert!(!retried.more_pages);
    let all = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(all.items.len(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].lines().next(), requests[2].lines().next());
    assert!(requests[1].starts_with("GET /api/v1/models?offset=1&limit=50 "));
    assert_eq!(directory.database_bytes(), before);
}

#[tokio::test]
async fn cancelled_browse_closes_http_and_releases_registration_for_a_fresh_retry() {
    let server = MockServer::start(vec![Reply::Hang, ok_json(&deepseek_body(&["retry"]))]);
    let directory = TestDirectory::new();
    let storage = directory.open();
    insert_provider(
        &storage,
        "deepseek",
        "chat_completions",
        server.url("").as_str(),
    );
    let before = directory.database_bytes();
    let prepared = prepare_fetch(&storage, &key_store(), &provider_id()).unwrap();
    let registry = ModelFetchRegistry::new();
    let fetcher = fetcher();
    let (result, cancelled) = tokio::join!(
        browse_once(&fetcher, &registry, &prepared, 0, None),
        async {
            wait_for_requests(&server, 1).await;
            assert_eq!(
                registry.begin(&provider_id()).map(|_| ()),
                Err(ModelError::ModelFetchInProgress)
            );
            registry.cancel(&provider_id())
        }
    );
    assert!(cancelled);
    assert_eq!(result.unwrap_err(), ModelError::ModelFetchCancelled);
    wait_for_close(&server).await;
    assert!(!registry.is_running(&provider_id()));
    let retried = browse_once(&fetcher, &registry, &prepared, 0, None)
        .await
        .unwrap();
    assert_eq!(retried.items[0].model_id, "retry");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(directory.database_bytes(), before);
}
