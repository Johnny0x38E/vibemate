//! Shared apply results and safe failures for Agent model configuration.
//! Both adapters use the same file writer and expose no credential values.

use crate::config_apply::{OperationState, StepOutcome, WriteError};
use crate::config_plan::PlanError;
use serde::Serialize;

/// What one apply call did to the settings file.
///
/// The real credential never appears here: the interface shows the state, the
/// per-file outcome, and where the copy of the previous content is.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigApplied {
    /// Identity of the recorded operation; the recovery entry (P34) reads it.
    pub operation_id: String,
    /// Final state: `applied`, or what the rollback reached after a failure.
    pub state: OperationState,
    /// One entry per file, with the failure code when the write did not happen.
    pub steps: Vec<StepOutcome>,
    /// Copy of the previous settings content, when there was one to replace.
    pub backup_path: Option<String>,
    /// Whether a new Agent session is needed. Only a file that now holds
    /// the planned keys requires one; a refused or rolled-back write does not.
    pub restart_required: bool,
}

/// Why a reviewed plan could not be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigApplyError {
    /// The Provider revision, the model content or the settings file changed
    /// since the preview; the user must preview again before anything is written.
    PlanStale,
    /// The installed Agent version has no verified configuration contract.
    AgentVersionUnverified,
    /// The model has no supported native request protocol.
    ProtocolUnsupported,
    /// A higher-priority credential would override the planned key.
    CredentialConflict,
    /// The Provider has no key in the OS credential store.
    CredentialMissing,
    /// The OS credential store could not be read.
    CredentialUnavailable,
    /// The request or a stored value is malformed.
    InvalidRequest,
    /// No provider instance has the requested ID.
    ProviderNotFound,
    /// The provider instance has no saved model with the requested ID.
    ModelNotFound,
    /// This Provider has no verified Claude Code gateway instructions.
    GatewayUnverified,
    /// The instance address is not one the vendor documents.
    BaseUrlUnverified,
    /// The settings file cannot be read, is too large, or its path cannot be used.
    TargetUnreadable,
    /// A key the plan must change holds a value of another shape.
    TargetConflict,
    /// The user configuration directory cannot be resolved.
    LocationUnavailable,
    /// The configuration database is not open, or reading it failed.
    StorageUnavailable,
    /// The write could not be recorded, so its outcome is not reported as one.
    OperationFailed,
}

impl ConfigApplyError {
    /// Map a preview refusal onto the apply step's own codes.
    pub(crate) fn from_plan(error: PlanError) -> Self {
        match error {
            PlanError::AgentVersionUnverified => Self::AgentVersionUnverified,
            PlanError::ProtocolUnsupported => Self::ProtocolUnsupported,
            PlanError::CredentialConflict => Self::CredentialConflict,
            PlanError::InvalidRequest => Self::InvalidRequest,
            PlanError::GatewayUnverified => Self::GatewayUnverified,
            PlanError::BaseUrlUnverified => Self::BaseUrlUnverified,
            PlanError::TargetPathInvalid
            | PlanError::TargetUnreadable
            | PlanError::TargetTooLarge => Self::TargetUnreadable,
            PlanError::TargetConflict => Self::TargetConflict,
            PlanError::StorageUnavailable => Self::StorageUnavailable,
            PlanError::ProviderNotFound => Self::ProviderNotFound,
            PlanError::ModelNotFound => Self::ModelNotFound,
            PlanError::LocationUnavailable => Self::LocationUnavailable,
            PlanError::OperationFailed => Self::OperationFailed,
        }
    }

    /// Map a write-layer failure that stopped the operation from being recorded.
    ///
    /// A file-level failure is a result, not an error: it travels in the step's
    /// `error_code`, and reaching this mapping means the log itself failed.
    pub(crate) fn from_write(error: WriteError) -> Self {
        match error {
            WriteError::StorageUnavailable => Self::StorageUnavailable,
            WriteError::InvalidRequest => Self::InvalidRequest,
            _ => Self::OperationFailed,
        }
    }
}

impl std::fmt::Display for ConfigApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AgentVersionUnverified => {
                "The installed Agent version has no verified configuration contract."
            }
            Self::ProtocolUnsupported => "The model has no supported native request protocol.",
            Self::CredentialConflict => {
                "A higher-priority credential conflicts with the planned key."
            }
            Self::PlanStale => "The configuration changed since the preview.",
            Self::CredentialMissing => "The Provider key is not in the OS credential store.",
            Self::CredentialUnavailable => "The OS credential store could not be read.",
            Self::InvalidRequest => "The apply request is invalid.",
            Self::ProviderNotFound => "The Provider configuration no longer exists.",
            Self::ModelNotFound => "The model is no longer saved for this Provider.",
            Self::GatewayUnverified => "This Provider has no verified Claude Code instructions.",
            Self::BaseUrlUnverified => "The instance address is not one the vendor documents.",
            Self::TargetUnreadable => "The settings file could not be read.",
            Self::TargetConflict => "A settings key holds a value of another shape.",
            Self::LocationUnavailable => "The user configuration directory is unavailable.",
            Self::StorageUnavailable => "Private storage is unavailable.",
            Self::OperationFailed => "The configuration write could not be recorded.",
        })
    }
}

impl std::error::Error for ConfigApplyError {}
