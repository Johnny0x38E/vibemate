//! Saved-model domain facade. Local queries, selection/edit transactions, and
//! cursor validation retain their existing public entrypoints and wire formats.
//! IPC error codes live in `model_error` and are re-exported here.

mod edit;
mod repository;
mod selection;
mod types;
mod validation;

pub use crate::model_error::ModelError;
pub(crate) use crate::model_error::parse_provider_id;
pub use edit::{
    GetModelSnapshotRequest, ModelSnapshot, UpdateModelEditRequest, get_model_snapshot,
    update_model_edit,
};
pub use repository::{count_provider_models, list_provider_models};
pub use selection::{
    add_manual_model, delete_manual_model, save_provider_model_selections, set_models_selected,
};
pub use types::{
    AddManualModelRequest, DeleteManualModelRequest, ListProviderModelsRequest,
    MAX_MODEL_PAGE_SIZE, MAX_SELECTION_BATCH, ModelSource, ProviderModel, ProviderModelCount,
    ProviderModelPage, SaveProviderModelEntry, SaveProviderModelSelectionsRequest,
    SetModelsSelectedRequest, UpstreamState,
};
pub use validation::{decode_cursor, encode_cursor};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod edit_tests;
