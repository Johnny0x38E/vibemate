//! Saved-model refresh regression tests (P16.c) against local HTTP fixtures.

use super::*;

/// Run a refresh on the test runtime: register, snapshot and key, download, refresh.
async fn refresh(
    storage: &Storage,
    store: &FakeStore,
    now_ms: i64,
) -> Result<ModelRefreshSummary, ModelError> {
    let provider_id = parse_provider_id(PROVIDER)?;
    let registry = ModelFetchRegistry::new();
    let (registration, cancel) = registry.begin(&provider_id)?;
    let prepared = prepare_fetch(storage, store, &provider_id)?;
    let catalog = download_catalog(&fetcher(), &prepared, cancel).await?;
    let PreparedFetch { snapshot, key } = prepared;
    drop(key);
    registration.enter_merge();
    refresh_saved_models(storage, &snapshot, &catalog, now_ms)
}

/// Insert one selected saved row with explicit provenance, alias and last-seen time.
fn insert_saved_model(
    storage: &Storage,
    model_id: &str,
    source: &str,
    alias: Option<&str>,
    upstream_name: &str,
    last_seen_at: Option<i64>,
) {
    storage
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO provider_model
                 (provider_id, model_id, source, selected, alias, upstream_name, context_window,
                  input_modalities, capabilities, created_at, updated_at, last_seen_at, missing_since)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, 100, '[\"text\"]', ?6, 1, 1, ?7, NULL)",
            params![
                PROVIDER,
                model_id,
                source,
                alias,
                upstream_name,
                r#"{"thinking":{"state":"supported","source":"deepseek_model_effort_levels"},"imageInput":{"state":"unknown","source":null},"toolCalls":{"state":"unknown","source":null}}"#,
                last_seen_at
            ],
        )
        .unwrap();
}

#[test]
fn refresh_updates_saved_rows_and_never_inserts_or_deletes() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let server = MockServer::start(vec![ok_json(&deepseek_body(&[
            "deepseek-flash",
            "new-upstream-model",
        ]))]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        insert_saved_model(
            &storage,
            "deepseek-flash",
            "fetched",
            Some("Mine"),
            "Old name",
            Some(5),
        );
        insert_saved_model(
            &storage,
            "manual-never-listed",
            "manual",
            Some("Manual"),
            "Manual name",
            None,
        );
        insert_saved_model(
            &storage,
            "removed-upstream",
            "fetched",
            None,
            "Removed",
            Some(5),
        );

        let summary = refresh(&storage, &key_store(), 900).await.unwrap();
        assert!(summary.complete);
        assert_eq!(summary.incomplete_reason, None);
        assert_eq!(summary.refreshed, 1);
        assert_eq!(summary.marked_missing, 1);
        assert_eq!(summary.fetched_at_ms, 900);

        let models = all_models(&storage);
        assert_eq!(models.len(), 3);
        let refreshed = model(&models, "deepseek-flash").unwrap();
        assert_eq!(refreshed.source, ModelSource::Fetched);
        assert_eq!(refreshed.alias.as_deref(), Some("Mine"));
        assert!(refreshed.selected);
        assert_eq!(refreshed.upstream_name, None);
        assert_eq!(refreshed.context_window, Some(1_000_000));
        assert_eq!(refreshed.last_seen_at_ms, Some(900));
        assert_eq!(refreshed.missing_since_ms, None);
        // The catalog replaced the previously claimed thinking support.
        assert_eq!(
            refreshed.capabilities.thinking.state,
            crate::model_catalog::CapabilityState::Unknown
        );
        assert_eq!(
            refreshed.capabilities.image_input.state,
            crate::model_catalog::CapabilityState::Unsupported
        );
        assert_eq!(
            refreshed.capabilities.tool_calls.state,
            crate::model_catalog::CapabilityState::Supported
        );

        let manual = model(&models, "manual-never-listed").unwrap();
        assert_eq!(manual.source, ModelSource::Manual);
        assert_eq!(manual.alias.as_deref(), Some("Manual"));
        assert_eq!(manual.upstream_name.as_deref(), Some("Manual name"));
        assert_eq!(manual.last_seen_at_ms, None);
        assert_eq!(manual.missing_since_ms, None);
        assert_eq!(
            manual.capabilities.thinking.state,
            crate::model_catalog::CapabilityState::Supported
        );

        let removed = model(&models, "removed-upstream").unwrap();
        assert_eq!(removed.missing_since_ms, Some(900));
        assert!(removed.selected);
        assert!(model(&models, "new-upstream-model").is_none());
        assert_eq!(count_rows(&storage, "provider_model"), 3);
        assert_eq!(count_rows(&storage, "provider_model_fetch"), 1);
    });
}

#[test]
fn incomplete_refresh_never_marks_absence_and_stale_provider_writes_nothing() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let replies: Vec<Reply> = (0..10)
            .map(|_| ok_json(&openrouter_body(&["page-one"], true)))
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
        insert_saved_model(
            &storage,
            "only-on-a-later-page",
            "fetched",
            None,
            "Later",
            Some(5),
        );

        // The page cap makes the refresh incomplete; absence must not be recorded.
        let summary = refresh(&storage, &key_store(), 901).await.unwrap();
        assert!(!summary.complete);
        assert_eq!(
            summary.incomplete_reason,
            Some(crate::model_catalog::IncompleteReason::PageLimit)
        );
        assert_eq!(summary.refreshed, 0);
        assert_eq!(summary.marked_missing, 0);
        let row = model(&all_models(&storage), "only-on-a-later-page")
            .unwrap()
            .clone();
        assert_eq!(row.missing_since_ms, None);
        assert_eq!(row.upstream_name.as_deref(), Some("Later"));

        // A provider settings change after preparation makes the refresh stale,
        // and nothing is written.
        let server = MockServer::start(vec![ok_json(&deepseek_body(&["deepseek-flash"]))]);
        let directory = TestDirectory::new();
        let storage = directory.open();
        insert_provider(
            &storage,
            "deepseek",
            "chat_completions",
            server.url("").as_str(),
        );
        insert_saved_model(
            &storage,
            "deepseek-flash",
            "fetched",
            None,
            "Old name",
            Some(5),
        );
        let provider_id = parse_provider_id(PROVIDER).unwrap();
        let registry = ModelFetchRegistry::new();
        let (registration, cancel) = registry.begin(&provider_id).unwrap();
        let prepared = prepare_fetch(&storage, &key_store(), &provider_id).unwrap();
        let catalog = download_catalog(&fetcher(), &prepared, cancel)
            .await
            .unwrap();
        storage
            .lock()
            .unwrap()
            .execute(
                "UPDATE provider_instance SET revision = 2 WHERE id = ?1",
                [PROVIDER],
            )
            .unwrap();
        let PreparedFetch { snapshot, key } = prepared;
        drop(key);
        registration.enter_merge();
        assert_eq!(
            refresh_saved_models(&storage, &snapshot, &catalog, 902),
            Err(ModelError::ModelFetchStale)
        );
        let models = all_models(&storage);
        let row = model(&models, "deepseek-flash").unwrap();
        assert_eq!(row.upstream_name.as_deref(), Some("Old name"));
        assert_eq!(row.updated_at_ms, 1);
    });
}
