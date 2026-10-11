//! In-memory browse sessions keyed by provider identity, revision and base URL.
//! Cache access never holds its mutex while downloading or touching SQLite.

use super::registry::ModelFetchRegistry;
use crate::model_catalog::CatalogModel;
use crate::models::ModelError;
use crate::providers::ProviderId;

/// Upstream models accumulated for one browse session (all vendors).
#[derive(Debug, Clone)]
pub(super) struct CachedVendorBrowseList {
    revision: i64,
    base_url: String,
    /// Unique upstream rows accumulated so far, in response order.
    pub(super) models: Vec<CatalogModel>,
    /// When true, another provider API page may be downloaded into `models`.
    pub(super) upstream_more: bool,
    /// Next OpenRouter API offset; ignored once the vendor returns a single page.
    pub(super) upstream_api_offset: usize,
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

impl ModelFetchRegistry {
    /// Read browse cache for this provider, or an empty session when stale or missing.
    pub(super) fn load_browse_cache(
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

    pub(super) fn save_browse_cache(
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
}
