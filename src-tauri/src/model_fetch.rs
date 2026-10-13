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
//! Steps 2 and 4 are blocking; the Tauri command runs them with `spawn_blocking`
//! and holds the credential lock around the snapshot. Tests repeat the same step
//! order in a helper that may block.
//!
//! A second entry point, `refresh_saved_models`, downloads the same catalog but
//! updates only already-saved rows (P16.c): it never inserts or deletes, and it
//! preserves `source`, `selected` and `alias`. Its absence rule matches the merge
//! rule: only a complete catalog marks a previously listed row missing.
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

mod browse;
mod cache;
mod download;
mod merge;
mod refresh;
mod registry;
mod repository;
mod service;
mod types;

pub use browse::browse_upstream_page;
pub use download::{ModelFetcher, download_catalog};
pub use merge::merge_catalog;
pub use refresh::refresh_saved_models;
pub use registry::{FetchRegistration, ModelFetchRegistry};
pub use repository::fetch_status;
pub use service::prepare_fetch;
pub use types::{
    BrowseUpstreamModelsRequest, LastModelFetch, ModelFetchStatus, ModelFetchSummary,
    ModelRefreshSummary, PreparedFetch, ProviderSnapshot, UPSTREAM_BROWSE_PAGE_SIZE,
    UpstreamBrowseModel, UpstreamBrowsePage,
};

#[cfg(test)]
mod tests;
