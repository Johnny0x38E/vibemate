//! Behavior tests for the parent module, kept separate from runtime code.

use std::collections::BTreeMap;

use super::*;

/// The real Command Code response captured on 2026-10-10 (decision D16).
const COMMAND_CODE_FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/command-code-models-2026-10-10.json"
));

/// The example response from DeepSeek's "Lists Models" documentation.
const DEEPSEEK_EXAMPLE: &str = r#"{"object":"list","data":[
    {"id":"deepseek-flash","object":"model","owned_by":"deepseek","name":"DeepSeek-V4.1-Flash",
     "context_window":1048576,"max_output_tokens":393216,"input_modalities":["text","image"],
     "output_modalities":["text"],"effort":{"supported_levels":["low","high","max"],"default_level":"high"},
     "api_capabilities":{"anthropic_messages":{"system_prompt_update":"in-history"}}},
    {"id":"deepseek-v4-pro","object":"model","owned_by":"deepseek","name":"DeepSeek-V4-Pro",
     "context_window":1048576,"max_output_tokens":393216,"input_modalities":["text"],
     "output_modalities":["text"]}]}"#;

/// One OpenRouter model shaped like the documented example.
fn openrouter_entry(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","canonical_slug":"{id}","name":"Model {id}","created":1692901234,
        "context_length":8192,"architecture":{{"input_modalities":["text"],"modality":"text->text",
        "output_modalities":["text"],"tokenizer":"GPT"}},"pricing":{{"prompt":"0.00003"}},
        "top_provider":{{"context_length":8192,"is_moderated":true,"max_completion_tokens":4096}},
        "per_request_limits":null,"supported_parameters":["temperature"],"default_parameters":null,
        "supported_voices":null,"links":{{"details":"/api/v1/models/{id}/endpoints"}}}}"#
    )
}

fn openrouter_body(ids: &[&str], next: Option<&str>, total: usize) -> Vec<u8> {
    let entries: Vec<String> = ids.iter().map(|id| openrouter_entry(id)).collect();
    let next = next.map_or("null".to_owned(), |url| format!("\"{url}\""));
    format!(
        r#"{{"data":[{}],"links":{{"next":{next}}},"total_count":{total}}}"#,
        entries.join(",")
    )
    .into_bytes()
}

fn command_code_body(entries: &str) -> Vec<u8> {
    format!(r#"{{"object":"list","data":[{entries}]}}"#).into_bytes()
}

#[test]
fn command_code_fixture_matches_the_recorded_shape() {
    let page = parse_command_code(COMMAND_CODE_FIXTURE).expect("fixture parses");
    assert_eq!(page.entries, 87);
    assert_eq!(page.models.len(), 87);
    assert_eq!(page.skipped_invalid, 0);
    assert_eq!(
        page.models
            .iter()
            .filter(|model| model.model_id.contains('/'))
            .count(),
        65
    );
    assert!(page.models.iter().all(|model| {
        model.upstream_name.is_some()
            && model.context_window.is_some()
            && model.max_output_tokens.is_none()
    }));

    let mut combinations: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for model in &page.models {
        let endpoints = model.supported_endpoints.clone().expect("endpoints");
        *combinations.entry(endpoints).or_default() += 1;
    }
    let combination = |values: &[&str]| -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    };
    assert_eq!(combinations.len(), 3);
    assert_eq!(
        combinations[&combination(&["/chat/completions", "/responses"])],
        68
    );
    assert_eq!(combinations[&combination(&["/messages"])], 11);
    assert_eq!(combinations[&combination(&["/chat/completions"])], 8);

    // The Anthropic-only models are the Claude models.
    assert!(page.models.iter().all(|model| {
        let messages_only =
            model.supported_endpoints.as_deref() == Some(&combination(&["/messages"])[..]);
        messages_only == model.model_id.starts_with("claude")
    }));

    let mut selectable = 0;
    let mut blocked = 0;
    for model in &page.models {
        match route_support(
            ProviderKind::CommandCode,
            ProviderProtocol::ChatCompletions,
            model.supported_endpoints.as_deref(),
        ) {
            RouteSupport::Supported => selectable += 1,
            RouteSupport::Unsupported => blocked += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!((selectable, blocked), (76, 11));

    let collected = {
        let mut collector = CatalogCollector::new();
        collector.add_page(page);
        collector.finish(false)
    };
    assert_eq!(collected.models.len(), 87);
    assert_eq!(collected.incomplete, None);
}

#[test]
fn endpoints_with_and_without_the_v1_prefix_are_the_same_route() {
    assert_eq!(
        normalize_endpoint("/v1/chat/completions"),
        "/chat/completions"
    );
    assert_eq!(normalize_endpoint("/chat/completions"), "/chat/completions");
    assert_eq!(normalize_endpoint("/v1/messages"), "/messages");
    // Only a whole `/v1` path segment is removed.
    assert_eq!(normalize_endpoint("/v1"), "/v1");
    assert_eq!(normalize_endpoint("/v10/messages"), "/v10/messages");

    let page = parse_command_code(&command_code_body(
        r#"{"id":"a","supported_endpoints":["/v1/chat/completions","/chat/completions","/v1/responses"]},
           {"id":"b","supported_endpoints":["/v1/messages"]}"#,
    ))
    .unwrap();
    let first = page.models[0].supported_endpoints.as_deref().unwrap();
    assert_eq!(first, ["/chat/completions", "/responses"]);
    for (protocol, expected) in [
        (ProviderProtocol::ChatCompletions, RouteSupport::Supported),
        (ProviderProtocol::Responses, RouteSupport::Supported),
        (
            ProviderProtocol::AnthropicMessages,
            RouteSupport::Unsupported,
        ),
    ] {
        assert_eq!(
            route_support(ProviderKind::CommandCode, protocol, Some(first)),
            expected
        );
    }
    let second = page.models[1].supported_endpoints.as_deref();
    assert_eq!(
        route_support(
            ProviderKind::CommandCode,
            ProviderProtocol::AnthropicMessages,
            second
        ),
        RouteSupport::Supported
    );
}

#[test]
fn missing_or_malformed_endpoints_are_unknown_and_other_vendors_not_applicable() {
    let page = parse_command_code(&command_code_body(
        r#"{"id":"none"},{"id":"string","supported_endpoints":"/chat/completions"},
           {"id":"empty","supported_endpoints":[]},
           {"id":"junk","supported_endpoints":[1,null,"has space","/chat/completions"]}"#,
    ))
    .unwrap();
    let support = |index: usize| {
        route_support(
            ProviderKind::CommandCode,
            ProviderProtocol::ChatCompletions,
            page.models[index].supported_endpoints.as_deref(),
        )
    };
    assert_eq!(support(0), RouteSupport::Unknown);
    assert_eq!(support(1), RouteSupport::Unknown);
    assert_eq!(support(2), RouteSupport::Unsupported);
    assert_eq!(support(3), RouteSupport::Supported);
    for kind in [ProviderKind::DeepSeek, ProviderKind::OpenRouter] {
        assert_eq!(
            route_support(kind, ProviderProtocol::ChatCompletions, None),
            RouteSupport::NotApplicable
        );
    }
}

#[test]
fn deepseek_documented_example_parses() {
    let page = parse_deepseek(DEEPSEEK_EXAMPLE.as_bytes()).unwrap();
    assert_eq!(page.skipped_invalid, 0);
    assert_eq!(
        page.models[0],
        CatalogModel {
            model_id: "deepseek-flash".to_owned(),
            upstream_name: Some("DeepSeek-V4.1-Flash".to_owned()),
            context_window: Some(1_048_576),
            max_output_tokens: Some(393_216),
            input_modalities: Some(vec!["text".to_owned(), "image".to_owned()]),
            output_modalities: Some(vec!["text".to_owned()]),
            supported_endpoints: None,
        }
    );
    assert_eq!(page.models[1].model_id, "deepseek-v4-pro");
    assert_eq!(
        parse_page(ProviderKind::DeepSeek, DEEPSEEK_EXAMPLE.as_bytes()).unwrap(),
        page
    );
}

#[test]
fn openrouter_page_maps_nested_fields_and_paging_links() {
    let body = openrouter_body(
        &["openai/gpt-4"],
        Some("/api/v1/models?offset=500&limit=500"),
        150,
    );
    let page = parse_openrouter_page(&body).unwrap();
    assert!(page.has_more);
    assert_eq!(page.total_count, Some(150));
    assert_eq!(
        page.models[0],
        CatalogModel {
            model_id: "openai/gpt-4".to_owned(),
            upstream_name: Some("Model openai/gpt-4".to_owned()),
            context_window: Some(8192),
            max_output_tokens: Some(4096),
            input_modalities: Some(vec!["text".to_owned()]),
            output_modalities: Some(vec!["text".to_owned()]),
            supported_endpoints: None,
        }
    );
    let last = parse_openrouter_page(&openrouter_body(&["a"], None, 1)).unwrap();
    assert!(!last.has_more);
}

#[test]
fn openrouter_requires_its_documented_envelope() {
    for body in [
        r#"{"data":[],"total_count":0}"#,
        r#"{"data":[],"links":{},"total_count":0}"#,
        r#"{"data":[],"links":{"next":5},"total_count":0}"#,
        r#"{"data":[],"links":{"next":null}}"#,
        r#"{"data":[],"links":{"next":null},"total_count":-1}"#,
    ] {
        assert_eq!(
            parse_openrouter_page(body.as_bytes()),
            Err(CatalogError::UpstreamResponseInvalid),
            "{body}"
        );
    }
}

#[test]
fn invalid_envelopes_are_rejected_for_every_vendor() {
    for kind in [
        ProviderKind::CommandCode,
        ProviderKind::DeepSeek,
        ProviderKind::OpenRouter,
    ] {
        for body in [
            &b"not json"[..],
            b"",
            b"[]",
            b"{}",
            br#"{"data":{}}"#,
            br#"{"data":null}"#,
        ] {
            assert_eq!(
                parse_page(kind, body),
                Err(CatalogError::UpstreamResponseInvalid)
            );
        }
    }
    assert_eq!(
        CatalogError::UpstreamResponseInvalid.to_string(),
        "upstream_response_invalid"
    );
}

#[test]
fn invalid_ids_are_skipped_and_counted() {
    let long_ok = "m".repeat(MAX_MODEL_ID_CHARS);
    let too_long = "m".repeat(MAX_MODEL_ID_CHARS + 1);
    let entries = [
        r#"{"id":"ok/with-slash.v1:free"}"#.to_owned(),
        format!(r#"{{"id":"{long_ok}"}}"#),
        r#"{"name":"no id"}"#.to_owned(),
        r#"{"id":42}"#.to_owned(),
        r#"{"id":""}"#.to_owned(),
        r#"{"id":" padded"}"#.to_owned(),
        r#"{"id":"has space"}"#.to_owned(),
        r#"{"id":"bell\u0007"}"#.to_owned(),
        r#"{"id":"zero\u200bwidth"}"#.to_owned(),
        format!(r#"{{"id":"{too_long}"}}"#),
        r#""just a string""#.to_owned(),
    ];
    let page = parse_deepseek(&command_code_body(&entries.join(","))).unwrap();
    assert_eq!(page.entries, 11);
    assert_eq!(page.skipped_invalid, 9);
    let ids: Vec<&str> = page.models.iter().map(|m| m.model_id.as_str()).collect();
    assert_eq!(ids, ["ok/with-slash.v1:free", long_ok.as_str()]);
    // Characters count, not bytes: 256 CJK characters are still valid.
    assert!(is_valid_model_id(&"模".repeat(MAX_MODEL_ID_CHARS)));
}

#[test]
fn wrong_typed_optional_fields_become_none() {
    let page = parse_deepseek(&command_code_body(
        r#"{"id":"m","name":"  ","context_window":"big","max_output_tokens":0,
            "input_modalities":"text","output_modalities":[1,"text","text"]},
           {"id":"n","name":"  Trimmed  ","context_window":-5,"max_output_tokens":1.5},
           {"id":"o","name":"bad\u202ename","context_window":18446744073709551615}"#,
    ))
    .unwrap();
    let [m, n, o] = &page.models[..] else {
        panic!("three models");
    };
    assert_eq!(m.upstream_name, None);
    assert_eq!(m.context_window, None);
    assert_eq!(m.max_output_tokens, None);
    assert_eq!(m.input_modalities, None);
    assert_eq!(m.output_modalities, Some(vec!["text".to_owned()]));
    assert_eq!(n.upstream_name.as_deref(), Some("Trimmed"));
    assert_eq!(n.context_window, None);
    assert_eq!(n.max_output_tokens, None);
    assert_eq!(o.upstream_name, None);
    // Larger than SQLite's INTEGER range.
    assert_eq!(o.context_window, None);
}

#[test]
fn string_lists_are_bounded() {
    let many: Vec<String> = (0..40).map(|index| format!("\"m{index}\"")).collect();
    let long = format!("\"{}\"", "x".repeat(MAX_LIST_ITEM_LEN + 1));
    let body = command_code_body(&format!(
        r#"{{"id":"a","input_modalities":[{}],"output_modalities":[{long},"text"]}}"#,
        many.join(",")
    ));
    let page = parse_deepseek(&body).unwrap();
    assert_eq!(
        page.models[0].input_modalities.as_ref().unwrap().len(),
        MAX_LIST_ITEMS
    );
    assert_eq!(
        page.models[0].output_modalities,
        Some(vec!["text".to_owned()])
    );
}

#[test]
fn collector_keeps_the_first_duplicate_and_reports_empty_lists() {
    let mut collector = CatalogCollector::new();
    collector.add_page(
        parse_command_code(&command_code_body(
            r#"{"id":"a","name":"First"},{"id":"a","name":"Second"},{"id":5}"#,
        ))
        .unwrap(),
    );
    let catalog = collector.finish(false);
    assert_eq!(catalog.models.len(), 1);
    assert_eq!(catalog.models[0].upstream_name.as_deref(), Some("First"));
    assert_eq!((catalog.duplicates, catalog.skipped_invalid), (1, 1));
    assert_eq!(catalog.incomplete, None);

    let empty = CatalogCollector::new().finish(false);
    assert_eq!(empty.incomplete, Some(IncompleteReason::EmptyList));
}

#[test]
fn collector_caps_models_per_provider() {
    let entries: Vec<String> = (0..=MAX_MODELS_PER_PROVIDER)
        .map(|index| format!(r#"{{"id":"m{index}"}}"#))
        .collect();
    let mut collector = CatalogCollector::new();
    collector.add_page(parse_deepseek(&command_code_body(&entries.join(","))).unwrap());
    let catalog = collector.finish(false);
    assert_eq!(catalog.models.len(), MAX_MODELS_PER_PROVIDER);
    assert_eq!(catalog.incomplete, Some(IncompleteReason::ModelLimit));

    // OpenRouter's total_count alone can show the list is over the cap.
    let mut collector = CatalogCollector::new();
    collector.add_page(
        parse_openrouter_page(&openrouter_body(&["a"], None, MAX_MODELS_PER_PROVIDER + 1)).unwrap(),
    );
    assert_eq!(
        collector.finish(false).incomplete,
        Some(IncompleteReason::ModelLimit)
    );
}

#[test]
fn openrouter_pager_counts_offsets_itself_and_stops_at_the_page_cap() {
    let full_page = |next: Option<&str>| {
        let ids: Vec<String> = (0..3).map(|index| format!("m{index}")).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        parse_openrouter_page(&openrouter_body(&ids, next, 100)).unwrap()
    };
    let mut pager = OpenRouterPager::new();
    assert_eq!(pager.next_offset(), 0);
    // The server's `next` URL points elsewhere; it is ignored.
    let step = pager.record(&full_page(Some("https://evil.example/steal")));
    assert_eq!(step, PageStep::Fetch { offset: 3 });
    assert_eq!(pager.record(&full_page(None)), PageStep::Done);

    let mut pager = OpenRouterPager::new();
    let mut steps = Vec::new();
    for _ in 0..OPENROUTER_MAX_PAGES {
        steps.push(pager.record(&full_page(Some("/next"))));
    }
    assert_eq!(steps.last(), Some(&PageStep::PageLimitReached));
    assert_eq!(steps.len(), OPENROUTER_MAX_PAGES);
    assert!(
        steps[..OPENROUTER_MAX_PAGES - 1]
            .iter()
            .all(|step| matches!(step, PageStep::Fetch { .. }))
    );

    // An empty page that still claims "more" ends the list.
    let mut pager = OpenRouterPager::new();
    let empty = parse_openrouter_page(&openrouter_body(&[], Some("/next"), 100)).unwrap();
    assert_eq!(pager.record(&empty), PageStep::Done);

    let mut collector = CatalogCollector::new();
    collector.add_page(full_page(Some("/next")));
    assert_eq!(
        collector.finish(true).incomplete,
        Some(IncompleteReason::PageLimit)
    );
}

#[test]
fn urls_append_models_and_set_paging_through_the_url_crate() {
    for base in ["https://api.deepseek.com", "https://api.deepseek.com/"] {
        assert_eq!(
            models_url(base).unwrap().as_str(),
            "https://api.deepseek.com/models"
        );
    }
    assert_eq!(
        models_url("https://api.commandcode.ai/provider/v1")
            .unwrap()
            .as_str(),
        "https://api.commandcode.ai/provider/v1/models"
    );
    assert_eq!(
        openrouter_page_url("https://openrouter.ai/api/v1", 1000)
            .unwrap()
            .as_str(),
        "https://openrouter.ai/api/v1/models?offset=1000&limit=500"
    );
    // A stored base URL that cannot be parsed is a stored-data problem, not
    // the vendor's fault.
    for broken in ["", "not a url", "relative/path"] {
        assert_eq!(models_url(broken), Err(CatalogError::InvalidStoredProvider));
    }
    assert_eq!(
        CatalogError::InvalidStoredProvider.code(),
        "invalid_stored_provider"
    );
}
