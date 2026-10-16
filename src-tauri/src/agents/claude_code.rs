//! Claude Code configuration preview.
//!
//! Maps a saved Provider instance and model to the gateway variables Claude Code
//! documents, following the vendor instructions recorded in
//! `docs/integrations/claude-code.md`. Only combinations with official
//! instructions are planned; anything else is refused with a reason instead of
//! being guessed. This module never writes a file, never reads a credential
//! value and never sends a request.

pub use super::configuration::{
    ConfigApplied as ClaudeCodeApplied, ConfigApplyError as ClaudeCodeApplyError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::PathBuf;

use super::AgentId;
use crate::config_apply::{
    BackupStore, OperationKind, OperationState, PlannedFile, WriteTarget, apply_files,
};
use crate::config_plan::{
    ConfigPlan, NoticeCode, PlanError, PlanNotice, PlanSource, PlannedValue, PlannedWrite,
    planned_changes, read_target_document,
};
use crate::credentials::CredentialStore;
use crate::model_identity::is_valid_model_id;
use crate::providers::{ProviderId, ProviderKind};
use crate::storage::Storage;

/// The settings filename Claude Code reads from the user configuration root.
const SETTINGS_FILE: &str = "settings.json";

/// Keys that decide which model Claude Code's own surfaces run on.
///
/// `ANTHROPIC_MODEL` only picks the model a session starts with. The `opus`,
/// `sonnet` and `haiku` aliases keep resolving through their own variables, and
/// subagents, teammate and workflow agents fall back to
/// `CLAUDE_CODE_SUBAGENT_MODEL`; on a user settings file that already carried
/// those keys, a plan that wrote only the session model would leave every one
/// of those surfaces on the previous model.
///
/// Claude Code documents all four as environment variables, and its example
/// puts them in the settings `env` block, so they are written next to the
/// session model. The value is the selected model ID exactly as stored: `[1m]`
/// is Claude Code's own context-window convention and no verified source says
/// how it applies to a gateway ID, so the plan never appends a suffix.
const CLASS_MODEL_KEYS: &[&str] = &[
    "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
    "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
    "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "env.CLAUDE_CODE_SUBAGENT_MODEL",
];

/// How confidently vibemate can point Claude Code at a Provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayStatus {
    /// Official instructions exist for this Provider.
    Verified,
    /// Official instructions exist and state a limitation.
    VerifiedWithLimits,
    /// No official instructions were found; no plan is built.
    Unverified,
}

/// Vendor gateway facts for one Provider kind.
struct Gateway {
    /// Address written to `ANTHROPIC_BASE_URL`.
    base_url: &'static str,
    /// Key the credential is written to.
    credential_key: &'static str,
    /// Addresses vibemate treats as an unmodified instance of this Provider.
    vendor_bases: &'static [&'static str],
    /// Further keys the vendor requires, with their literal values.
    extra_writes: &'static [(&'static str, &'static str)],
    /// Notices that always apply to this Provider.
    notices: &'static [NoticeCode],
    /// Combination status shown in the interface.
    status: GatewayStatus,
}

/// Vendor gateway facts, or `None` when no official instructions were found.
///
/// Command Code publishes an Anthropic-schema route but no Claude Code setup, so
/// it stays unplanned rather than being presented as supported.
fn gateway(kind: ProviderKind) -> Option<Gateway> {
    match kind {
        ProviderKind::DeepSeek => Some(Gateway {
            base_url: "https://api.deepseek.com/anthropic",
            credential_key: "env.ANTHROPIC_AUTH_TOKEN",
            // Both DeepSeek addresses are documented vendor defaults; an instance
            // using either one is still the vendor's own endpoint.
            vendor_bases: &[
                "https://api.deepseek.com",
                "https://api.deepseek.com/anthropic",
            ],
            extra_writes: &[],
            notices: &[],
            status: GatewayStatus::Verified,
        }),
        ProviderKind::OpenRouter => Some(Gateway {
            // The Claude Code gateway base differs from the `/api/v1` base used
            // for vibemate's own requests; both end at the same messages route.
            base_url: "https://openrouter.ai/api",
            credential_key: "env.ANTHROPIC_AUTH_TOKEN",
            vendor_bases: &["https://openrouter.ai/api/v1"],
            // The vendor requires an explicitly empty API key so Claude Code
            // cannot fall back to authenticating against Anthropic directly.
            extra_writes: &[("env.ANTHROPIC_API_KEY", "")],
            notices: &[NoticeCode::ProviderLimited],
            status: GatewayStatus::VerifiedWithLimits,
        }),
        ProviderKind::CommandCode => None,
    }
}

/// Combination status for the interface; also used before a plan is requested.
pub fn gateway_status(kind: ProviderKind) -> GatewayStatus {
    gateway(kind)
        .map(|gateway| gateway.status)
        .unwrap_or(GatewayStatus::Unverified)
}

/// Facts the preview needs about the selected Provider instance.
pub struct ProviderFacts {
    /// Stored Provider instance ID; already validated when it was stored.
    pub id: String,
    /// Provider kind that selects the vendor instructions.
    pub kind: ProviderKind,
    /// Stored instance base URL, used only to refuse an unverified address.
    pub base_url: String,
    /// Stored `revision`, the plan's concurrency token.
    pub revision: i64,
}

/// Facts about the selected model.
pub struct ModelFacts {
    /// Upstream model ID, written to `ANTHROPIC_MODEL` and to the class keys in
    /// [`CLASS_MODEL_KEYS`].
    pub id: String,
    /// SHA-256 of the stored model record.
    pub content_digest: String,
}

/// One Claude Code preview request.
///
/// `config_root` is the resolved user settings directory (`CLAUDE_CONFIG_DIR` or
/// `<home>/.claude`), so tests never touch a real home directory.
pub struct ClaudeCodePlanInput {
    /// User settings root Claude Code reads.
    pub config_root: PathBuf,
    /// Selected Provider instance.
    pub provider: ProviderFacts,
    /// Selected model.
    pub model: ModelFacts,
}

/// Build a read-only plan for pointing Claude Code at the selected model.
///
/// # Errors
/// Returns `GatewayUnverified` for a Provider without official Claude Code
/// instructions, `BaseUrlUnverified` when the instance address is not one the
/// vendor documents, `InvalidRequest` for a malformed revision, model ID or
/// digest, and the [`read_target_document`] errors for the settings file.
pub fn plan_claude_code_settings(input: &ClaudeCodePlanInput) -> Result<ConfigPlan, PlanError> {
    build(input).map(|(plan, _document)| plan)
}

/// Build the plan together with the document it would produce.
///
/// The merged document stays out of [`ConfigPlan`] on purpose: it holds the
/// user's whole settings file, including credentials of other tools, and must
/// not travel to the interface. The write step (P19) rebuilds it from the file.
fn build(input: &ClaudeCodePlanInput) -> Result<(ConfigPlan, Map<String, Value>), PlanError> {
    let gateway = gateway(input.provider.kind).ok_or(PlanError::GatewayUnverified)?;
    if !gateway
        .vendor_bases
        .contains(&input.provider.base_url.as_str())
    {
        return Err(PlanError::BaseUrlUnverified);
    }
    if input.provider.revision < 1
        || !is_valid_model_id(&input.model.id)
        || !is_digest(&input.model.content_digest)
    {
        return Err(PlanError::InvalidRequest);
    }

    let read = read_target_document(&input.config_root.join(SETTINGS_FILE))?;
    let target = read.target;
    let mut document = read.document;
    // Read before writing: the notice explains that the planned environment
    // variable takes precedence over this file-level default.
    let previous_model_key = document.get("model").map(display_value);

    let mut writes = vec![
        PlannedWrite {
            key_path: "env.ANTHROPIC_BASE_URL",
            value: PlannedValue::Literal(gateway.base_url.to_owned()),
        },
        PlannedWrite {
            key_path: gateway.credential_key,
            value: PlannedValue::Credential,
        },
        PlannedWrite {
            key_path: "env.ANTHROPIC_MODEL",
            value: PlannedValue::Literal(input.model.id.clone()),
        },
    ];
    // The class keys carry the same model so the aliases, subagents and
    // background work follow the session model instead of the user's previous
    // values for those surfaces.
    writes.extend(CLASS_MODEL_KEYS.iter().map(|&key_path| PlannedWrite {
        key_path,
        value: PlannedValue::Literal(input.model.id.clone()),
    }));
    writes.extend(
        gateway
            .extra_writes
            .iter()
            .map(|(key_path, value)| PlannedWrite {
                key_path,
                value: PlannedValue::Literal((*value).to_owned()),
            }),
    );
    let changes = planned_changes(&mut document, &writes)?;

    let mut notices = vec![
        PlanNotice::code(NoticeCode::CredentialFromStore),
        PlanNotice::code(NoticeCode::OverridesNotChecked),
    ];
    if let Some(previous) = previous_model_key {
        notices.push(PlanNotice::with_detail(
            NoticeCode::ModelKeyOverridden,
            previous,
        ));
    }
    notices.extend(gateway.notices.iter().copied().map(PlanNotice::code));

    let plan = ConfigPlan {
        agent: AgentId::ClaudeCode,
        target,
        source: PlanSource {
            provider_id: input.provider.id.clone(),
            provider_revision: input.provider.revision,
            model_id: input.model.id.clone(),
            model_content_digest: input.model.content_digest.clone(),
        },
        changes,
        notices,
        // The planned keys live in the `env` block, which a running session
        // cannot pick up; `model` is read at session start as well.
        restart_required: true,
    };
    Ok((plan, document))
}

/// Tokens the interface sends back from the plan the user reviewed.
///
/// The exact key set is enforced on the way in: a payload this build does not
/// understand is refused instead of being applied with missing tokens.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedPlan {
    /// Settings file the plan was built for.
    pub target_path: String,
    /// SHA-256 of that file when the plan was built; `None` when it did not exist.
    pub target_digest: Option<String>,
    /// Provider `revision` the plan bound.
    pub provider_revision: i64,
    /// Model content digest the plan bound.
    pub model_content_digest: String,
}

/// Everything one apply call needs besides the database and the credential store.
pub struct ClaudeCodeApplyInput {
    /// Provider, model and settings root, read again by the caller.
    pub plan: ClaudeCodePlanInput,
    /// Tokens the user reviewed.
    pub reviewed: ReviewedPlan,
    /// Root of the private backup directory, normally the app-data folder.
    pub backups: PathBuf,
}

/// Apply a reviewed Claude Code plan: write the settings file with the Provider key.
///
/// The plan is rebuilt from the current Provider, model and file state first, and
/// every token the user reviewed is compared again, so an edit made after the
/// preview is refused with [`ClaudeCodeApplyError::PlanStale`] instead of being
/// overwritten. The key is read from the OS credential store at this point, put
/// where the plan showed the placeholder, and written through the shared
/// configuration write, which keeps a private copy of the previous content and
/// records the operation.
///
/// # Errors
/// Returns `PlanStale` for any changed token, `CredentialMissing` or
/// `CredentialUnavailable` for the key, the preview refusals for an unusable
/// Provider or file, and `OperationFailed` when the write could not be recorded.
/// A file-level write failure is reported in the returned steps, not as an error.
pub fn apply_claude_code_settings(
    storage: &Storage,
    credentials: &impl CredentialStore,
    input: &ClaudeCodeApplyInput,
) -> Result<ClaudeCodeApplied, ClaudeCodeApplyError> {
    let (plan, document) = build(&input.plan).map_err(ClaudeCodeApplyError::from_plan)?;
    if plan.target.path != input.reviewed.target_path
        || plan.target.content_digest != input.reviewed.target_digest
        || plan.source.provider_revision != input.reviewed.provider_revision
        || plan.source.model_content_digest != input.reviewed.model_content_digest
    {
        return Err(ClaudeCodeApplyError::PlanStale);
    }
    let credential_key = gateway(input.plan.provider.kind)
        .map(|gateway| gateway.credential_key)
        .ok_or(ClaudeCodeApplyError::GatewayUnverified)?;
    // The reference is built from the stored ID, never from the display name.
    let reference = ProviderId::parse(&input.plan.provider.id)
        .ok_or(ClaudeCodeApplyError::InvalidRequest)?
        .credential_reference();
    let secret = credentials
        .load(&reference)
        .map_err(|_| ClaudeCodeApplyError::CredentialUnavailable)?
        .ok_or(ClaudeCodeApplyError::CredentialMissing)?;

    let mut document = document;
    put_credential(&mut document, credential_key, secret.expose())?;
    let mut content =
        serde_json::to_vec_pretty(&document).map_err(|_| ClaudeCodeApplyError::OperationFailed)?;
    // Editors and Claude Code both accept a trailing newline.
    content.push(b'\n');

    let backups = BackupStore::new(input.backups.clone());
    let files = [PlannedFile {
        target: WriteTarget::from_plan(&plan.target),
        content,
    }];
    let applied = apply_files(
        storage,
        OperationKind::ClaudeCodeSettings,
        // The stored Provider ID labels the operation, so the restore view can
        // say which key the written file carried without reading any key.
        Some(plan.source.provider_id.as_str()),
        &files,
        &backups,
    )
    .map_err(ClaudeCodeApplyError::from_write)?;
    Ok(ClaudeCodeApplied {
        // Only a settings file that now holds the planned keys needs a new
        // session; a refused or rolled-back write leaves the previous content.
        restart_required: applied.state == OperationState::Applied,
        operation_id: applied.operation_id,
        state: applied.state,
        backup_path: applied
            .steps
            .first()
            .and_then(|step| step.backup_path.clone()),
        steps: applied.steps,
    })
}

/// Put the real key where the plan showed the credential placeholder.
///
/// The document stays inside this call; only the written file and the private
/// backup ever hold the value.
fn put_credential(
    document: &mut Map<String, Value>,
    key_path: &str,
    value: &str,
) -> Result<(), ClaudeCodeApplyError> {
    let (parent, key) = key_path
        .rsplit_once('.')
        .ok_or(ClaudeCodeApplyError::InvalidRequest)?;
    let mut cursor = document;
    for segment in parent.split('.') {
        let next = cursor
            .get_mut(segment)
            .and_then(Value::as_object_mut)
            .ok_or(ClaudeCodeApplyError::InvalidRequest)?;
        cursor = next;
    }
    cursor.insert(key.to_owned(), Value::String(value.to_owned()));
    Ok(())
}

/// Whether the text is a lowercase SHA-256 token, the model digest format.
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
/// Render a JSON value as short display text.
fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod apply_tests;

mod restore;
pub use restore::{
    ApplyClaudeCodeRestoreRequest, ClaudeCodeOperation, ClaudeCodeOperationStep,
    ClaudeCodeRestoreError, ClaudeCodeRestorePlan, ClaudeCodeRestoreStep,
    ReviewedRestoreStepRequest, apply_claude_code_restore, apply_config_restore,
    list_claude_code_operations, list_config_operations, plan_claude_code_restore,
    plan_config_restore,
};

#[cfg(test)]
mod restore_tests;
