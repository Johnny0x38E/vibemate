//! Fetch a provider's model list and merge it into `provider_model` (P12).
//!
//! A fetch runs only when the user asks for it. The steps, in order:
//!
//! 1. **Register** in `ModelFetchRegistry`. A second fetch for the same provider
//!    is refused with `model_fetch_in_progress`. Registering returns the cancel
//!    signal for this fetch.
//! 2. **Snapshot** (`prepare_fetch`): read kind, protocol, base URL and revision
//!    under the database lock, release the lock, then read the API key from the
//!    credential store. The store can show an OS prompt, so the lock is never held
//!    while it runs. No key reference or no stored key is `secret_missing`.
//! 3. **Download** (`download_catalog`): send `GET <base_url>/models` with
//!    `Authorization: Bearer <key>` to every vendor (decision D17). OpenRouter is
//!    paged with offsets vibemate computes itself. Cancelling drops the request at
//!    once, and the whole download has an overall deadline (90 s in production).
//! 4. **Merge** (`merge_catalog`): in one `IMMEDIATE` transaction, check that the
//!    provider still has the snapshot's revision and base URL (else
//!    `model_fetch_stale`; deleted is `not_found`), then apply the merge rules and
//!    record the fetch summary. Once merging starts, cancelling has no effect.
//!
//! Steps 2 and 4 are blocking; the Tauri command (P12.b.1) runs them with
//! `spawn_blocking`. `fetch_provider_models` runs all steps in order for callers
//! that may block, such as tests.
//!
//! Merge rules (a complete fetch saw the whole, non-empty list without hitting a cap):
//! - A listed model that has no row is inserted as `fetched` and **not selected**.
//! - A listed model that has a row gets fresh metadata, `last_seen_at = now` and
//!   no `missing_since`. Its `source`, `selected` and `alias` never change, so a
//!   manual row stays manual when the vendor later lists the same ID.
//! - On a complete fetch, a row that is not listed:
//!   - fetched and not selected: deleted;
//!   - fetched and selected: kept, and `missing_since` is set (if not already);
//!   - manual and listed before: kept, and `missing_since` is set (if not already);
//!   - manual and never listed: unchanged.
//! - An incomplete fetch (page cap, model cap, or an empty list) only inserts and
//!   updates. It never marks a model missing and never deletes one.
//!
//! The API key exists only in the `Secret` passed to the HTTP client. Errors are
//! payload-free codes, and nothing here logs, stores or returns the key.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::credentials::{CredentialStore, Secret};
use crate::http_client::{
    HttpClient, HttpSettings, OPERATION_DEADLINE, run_cancellable, with_deadline,
};
use crate::model_catalog::{
    Catalog, CatalogCollector, CatalogModel, IncompleteReason, OpenRouterPager, PageStep,
    RouteSupport, models_url, openrouter_page_url, openrouter_page_url_with_limit, parse_page,
    route_support,
};
use crate::model_search::{MatchScore, SearchQuery, sort_ranked};

/// Upstream browse returns this many models per lazy-load page (UI + OpenRouter `limit`).
pub const UPSTREAM_BROWSE_PAGE_SIZE: usize = 50;

use crate::models::{ModelError, lock, parse_provider_id};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol};
use crate::storage::Storage;

/// The provider settings a fetch was started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshot {
    pub provider_id: ProviderId,
    pub kind: ProviderKind,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub revision: i64,
}

/// A snapshot plus the key read for it. `Secret`'s `Debug` output is redacted.
#[derive(Debug)]
pub struct PreparedFetch {
    pub snapshot: ProviderSnapshot,
    key: Secret,
}

/// What one fetch changed, returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFetchSummary {
    /// Whether the fetch saw the whole list (only then are models marked or pruned).
    pub complete: bool,
    pub incomplete_reason: Option<IncompleteReason>,
    /// Unique valid models in the vendor's response.
    pub listed: usize,
    /// Listed models that had no row and were inserted, not selected.
    pub added: usize,
    /// Listed models whose existing row was refreshed.
    pub updated: usize,
    /// Rows newly marked as missing upstream.
    pub marked_missing: usize,
    /// Unselected fetched rows deleted because the vendor no longer lists them.
    pub pruned: usize,
    /// Entries skipped because their ID was invalid or repeated.
    pub skipped_invalid: usize,
    pub fetched_at_ms: i64,
}

/// The last saved fetch of a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastModelFetch {
    pub fetched_at_ms: i64,
    pub complete: bool,
    pub listed_count: i64,
}

/// Whether a fetch is running and how the last saved one went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFetchStatus {
    pub running: bool,
    pub last_fetch: Option<LastModelFetch>,
}

/// The HTTP client and overall deadline used for fetches.
#[derive(Debug, Clone)]
pub struct ModelFetcher {
    client: HttpClient,
    deadline: Duration,
}

impl ModelFetcher {
    /// The production fetcher: system proxy, HTTPS only, 90 s overall deadline.
    ///
    /// # Errors
    /// `OperationFailed` when the HTTP client cannot be built.
    pub fn production() -> Result<Self, ModelError> {
        Ok(Self {
            client: HttpClient::new(HttpSettings::production())?,
            deadline: OPERATION_DEADLINE,
        })
    }

    /// A fetcher for the local test server, with a chosen overall deadline.
    #[cfg(test)]
    pub(crate) fn for_mock_server(deadline: Duration) -> Self {
        Self {
            client: HttpClient::new(HttpSettings::for_mock_server()).expect("test client"),
            deadline,
        }
    }
}

/// Upstream models accumulated for one browse session (all vendors).
#[derive(Debug, Clone)]
struct CachedVendorBrowseList {
    revision: i64,
    base_url: String,
    models: Vec<CatalogModel>,
    /// When true, another provider API page may be downloaded into `models`.
    upstream_more: bool,
    /// Next OpenRouter API offset; ignored once the vendor returns a single page.
    upstream_api_offset: usize,
}

impl CachedVendorBrowseList {
    fn fresh(revision: i64, base_url: String) -> Self {
        Self {
            revision,
            base_url,
            models: Vec::new(),
            upstream_more: true,
            upstream_api_offset: 0,
        }
    }
}

/// Running fetches, one per provider. Clones share the same registry.
#[derive(Debug, Clone, Default)]
pub struct ModelFetchRegistry {
    /// Provider ID → the cancel sender, or `None` once the fetch is merging.
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
    /// Provider ID → catalog accumulated across lazy browse requests.
    browse_lists: Arc<Mutex<HashMap<String, CachedVendorBrowseList>>>,
}

/// Proof that a fetch is registered. Dropping it unregisters the fetch.
#[derive(Debug)]
pub struct FetchRegistration {
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
    provider_id: String,
}

impl ModelFetchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fetch and return its registration and cancel signal.
    ///
    /// # Errors
    /// `ModelFetchInProgress` when this provider already has a running fetch.
    pub fn begin(
        &self,
        provider_id: &ProviderId,
    ) -> Result<(FetchRegistration, oneshot::Receiver<()>), ModelError> {
        let mut running = self
            .running
            .lock()
            .map_err(|_| ModelError::OperationFailed)?;
        if running.contains_key(provider_id.as_str()) {
            return Err(ModelError::ModelFetchInProgress);
        }
        let (sender, receiver) = oneshot::channel();
        running.insert(provider_id.as_str().to_owned(), Some(sender));
        Ok((
            FetchRegistration {
                running: Arc::clone(&self.running),
                provider_id: provider_id.as_str().to_owned(),
            },
            receiver,
        ))
    }

    /// Ask a running fetch to stop. Returns whether a download was still running
    /// and received the signal; `false` when nothing runs or it is already merging.
    pub fn cancel(&self, provider_id: &ProviderId) -> bool {
        let Ok(mut running) = self.running.lock() else {
            return false;
        };
        match running.get_mut(provider_id.as_str()).and_then(Option::take) {
            Some(sender) => sender.send(()).is_ok(),
            None => false,
        }
    }

    /// Read browse cache for this provider, or an empty session when stale or missing.
    fn load_browse_cache(
        &self,
        provider_id: &ProviderId,
        revision: i64,
        base_url: &str,
    ) -> Result<CachedVendorBrowseList, ModelError> {
        let lists = self
            .browse_lists
            .lock()
            .map_err(|_| ModelError::OperationFailed)?;
        Ok(lists
            .get(provider_id.as_str())
            .filter(|entry| entry.revision == revision && entry.base_url == base_url)
            .cloned()
            .unwrap_or_else(|| CachedVendorBrowseList::fresh(revision, base_url.to_owned())))
    }

    fn save_browse_cache(
        &self,
        provider_id: &ProviderId,
        cache: &CachedVendorBrowseList,
    ) -> Result<(), ModelError> {
        let mut lists = self
            .browse_lists
            .lock()
            .map_err(|_| ModelError::OperationFailed)?;
        lists.insert(provider_id.as_str().to_owned(), cache.clone());
        Ok(())
    }

    /// Whether a fetch for this provider is registered.
    pub fn is_running(&self, provider_id: &ProviderId) -> bool {
        self.running
            .lock()
            .is_ok_and(|running| running.contains_key(provider_id.as_str()))
    }
}

impl FetchRegistration {
    /// Mark the fetch as merging: from now on `cancel` reports `false`.
    pub fn enter_merge(&self) {
        if let Ok(mut running) = self.running.lock()
            && let Some(slot) = running.get_mut(&self.provider_id)
        {
            *slot = None;
        }
    }
}

impl Drop for FetchRegistration {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&self.provider_id);
        }
    }
}

/// Read the provider snapshot, release the database, then read the key.
///
/// # Errors
/// `NotFound`, `InvalidStoredProvider`, `SecretMissing`, a credential-store code,
/// or a storage code.
pub fn prepare_fetch(
    storage: &Storage,
    store: &dyn CredentialStore,
    provider_id: &ProviderId,
) -> Result<PreparedFetch, ModelError> {
    let (snapshot, reference) = {
        let connection = lock(storage)?;
        let row: Option<(String, String, String, i64, Option<String>)> = connection
            .query_row(
                "SELECT p.kind, p.protocol, p.base_url, p.revision, c.credential_ref
                 FROM provider_instance p
                 LEFT JOIN provider_credential c ON c.provider_id = p.id
                 WHERE p.id = ?1",
                [provider_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| ModelError::ReadFailed)?;
        let (kind, protocol, base_url, revision, reference) = row.ok_or(ModelError::NotFound)?;
        let snapshot = ProviderSnapshot {
            provider_id: provider_id.clone(),
            kind: ProviderKind::from_value(&kind).ok_or(ModelError::InvalidStoredProvider)?,
            protocol: ProviderProtocol::from_value(&protocol)
                .ok_or(ModelError::InvalidStoredProvider)?,
            base_url,
            revision,
        };
        (snapshot, reference)
        // The connection guard is dropped here, before the credential store runs.
    };
    let reference = reference.ok_or(ModelError::SecretMissing)?;
    let key = store.load(&reference)?.ok_or(ModelError::SecretMissing)?;
    Ok(PreparedFetch { snapshot, key })
}

/// Download and parse the whole list, stopping at once on `cancel` or the deadline.
///
/// # Errors
/// `ModelFetchCancelled`, `RequestTimedOut`, a network or status code,
/// `UpstreamResponseInvalid`, or `InvalidStoredProvider` for an unusable base URL.
pub async fn download_catalog(
    fetcher: &ModelFetcher,
    prepared: &PreparedFetch,
    cancel: oneshot::Receiver<()>,
) -> Result<Catalog, ModelError> {
    let work = with_deadline(fetcher.deadline, download(&fetcher.client, prepared));
    run_cancellable(work, cancel).await
}

/// One upstream model for browse-only UI (no SQLite write).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamBrowseModel {
    pub model_id: String,
    pub upstream_name: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub supported_endpoints: Option<Vec<String>>,
    pub route_support: RouteSupport,
}

/// One lazy upstream page for the Models tab browse view (P12.c.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamBrowsePage {
    pub items: Vec<UpstreamBrowseModel>,
    pub next_offset: Option<usize>,
    pub more_pages: bool,
}

/// Request the next upstream browse page without merging into SQLite.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseUpstreamModelsRequest {
    pub provider_id: String,
    /// Index into the full browse list, or into ranked search matches when `query`
    /// is set. Omit or `0` for the first page of results.
    pub offset: Option<usize>,
    /// Fuzzy search over model ID and upstream name (same rules as the selected list).
    pub query: Option<String>,
}

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
    UpstreamBrowseModel {
        model_id: model.model_id.clone(),
        upstream_name: model.upstream_name.clone(),
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        input_modalities: model.input_modalities.clone(),
        output_modalities: model.output_modalities.clone(),
        supported_endpoints: model.supported_endpoints.clone(),
        route_support: route_support(
            snapshot.kind,
            snapshot.protocol,
            model.supported_endpoints.as_deref(),
        ),
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

async fn download_single_page(
    client: &HttpClient,
    prepared: &PreparedFetch,
    offset: usize,
    openrouter_limit: Option<usize>,
) -> Result<crate::model_catalog::CatalogPage, ModelError> {
    let snapshot = &prepared.snapshot;
    if snapshot.kind != ProviderKind::OpenRouter {
        let url = models_url(&snapshot.base_url)?;
        let body = client.get_bounded(&url, &prepared.key).await?;
        return Ok(parse_page(snapshot.kind, &body)?);
    }
    let url = match openrouter_limit {
        Some(limit) => openrouter_page_url_with_limit(&snapshot.base_url, offset, limit)?,
        None => openrouter_page_url(&snapshot.base_url, offset)?,
    };
    let body = client.get_bounded(&url, &prepared.key).await?;
    Ok(parse_page(snapshot.kind, &body)?)
}

async fn download(client: &HttpClient, prepared: &PreparedFetch) -> Result<Catalog, ModelError> {
    let snapshot = &prepared.snapshot;
    let mut collector = CatalogCollector::new();
    if snapshot.kind != ProviderKind::OpenRouter {
        let url = models_url(&snapshot.base_url)?;
        let body = client.get_bounded(&url, &prepared.key).await?;
        collector.add_page(parse_page(snapshot.kind, &body)?);
        return Ok(collector.finish(false));
    }
    let mut pager = OpenRouterPager::new();
    let mut offset = pager.next_offset();
    loop {
        let url = openrouter_page_url(&snapshot.base_url, offset)?;
        let body = client.get_bounded(&url, &prepared.key).await?;
        let page = parse_page(snapshot.kind, &body)?;
        let step = pager.record(&page);
        collector.add_page(page);
        match step {
            PageStep::Fetch { offset: next } => offset = next,
            PageStep::Done => return Ok(collector.finish(false)),
            PageStep::PageLimitReached => return Ok(collector.finish(true)),
        }
    }
}

/// One existing row, as far as merging needs it.
struct ExistingRow {
    manual: bool,
    selected: bool,
    listed_before: bool,
    missing: bool,
}

/// Check that the provider is unchanged, apply the merge rules, and record the
/// fetch, all in one transaction.
///
/// # Errors
/// `NotFound` when the provider was deleted, `ModelFetchStale` when its revision or
/// base URL changed, or a storage code. On any error nothing is written.
pub fn merge_catalog(
    storage: &Storage,
    snapshot: &ProviderSnapshot,
    catalog: &Catalog,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    let provider_id = snapshot.provider_id.as_str();
    let mut connection = lock(storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ModelError::WriteFailed)?;

    let current: Option<(i64, String)> = transaction
        .query_row(
            "SELECT revision, base_url FROM provider_instance WHERE id = ?1",
            [provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    let (revision, base_url) = current.ok_or(ModelError::NotFound)?;
    if revision != snapshot.revision || base_url != snapshot.base_url {
        return Err(ModelError::ModelFetchStale);
    }

    let mut existing = HashMap::new();
    {
        let mut statement = transaction
            .prepare(
                "SELECT model_id, source, selected, last_seen_at, missing_since
                 FROM provider_model WHERE provider_id = ?1",
            )
            .map_err(|_| ModelError::ReadFailed)?;
        let rows = statement
            .query_map([provider_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ExistingRow {
                        manual: row.get::<_, String>(1)? == "manual",
                        selected: row.get::<_, i64>(2)? == 1,
                        listed_before: row.get::<_, Option<i64>>(3)?.is_some(),
                        missing: row.get::<_, Option<i64>>(4)?.is_some(),
                    },
                ))
            })
            .map_err(|_| ModelError::ReadFailed)?;
        for row in rows {
            let (model_id, state) = row.map_err(|_| ModelError::ReadFailed)?;
            existing.insert(model_id, state);
        }
    }

    let mut summary = ModelFetchSummary {
        complete: catalog.incomplete.is_none(),
        incomplete_reason: catalog.incomplete,
        listed: catalog.models.len(),
        added: 0,
        updated: 0,
        marked_missing: 0,
        pruned: 0,
        skipped_invalid: catalog.skipped_invalid + catalog.duplicates,
        fetched_at_ms: now_ms,
    };

    let mut listed = HashSet::with_capacity(catalog.models.len());
    for model in &catalog.models {
        listed.insert(model.model_id.as_str());
        let json = |list: &Option<Vec<String>>| -> Result<Option<String>, ModelError> {
            list.as_ref()
                .map(|items| serde_json::to_string(items).map_err(|_| ModelError::WriteFailed))
                .transpose()
        };
        let input = json(&model.input_modalities)?;
        let output = json(&model.output_modalities)?;
        let endpoints = json(&model.supported_endpoints)?;
        if existing.contains_key(&model.model_id) {
            transaction
                .execute(
                    "UPDATE provider_model SET
                         upstream_name = ?3, context_window = ?4, max_output_tokens = ?5,
                         input_modalities = ?6, output_modalities = ?7,
                         supported_endpoints = ?8, last_seen_at = ?9, missing_since = NULL,
                         updated_at = max(updated_at, ?9)
                     WHERE provider_id = ?1 AND model_id = ?2",
                    params![
                        provider_id,
                        model.model_id,
                        model.upstream_name,
                        model.context_window,
                        model.max_output_tokens,
                        input,
                        output,
                        endpoints,
                        now_ms
                    ],
                )
                .map_err(|_| ModelError::WriteFailed)?;
            summary.updated += 1;
        } else {
            transaction
                .execute(
                    "INSERT INTO provider_model
                         (provider_id, model_id, source, selected, upstream_name, context_window,
                          max_output_tokens, input_modalities, output_modalities,
                          supported_endpoints, created_at, updated_at, last_seen_at)
                     VALUES (?1, ?2, 'fetched', 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?9)",
                    params![
                        provider_id,
                        model.model_id,
                        model.upstream_name,
                        model.context_window,
                        model.max_output_tokens,
                        input,
                        output,
                        endpoints,
                        now_ms
                    ],
                )
                .map_err(|_| ModelError::WriteFailed)?;
            summary.added += 1;
        }
    }

    if summary.complete {
        for (model_id, row) in &existing {
            if listed.contains(model_id.as_str()) {
                continue;
            }
            if !row.manual && !row.selected {
                transaction
                    .execute(
                        "DELETE FROM provider_model WHERE provider_id = ?1 AND model_id = ?2",
                        params![provider_id, model_id],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
                summary.pruned += 1;
            } else if row.listed_before && !row.missing {
                transaction
                    .execute(
                        "UPDATE provider_model
                         SET missing_since = ?3, updated_at = max(updated_at, ?3)
                         WHERE provider_id = ?1 AND model_id = ?2",
                        params![provider_id, model_id, now_ms],
                    )
                    .map_err(|_| ModelError::WriteFailed)?;
                summary.marked_missing += 1;
            }
        }
    }

    let listed_count = i64::try_from(summary.listed).map_err(|_| ModelError::WriteFailed)?;
    transaction
        .execute(
            "INSERT INTO provider_model_fetch (provider_id, fetched_at, complete, listed_count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (provider_id) DO UPDATE SET
                 fetched_at = excluded.fetched_at,
                 complete = excluded.complete,
                 listed_count = excluded.listed_count",
            params![provider_id, now_ms, summary.complete, listed_count],
        )
        .map_err(|_| ModelError::WriteFailed)?;
    transaction.commit().map_err(|_| ModelError::WriteFailed)?;
    Ok(summary)
}

/// Run a whole fetch: register, snapshot and key, download, merge.
///
/// The snapshot and merge steps block on SQLite and the credential store, so call
/// this only where blocking is allowed. The Tauri command runs those steps with
/// `spawn_blocking` instead.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `ModelFetchInProgress`, and every error of
/// `prepare_fetch`, `download_catalog` and `merge_catalog`.
pub async fn fetch_provider_models(
    storage: &Storage,
    store: &dyn CredentialStore,
    fetcher: &ModelFetcher,
    registry: &ModelFetchRegistry,
    provider_id: &str,
    now_ms: i64,
) -> Result<ModelFetchSummary, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let (registration, cancel) = registry.begin(&provider_id)?;
    let prepared = prepare_fetch(storage, store, &provider_id)?;
    let catalog = download_catalog(fetcher, &prepared, cancel).await?;
    // The key is no longer needed; drop it before touching the database again.
    let PreparedFetch { snapshot, key } = prepared;
    drop(key);
    registration.enter_merge();
    merge_catalog(storage, &snapshot, &catalog, now_ms)
}

/// Report whether a fetch is running and the last saved fetch.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `NotFound`, or a storage code.
pub fn fetch_status(
    storage: &Storage,
    registry: &ModelFetchRegistry,
    provider_id: &str,
) -> Result<ModelFetchStatus, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let connection = lock(storage)?;
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM provider_instance WHERE id = ?1",
            [provider_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    exists.ok_or(ModelError::NotFound)?;
    let last_fetch = connection
        .query_row(
            "SELECT fetched_at, complete, listed_count FROM provider_model_fetch
             WHERE provider_id = ?1",
            [provider_id.as_str()],
            |row| {
                Ok(LastModelFetch {
                    fetched_at_ms: row.get(0)?,
                    complete: row.get::<_, i64>(1)? == 1,
                    listed_count: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    Ok(ModelFetchStatus {
        running: registry.is_running(&provider_id),
        last_fetch,
    })
}

#[cfg(test)]
mod tests;
