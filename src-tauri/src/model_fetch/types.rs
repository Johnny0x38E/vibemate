//! Non-secret fetch/browse requests and results, plus the internal prepared credential.

use crate::credentials::Secret;
use crate::model_catalog::{IncompleteReason, ModelCapabilities};
use crate::model_identity::RouteSupport;
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol};
use serde::{Deserialize, Serialize};

/// Upstream browse returns this many models per lazy-load page (UI + OpenRouter `limit`).
pub const UPSTREAM_BROWSE_PAGE_SIZE: usize = 50;

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
    pub(super) key: Secret,
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

/// What one saved-model refresh changed, returned to the frontend (P16.c).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRefreshSummary {
    /// Whether the refresh saw the whole list (only then are rows marked missing).
    pub complete: bool,
    pub incomplete_reason: Option<IncompleteReason>,
    /// Saved rows whose metadata was updated from the catalog.
    pub refreshed: usize,
    /// Saved rows newly marked as missing upstream.
    pub marked_missing: usize,
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
    pub capabilities: ModelCapabilities,
    pub route_support: RouteSupport,
    /// Protocol this instance would use for the model (P15).
    pub resolved_protocol: Option<crate::providers::ProviderProtocol>,
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
