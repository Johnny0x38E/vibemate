//! Browse-only catalog growth, fuzzy ranking and UI result pagination.
//! One call downloads at most one upstream page and never writes model rows.

use super::cache::CachedVendorBrowseList;
use super::download::{ModelFetcher, download_single_page};
use super::registry::ModelFetchRegistry;
use super::types::*;
use crate::model_catalog::CatalogModel;
use crate::model_error::ModelError;
use crate::model_http::{run_cancellable, with_deadline};
use crate::model_identity::{resolved_protocol, route_support};
use crate::model_search::{MatchScore, SearchQuery, sort_ranked};
use crate::providers::ProviderKind;
use tokio::sync::oneshot;

fn catalog_search_score(model: &CatalogModel, query: &SearchQuery) -> Option<MatchScore> {
    let fields = [
        Some(model.model_id.as_str()),
        model.upstream_name.as_deref(),
    ];
    query.score(fields.into_iter().flatten())
}

fn ranked_catalog_matches<'a>(
    models: &'a [CatalogModel],
    query: &SearchQuery,
) -> Vec<&'a CatalogModel> {
    let mut scored: Vec<(MatchScore, &CatalogModel)> = models
        .iter()
        .filter_map(|model| catalog_search_score(model, query).map(|score| (score, model)))
        .collect();
    sort_ranked(&mut scored, |model| model.model_id.as_str());
    scored.into_iter().map(|(_, model)| model).collect()
}

fn browse_page_slice_searched(
    models: &[CatalogModel],
    offset: usize,
    query: &SearchQuery,
    snapshot: &ProviderSnapshot,
    catalog_may_grow: bool,
) -> UpstreamBrowsePage {
    let ranked = ranked_catalog_matches(models, query);
    if offset >= ranked.len() {
        return UpstreamBrowsePage {
            items: Vec::new(),
            next_offset: catalog_may_grow.then_some(offset),
            more_pages: catalog_may_grow,
        };
    }
    let end = offset
        .saturating_add(UPSTREAM_BROWSE_PAGE_SIZE)
        .min(ranked.len());
    let items = ranked[offset..end]
        .iter()
        .map(|model| upstream_browse_model(model, snapshot))
        .collect();
    let more_in_ranked = end < ranked.len();
    let more_pages = more_in_ranked || catalog_may_grow;
    let next_offset = if more_pages {
        Some(if more_in_ranked { end } else { ranked.len() })
    } else {
        None
    };
    UpstreamBrowsePage {
        items,
        next_offset,
        more_pages,
    }
}

fn browse_page_slice(
    models: &[CatalogModel],
    offset: usize,
    snapshot: &ProviderSnapshot,
    catalog_may_grow: bool,
) -> UpstreamBrowsePage {
    if offset >= models.len() {
        return UpstreamBrowsePage {
            items: Vec::new(),
            next_offset: catalog_may_grow.then_some(offset),
            more_pages: catalog_may_grow,
        };
    }
    let end = offset
        .saturating_add(UPSTREAM_BROWSE_PAGE_SIZE)
        .min(models.len());
    let items = models[offset..end]
        .iter()
        .map(|model| upstream_browse_model(model, snapshot))
        .collect();
    let more_in_cache = end < models.len();
    let more_pages = more_in_cache || catalog_may_grow;
    let next_offset = if more_pages {
        Some(if more_in_cache { end } else { models.len() })
    } else {
        None
    };
    UpstreamBrowsePage {
        items,
        next_offset,
        more_pages,
    }
}

fn browse_cache_covers_request(
    cache: &CachedVendorBrowseList,
    query: Option<&SearchQuery>,
    result_offset: usize,
) -> bool {
    let need = result_offset.saturating_add(UPSTREAM_BROWSE_PAGE_SIZE);
    match query {
        Some(query) => ranked_catalog_matches(&cache.models, query).len() >= need,
        None => cache.models.len() >= need,
    }
}

async fn extend_browse_catalog(
    fetcher: &ModelFetcher,
    prepared: &PreparedFetch,
    cancel: oneshot::Receiver<()>,
    cache: &mut CachedVendorBrowseList,
) -> Result<(), ModelError> {
    if !cache.upstream_more {
        return Ok(());
    }
    let snapshot = &prepared.snapshot;
    let api_offset = cache.upstream_api_offset;
    let openrouter_limit =
        (snapshot.kind == ProviderKind::OpenRouter).then_some(UPSTREAM_BROWSE_PAGE_SIZE);
    let work = with_deadline(
        fetcher.deadline,
        download_single_page(&fetcher.client, prepared, api_offset, openrouter_limit),
    );
    let page = run_cancellable(work, cancel).await?;
    for model in page.models {
        if !cache
            .models
            .iter()
            .any(|existing| existing.model_id == model.model_id)
        {
            cache.models.push(model);
        }
    }
    if snapshot.kind == ProviderKind::OpenRouter {
        cache.upstream_api_offset = api_offset.saturating_add(page.entries);
        cache.upstream_more = page.has_more && page.entries > 0;
    } else {
        cache.upstream_more = false;
    }
    Ok(())
}

fn upstream_browse_model(model: &CatalogModel, snapshot: &ProviderSnapshot) -> UpstreamBrowseModel {
    let endpoints = model.supported_endpoints.as_deref();
    UpstreamBrowseModel {
        model_id: model.model_id.clone(),
        upstream_name: model.upstream_name.clone(),
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        input_modalities: model.input_modalities.clone(),
        output_modalities: model.output_modalities.clone(),
        supported_endpoints: model.supported_endpoints.clone(),
        capabilities: model.capabilities,
        route_support: route_support(snapshot.kind, endpoints),
        resolved_protocol: resolved_protocol(snapshot.kind, snapshot.protocol, endpoints),
    }
}

/// Download one upstream browse page for the Models tab. Does not touch SQLite.
///
/// Each provider keeps one in-memory catalog in `ModelFetchRegistry` for the
/// browse session. A call may download at most one provider API page into that
/// catalog, then returns a slice (or ranked search slice). `next_offset` and
/// `more_pages` refer to result rows, not raw OpenRouter API offsets.
///
/// # Errors
/// Same network and credential errors as `download_catalog`, without merge or stale
/// checks (the UI saves selections via `save_provider_model_selections`).
pub async fn browse_upstream_page(
    fetcher: &ModelFetcher,
    registry: &ModelFetchRegistry,
    prepared: &PreparedFetch,
    cancel: oneshot::Receiver<()>,
    offset: Option<usize>,
    query: Option<&SearchQuery>,
) -> Result<UpstreamBrowsePage, ModelError> {
    let snapshot = &prepared.snapshot;
    let result_offset = offset.unwrap_or(0);
    let provider_id = &snapshot.provider_id;
    let mut cache =
        registry.load_browse_cache(provider_id, snapshot.revision, &snapshot.base_url)?;

    let needs_fetch = cache.models.is_empty()
        || (!browse_cache_covers_request(&cache, query, result_offset) && cache.upstream_more);
    if needs_fetch && cache.upstream_more {
        extend_browse_catalog(fetcher, prepared, cancel, &mut cache).await?;
        registry.save_browse_cache(provider_id, &cache)?;
    }

    let catalog_may_grow = cache.upstream_more;
    if let Some(query) = query {
        Ok(browse_page_slice_searched(
            &cache.models,
            result_offset,
            query,
            snapshot,
            catalog_may_grow,
        ))
    } else {
        Ok(browse_page_slice(
            &cache.models,
            result_offset,
            snapshot,
            catalog_may_grow,
        ))
    }
}
