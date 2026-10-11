//! HTTP model downloads, vendor pagination, cancellation and overall deadlines.
//! Credentials remain borrowed from PreparedFetch and never enter stored metadata.

use super::types::PreparedFetch;
use crate::model_catalog::{
    Catalog, CatalogCollector, OpenRouterPager, PageStep, models_url, openrouter_page_url,
    openrouter_page_url_with_limit, parse_page,
};
use crate::model_error::ModelError;
use crate::model_http::{
    HttpClient, HttpSettings, OPERATION_DEADLINE, run_cancellable, with_deadline,
};
use crate::providers::ProviderKind;
use std::time::Duration;
use tokio::sync::oneshot;

/// The HTTP client and overall deadline used for fetches.
#[derive(Debug, Clone)]
pub struct ModelFetcher {
    /// Shared client used by complete downloads and browse-only page requests.
    pub(super) client: HttpClient,
    /// Overall limit for one complete download or one browse extension.
    pub(super) deadline: Duration,
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

/// Fetch one provider page using a computed offset and an optional browse limit.
/// Link payloads never choose the next request URL. Errors remain stable model codes.
pub(super) async fn download_single_page(
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
