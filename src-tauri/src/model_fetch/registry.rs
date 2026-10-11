//! Per-provider registration, cancellation and the guard that unregisters on Drop.
//! A registration survives through merging; cancellation cannot interrupt a DB commit.

use super::cache::CachedVendorBrowseList;
use crate::model_error::ModelError;
use crate::providers::ProviderId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Running fetches, one per provider. Clones share the same registry.
#[derive(Debug, Clone, Default)]
pub struct ModelFetchRegistry {
    /// Provider ID → the cancel sender, or `None` once the fetch is merging.
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
    /// Provider ID → catalog accumulated across lazy browse requests.
    pub(super) browse_lists: Arc<Mutex<HashMap<String, CachedVendorBrowseList>>>,
}

/// Proof that a fetch is registered. Dropping it unregisters the fetch.
#[derive(Debug)]
pub struct FetchRegistration {
    running: Arc<Mutex<HashMap<String, Option<oneshot::Sender<()>>>>>,
    provider_id: String,
}

impl ModelFetchRegistry {
    /// Create an empty registry shared by its clones.
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
