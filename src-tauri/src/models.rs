//! Saved-model domain facade. Local queries, selection/edit transactions, and
//! cursor validation retain their existing public entrypoints and wire formats.

mod types;
pub use types::{
    AddManualModelRequest, DeleteManualModelRequest, ListProviderModelsRequest,
    MAX_MODEL_PAGE_SIZE, MAX_SELECTION_BATCH, ModelError, ModelSource, ProviderModel,
    ProviderModelPage, SaveProviderModelEntry, SaveProviderModelSelectionsRequest,
    SetModelsSelectedRequest, UpstreamState,
};
mod validation;
pub(crate) use validation::parse_provider_id;
pub use validation::{decode_cursor, encode_cursor};
mod repository;
pub use repository::list_provider_models;
pub(crate) use repository::lock;
mod selection;
pub use selection::{
    add_manual_model, delete_manual_model, save_provider_model_selections, set_models_selected,
};

#[cfg(test)]
mod tests;
