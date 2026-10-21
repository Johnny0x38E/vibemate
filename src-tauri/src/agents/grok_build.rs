//! Grok Build model configuration: a credential-blind preview and a recoverable
//! application of `config.toml`.
//!
//! Grok keeps every user setting in one TOML document, so this adapter parses it
//! with `toml_edit` and changes only what it owns: one `[model.<alias>]` table.
//! It deliberately leaves `[models].default` alone. That field selects the model
//! for new sessions and Grok's own picker writes it, so treating it as vibemate
//! data would make the two programs overwrite each other's choice. Comments, key
//! order and unrelated tables — the maintainer's own models, MCP servers, UI
//! settings — must survive untouched, which is what the preview promises. Alias
//! ownership lives in the private
//! `grok_model_binding` table, so an entry the user wrote by hand is never taken
//! over. Owned entries always set TOML `name` to
//! `(vibemate-<Provider>) <model ID>`, and a plan whose API `model` id
//! exactly matches an existing `[model.<alias>]` table name is refused as a
//! target conflict. The preview reads no OS
//! credential and writes nothing; only the apply step takes the key and
//! replaces the file through the shared writer, which backs up, atomically
//! replaces and can roll the write back. Grok itself is never launched here,
//! and the verified field set and refusals are recorded in
//! `docs/integrations/grok-build.md`.

use super::{AgentId, ConfigApplied, ConfigApplyError, DiscoveryEnvironment, DiscoveryInput};
use crate::config_apply::{
    BackupStore, OperationKind, OperationState, PlannedFile, StepState, WriteTarget,
    apply_private_files,
};
use crate::config_plan::{
    ConfigChange, NoticeCode, PlanError, PlanNotice, PlanSource, SECRET_PLACEHOLDER, TargetFile,
    digest_hex, read_target_bytes,
};
use crate::credentials::CredentialStore;
use crate::model_catalog::CapabilityState;
use crate::models::{ModelSnapshot, ProviderModel};
use crate::providers::{ProviderProtocol, ProviderRecord, base_url_for_protocol};
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, table, value};

/// Grok reads all user settings from this one file below its configuration root.
const CONFIG_FILE: &str = "config.toml";

/// Higher-precedence files a written `config.toml` can be overridden by. Their
/// content is never read here: it may hold another Provider's key.
const PRECEDENCE_FILES: [&str; 2] = ["managed_config.toml", "requirements.toml"];

/// Reference rows the installed Grok must document before this adapter writes.
///
/// Grok unpacks `docs/user-guide/26-config-reference.md` into its configuration
/// root at launch, so these rows tie the plan to the version actually installed
/// rather than to an online page. Each row records a native field this adapter
/// consumes, together with the documented type and scope.
const CONTRACT_ROWS: [&str; 9] = [
    "| `model.<id>.api_backend` | `chat_completions / responses / messages` |",
    "| `model.<id>.api_key` | `string` |",
    "| `model.<id>.base_url` | `string` |",
    "| `model.<id>.context_window` | `number` |",
    "| `model.<id>.vision` | `boolean` |",
    "| `model.<id>.env_key` | `string / string[]` |",
    "| `model.<id>.model` | `string` |",
    "| `model.<id>.name` | `string` |",
    "| `models.disabled_models` | `string[]` |",
];

/// Whether a higher-precedence configuration file was located.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GrokLayerState {
    /// Nothing to override the planned file at this layer.
    Absent,
    /// The file exists; its content was not read, so it may still override.
    Present,
    /// The path exists but is not a readable regular file.
    Unavailable,
}

/// One higher-precedence layer shown in the preview.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokPrecedenceFile {
    /// Fixed file name from the adapter, never taken from a directory listing.
    pub name: &'static str,
    /// Existence and readability, without content or a digest.
    pub state: GrokLayerState,
}

/// The single file a Grok plan changes, with its field-level diff.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokPlannedFile {
    /// File identity and exact-byte concurrency token.
    pub target: TargetFile,
    /// Field changes; a planned key is always masked on both sides.
    pub changes: Vec<ConfigChange>,
}

/// Where the planned context capacity comes from, not proof of Provider support.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GrokContextSource {
    /// Saved centrally; its original provenance is not recorded.
    Library,
    /// No central capacity was declared; native configuration is left alone.
    NotStated,
}

/// How the plan treats the native context capacity.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GrokContextState {
    /// Writes the saved central capacity.
    Written,
    /// Keeps an existing native field, without claiming its value is valid.
    Unchanged,
    /// No native field exists; Grok resolves its own default.
    NotStated,
}

/// A credential-blind context summary built beside the actual TOML write.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokContextWindow {
    /// Central metadata source; independent of the native value kept below.
    pub source: GrokContextSource,
    /// Whether the plan writes, preserves or omits the native field.
    pub state: GrokContextState,
    /// Integer spelling only. Unknown native shapes are never returned as text.
    pub value: Option<String>,
}

/// Where a declared image-input capability came from.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GrokCapabilitySource {
    /// The saved Provider model contains catalog evidence.
    Catalog,
    /// The user explicitly declared this capability in the Provider model.
    UserDeclaration,
    /// The Provider model has no definitive declaration.
    NotStated,
}

/// How the plan maps image-input capability to Grok's `vision` flag.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GrokCapabilityState {
    /// Writes the declared boolean to the native model entry.
    Written,
    /// Preserves an existing boolean and does not claim it is verified.
    Unchanged,
    /// No central or native value is available.
    NotStated,
}

/// Safe summary of the Grok image-input capability mapping.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokImageInput {
    /// Catalog, user or absent source for the declaration.
    pub source: GrokCapabilitySource,
    /// Whether the plan writes, preserves or omits the native flag.
    pub state: GrokCapabilityState,
    /// Boolean only; arbitrary user values are never exposed.
    pub value: Option<bool>,
}

/// A Grok preview contains no credential values and no merged document text.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokPlan {
    /// Fixed adapter identity.
    pub agent: AgentId,
    /// The `[model.<alias>]` table this plan owns.
    pub model_alias: String,
    /// Whether the alias table already existed before this plan.
    pub existing_alias: bool,
    /// Context capacity and its source/write state; not a request output limit.
    pub context_window: GrokContextWindow,
    /// Image-input capability mapping, separate from model behavior proof.
    pub image_input: GrokImageInput,
    /// The file, with every field the plan changes.
    pub file: GrokPlannedFile,
    /// Sources that can outrank the written file, checked for existence only.
    pub precedence: Vec<GrokPrecedenceFile>,
    /// What the user must know before applying.
    pub notices: Vec<PlanNotice>,
    /// Saved provider and model identity and change tokens.
    pub source: PlanSource,
    /// Grok reads configuration when a session starts; a running session keeps
    /// its own model choice, so a new session is required.
    pub restart_required: bool,
}

/// A reviewed file token; arbitrary paths are compared, never used as targets.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedGrokFile {
    /// Exact path the preview showed.
    pub path: String,
    /// SHA-256 the preview saw, or `None` when it saw no file.
    pub content_digest: Option<String>,
}

/// Every token that must still hold before a byte is written.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedGrokPlan {
    /// The alias the preview chose; a different one means the file changed shape.
    pub model_alias: String,
    /// The settings file the preview described.
    pub file: ReviewedGrokFile,
    /// Provider revision the preview was built from.
    pub provider_revision: i64,
    /// Model content digest the preview was built from.
    pub model_content_digest: String,
}

/// One Grok apply request: the selection the user made plus the reviewed tokens.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyGrokSettingsRequest {
    /// Stored Provider instance the plan was built from.
    pub provider_id: String,
    /// Upstream model ID the plan was built from.
    pub model_id: String,
    /// Manual entry path for the same Grok installation the preview used.
    #[serde(default)]
    pub entry_path: Option<String>,
    /// Tokens the interface received with the plan.
    pub reviewed: ReviewedGrokPlan,
}

/// Resolve Grok's user directory and verify the installed configuration contract.
///
/// Three claims must agree: the versioned download the entry resolves to, the
/// `.metadata_version` file Grok writes at launch, and the configuration
/// reference Grok unpacks beside it. Anything else has no verified field set.
/// No file changes and no Grok code runs.
///
/// # Errors
/// `AgentVersionUnverified` when the entry, the recorded version or the unpacked
/// reference does not match the verified contract, `LocationUnavailable` when
/// the configuration root cannot be used, and `InvalidRequest` for a manual path
/// the discovery step rejects.
pub fn configuration_root(
    environment: &DiscoveryEnvironment,
    entry: Option<String>,
) -> Result<PathBuf, PlanError> {
    let observations = super::discover_agents(
        environment,
        &DiscoveryInput {
            grok_path: entry,
            ..DiscoveryInput::default()
        },
    )
    .map_err(|_| PlanError::InvalidRequest)?;
    let observation = observations
        .iter()
        .find(|row| row.id == AgentId::GrokBuild)
        .ok_or(PlanError::AgentVersionUnverified)?;
    let root = observation
        .config_directory
        .as_ref()
        .map(PathBuf::from)
        .ok_or(PlanError::LocationUnavailable)?;
    // Discovery reads the version from the resolved download file name; an entry
    // that is not a regular file, or an unversioned download, is not a claim this
    // adapter can tie to a field set.
    let version = observation
        .version
        .as_deref()
        .filter(|version| verified_patch(version).is_some())
        .ok_or(PlanError::AgentVersionUnverified)?;
    verify_extracted_contract(&root, version)?;
    Ok(root)
}

/// The 1.0 releases this adapter verified: 1.0.60 and later patch releases.
///
/// Grok publishes patch releases weekly and ships its reference with them, so
/// the version window alone is never the claim; `verify_extracted_contract` must
/// still match the installed reference. A non-canonical version text such as
/// `1.0.060` is rejected rather than guessed at.
fn verified_patch(version: &str) -> Option<u32> {
    let patch = version.strip_prefix("1.0.")?;
    let parsed: u32 = patch.parse().ok()?;
    (parsed.to_string() == patch && parsed >= 60).then_some(parsed)
}

/// Check the launch-extracted version marker and configuration reference.
///
/// Both are read as bounded regular files below the configuration root; neither
/// is evaluated, and a symbolic link at either path stops the check instead of
/// following it elsewhere.
fn verify_extracted_contract(root: &Path, version: &str) -> Result<(), PlanError> {
    let (marker, bytes) = read_target_bytes(&root.join(".metadata_version"))?;
    if !marker.exists || marker.symlink || bytes.len() > 64 {
        return Err(PlanError::AgentVersionUnverified);
    }
    let written = std::str::from_utf8(&bytes).map_err(|_| PlanError::AgentVersionUnverified)?;
    if written.trim() != version {
        return Err(PlanError::AgentVersionUnverified);
    }
    let (reference, bytes) =
        read_target_bytes(&root.join("docs/user-guide/26-config-reference.md"))?;
    if !reference.exists || reference.symlink {
        return Err(PlanError::AgentVersionUnverified);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| PlanError::AgentVersionUnverified)?;
    if CONTRACT_ROWS.iter().any(|row| !text.contains(row)) {
        return Err(PlanError::AgentVersionUnverified);
    }
    Ok(())
}

/// Build the read-only preview for `config.toml`.
///
/// The plan binds the settings file digest, the Provider revision and the model
/// content digest, so a later apply step can refuse anything the user changed
/// after the preview. OS credentials are not read: the planned key is a
/// placeholder until the apply step replaces it.
///
/// # Errors
/// `ProtocolUnsupported` for a model whose Grok backend has no verified
/// credential mapping, `BaseUrlUnverified` for an address outside the vendor's
/// documented ones, `TargetUnreadable` for a file that does not parse as TOML,
/// and `TargetConflict` when a field or alias this plan needs is already
/// occupied by something the adapter does not own, or when the planned API
/// `model` id exactly matches an existing `[model.<alias>]` table name.
pub fn plan_grok_settings(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
) -> Result<GrokPlan, PlanError> {
    build(storage, root, provider, snapshot).map(|(plan, _)| plan)
}

/// A parsed `config.toml` with its file identity.
struct TomlTarget {
    target: TargetFile,
    document: DocumentMut,
}

/// Read and parse the settings document without writing anything.
///
/// A missing or blank file parses as an empty document: Grok falls back to its
/// built-in defaults, so an absent file is a normal starting point. A file that
/// does not parse is refused instead of being rewritten, because a rewrite would
/// discard the user's content.
fn read_toml_target(path: &Path) -> Result<TomlTarget, PlanError> {
    let (target, bytes) = read_target_bytes(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| PlanError::TargetUnreadable)?;
    let document = if text.chars().all(char::is_whitespace) {
        DocumentMut::new()
    } else {
        text.parse::<DocumentMut>()
            .map_err(|_| PlanError::TargetUnreadable)?
    };
    Ok(TomlTarget { target, document })
}

/// Build the preview and the merged document behind it.
///
/// The document is returned so the apply step serializes the same merge the card
/// described, with only the credential position filled in.
pub(crate) fn build(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
) -> Result<(GrokPlan, DocumentMut), PlanError> {
    let model = &snapshot.model;
    if !model.selected || model.provider_id != provider.id.as_str() {
        return Err(PlanError::InvalidRequest);
    }
    let protocol = model
        .resolved_protocol
        .ok_or(PlanError::ProtocolUnsupported)?;
    // Grok's `messages` backend authenticates with an `x-api-key` header, and no
    // official text states which header a per-model `api_key` is sent as there.
    // Refuse that combination instead of guessing at the request.
    let backend = match protocol {
        ProviderProtocol::ChatCompletions => "chat_completions",
        ProviderProtocol::Responses => "responses",
        ProviderProtocol::AnthropicMessages => {
            return Err(PlanError::ProtocolUnsupported);
        }
    };
    let base = if protocol == provider.protocol {
        provider.base_url.clone()
    } else if provider.base_url == base_url_for_protocol(provider.kind, provider.protocol) {
        base_url_for_protocol(provider.kind, protocol).to_owned()
    } else {
        return Err(PlanError::BaseUrlUnverified);
    };

    let mut target = read_toml_target(&root.join(CONFIG_FILE))?;
    // The writer refuses to follow a final symbolic link, so a link is a conflict
    // the user must resolve rather than a target to replace.
    if target.target.symlink {
        return Err(PlanError::TargetConflict);
    }
    let alias = choose_alias(storage, root, provider, model, &target.document)?;
    let existing_alias = holds_alias(&target.document, &alias);
    let prefix = format!("model.{alias}");
    let mut notices = Vec::new();
    // Grok treats `[model.<alias>]` names as configuration identities. Writing
    // `model = "<id>"` when `<id>` is already a table name would collide with
    // that identity (for example a handwritten `[model.deepseek-flash]`), so
    // the plan stops instead of producing an ambiguous document.
    if holds_alias(&target.document, &model.model_id) {
        return Err(PlanError::TargetConflict);
    }
    // A disabled or unlisted alias would be written but never selectable, and
    // vibemate does not silently re-enable a model the user turned off.
    if list_contains(&target.document, "models", "disabled_models", &alias)? {
        return Err(PlanError::TargetConflict);
    }
    // An allowlist that names neither the alias nor the model id would leave the
    // written entry unselectable, so the plan stops instead of writing it.
    let allowed = string_list(&target.document, "models", "allowed_models")?;
    if allowed.is_some_and(|names| {
        !names.is_empty()
            && !names
                .iter()
                .any(|entry| entry == &alias || entry == &model.model_id)
    }) {
        return Err(PlanError::TargetConflict);
    }
    if existing_alias && has_env_key(&target.document, &alias) {
        notices.push(PlanNotice::with_detail(
            NoticeCode::EnvKeyOverridden,
            alias.clone(),
        ));
    }

    let mut changes = Vec::new();
    let models = root_table(&mut target.document, "model")?;
    let entry = named_table(models, &alias)?;
    // The TOML `name` is what Grok's model picker shows. It identifies the
    // Provider and upstream model; the table name (`alias`) stays a separate
    // configuration identity. Effort choices belong in native model options.
    let display_name = crate::shared::agent_model_display_name(
        &provider.display_name,
        &model.model_id,
        crate::shared::MAX_AGENT_MODEL_NAME_CHARS,
    );
    set_text(
        entry,
        "model",
        &model.model_id,
        &prefix,
        false,
        &mut changes,
    )?;
    set_text(entry, "base_url", &base, &prefix, false, &mut changes)?;
    set_text(entry, "name", &display_name, &prefix, false, &mut changes)?;
    set_text(entry, "api_backend", backend, &prefix, false, &mut changes)?;
    set_text(
        entry,
        "api_key",
        SECRET_PLACEHOLDER,
        &prefix,
        true,
        &mut changes,
    )?;
    // Only verified model metadata is injected. Existing request parameters and
    // reasoning_efforts remain user-owned until a native effort-list mapping is
    // implemented; their presence does not establish capability support.
    let context_window = if let Some(tokens) = model.context_window {
        set_number(entry, "context_window", tokens, &prefix, &mut changes)?;
        GrokContextWindow {
            source: GrokContextSource::Library,
            state: GrokContextState::Written,
            value: Some(tokens.to_string()),
        }
    } else {
        let existing = entry.get("context_window");
        GrokContextWindow {
            source: GrokContextSource::NotStated,
            state: if existing.is_some() {
                GrokContextState::Unchanged
            } else {
                GrokContextState::NotStated
            },
            // Do not expose arbitrary strings/tables from user configuration:
            // even a capacity field can contain misplaced credential text.
            value: existing
                .and_then(Item::as_integer)
                .map(|count| count.to_string()),
        }
    };

    let image_input = map_image_input(entry, model, &prefix, &mut changes)?;

    // `[models].default` is intentionally untouched: it is the user's choice,
    // and Grok's own model picker writes it. The written alias becomes
    // selectable in Grok without silently changing which model new sessions use.

    notices.insert(0, PlanNotice::code(NoticeCode::CredentialFromStore));
    notices.push(PlanNotice::code(NoticeCode::OverridesNotChecked));

    let TomlTarget {
        target: file,
        document,
    } = target;
    Ok((
        GrokPlan {
            agent: AgentId::GrokBuild,
            model_alias: alias,
            existing_alias,
            context_window,
            image_input,
            file: GrokPlannedFile {
                target: file,
                changes,
            },
            precedence: precedence_files(root)?,
            notices,
            source: PlanSource {
                provider_id: provider.id.as_str().to_owned(),
                provider_revision: provider.revision,
                model_id: model.model_id.clone(),
                model_content_digest: snapshot.content_digest.clone(),
            },
            restart_required: true,
        },
        // The merged document keeps the user's comments, key order and unrelated
        // tables, so the apply step serializes exactly what this preview showed
        // with only the credential position filled in.
        document,
    ))
}

/// Locate the higher-precedence files without reading any of them.
fn precedence_files(root: &Path) -> Result<Vec<GrokPrecedenceFile>, PlanError> {
    PRECEDENCE_FILES
        .iter()
        .map(|name| {
            // `symlink_metadata` keeps a link visible as a link: the file it
            // points at is somebody else's data and must not be read here.
            let state = match std::fs::symlink_metadata(root.join(name)) {
                Ok(metadata) if metadata.is_file() => GrokLayerState::Present,
                Ok(_) => GrokLayerState::Unavailable,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    GrokLayerState::Absent
                }
                Err(_) => GrokLayerState::Unavailable,
            };
            Ok(GrokPrecedenceFile { name, state })
        })
        .collect()
}

/// The alias this plan owns: the recorded one, or a free readable name.
///
/// Entries without a binding row are the user's, so a candidate that already
/// appears in `config.toml` is skipped rather than adopted. Every candidate is
/// deterministic, so two previews of the same selection agree on the alias.
fn choose_alias(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    model: &ProviderModel,
    document: &DocumentMut,
) -> Result<String, PlanError> {
    let root = root.to_str().ok_or(PlanError::TargetPathInvalid)?;
    let bindings = model_bindings(storage, root)?;
    let provider_id = provider.id.as_str();
    let owned: Vec<String> = bindings
        .iter()
        .filter(|(_, owner, model_id)| owner == provider_id && model_id == &model.model_id)
        .map(|(alias, _, _)| alias.clone())
        .collect();
    if owned.len() > 1 {
        // Two owned aliases for one saved model make "the entry vibemate wrote"
        // ambiguous, so the plan stops instead of choosing one silently.
        return Err(PlanError::TargetConflict);
    }
    if let Some(alias) = owned.first() {
        return Ok(alias.clone());
    }
    let id8 = &provider_id[..8];
    let model8 = &digest_hex(model.model_id.as_bytes())[..8];
    let provider_label = slug(&provider.display_name, 40)
        .or_else(|| slug(provider.kind.as_str(), 40))
        .unwrap_or_else(|| String::from("provider"));
    let model_label = slug(&model.model_id, 60).unwrap_or_else(|| String::from("model"));
    let base = format!("vibemate-{provider_label}-{model_label}");
    for candidate in [
        base.clone(),
        format!("{base}-{id8}"),
        format!("vibemate-{id8}-{model8}"),
    ] {
        if candidate.len() > 160 {
            return Err(PlanError::InvalidRequest);
        }
        let bound = bindings.iter().any(|(alias, _, _)| alias == &candidate);
        if bound || holds_alias(document, &candidate) {
            continue;
        }
        return Ok(candidate);
    }
    Err(PlanError::TargetConflict)
}

/// Every alias vibemate recorded for this configuration root.
fn model_bindings(
    storage: &Storage,
    root: &str,
) -> Result<Vec<(String, String, String)>, PlanError> {
    let connection = storage.lock().map_err(|_| PlanError::StorageUnavailable)?;
    let mut statement = connection
        .prepare("SELECT model_alias, provider_id, model_id FROM grok_model_binding WHERE config_root=?1")
        .map_err(|_| PlanError::StorageUnavailable)?;
    let rows = statement
        .query_map([root], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| PlanError::StorageUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PlanError::StorageUnavailable)?;
    Ok(rows)
}

/// Lowercase bare-key text Grok can write without quoting, or `None` when the
/// input has no usable characters.
fn slug(text: &str, limit: usize) -> Option<String> {
    let mut out = String::new();
    for character in text.chars() {
        if out.len() >= limit {
            break;
        }
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_owned();
    (!out.is_empty()).then_some(out)
}

/// Whether `[model.<alias>]` exists in the document.
fn holds_alias(document: &DocumentMut, alias: &str) -> bool {
    document
        .get("model")
        .and_then(Item::as_table_like)
        .is_some_and(|table| table.contains_key(alias))
}

/// Whether the alias entry already carries an `env_key` credential reference.
///
/// Grok resolves `api_key` ahead of `env_key`, so writing the planned key takes
/// over that entry's authentication instead of adding to it. The preview keeps
/// the user's `env_key` line untouched and reports the change of source.
fn has_env_key(document: &DocumentMut, alias: &str) -> bool {
    document
        .get("model")
        .and_then(Item::as_table)
        .and_then(|models| models.get(alias))
        .and_then(Item::as_table_like)
        .is_some_and(|entry| entry.get("env_key").is_some())
}

/// The top-level table a plan writes into, created when the document has none.
fn root_table<'a>(document: &'a mut DocumentMut, key: &str) -> Result<&'a mut Table, PlanError> {
    document
        .entry(key)
        .or_insert_with(table)
        .as_table_mut()
        .ok_or(PlanError::TargetConflict)
}

/// The `[model.<alias>]` table, created when this alias is new.
fn named_table<'a>(parent: &'a mut Table, key: &str) -> Result<&'a mut Table, PlanError> {
    parent
        .entry(key)
        .or_insert_with(table)
        .as_table_mut()
        .ok_or(PlanError::TargetConflict)
}

/// Read one top-level table's string array, when it is spelled that way.
///
/// `Ok(None)` means the key is absent. Anything that is not an array of strings
/// is a conflict: the adapter cannot tell whether it names the planned alias,
/// and guessing could write a model Grok will never offer.
fn string_list(
    document: &DocumentMut,
    table_name: &str,
    key: &str,
) -> Result<Option<Vec<String>>, PlanError> {
    let Some(item) = document
        .get(table_name)
        .and_then(Item::as_table)
        .and_then(|table| table.get(key))
    else {
        return Ok(None);
    };
    let values = item
        .as_array()
        .filter(|array| array.iter().all(|value| value.as_str().is_some()))
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .ok_or(PlanError::TargetConflict)?;
    Ok(Some(values))
}

/// Whether a string array names the planned alias.
fn list_contains(
    document: &DocumentMut,
    table_name: &str,
    key: &str,
    wanted: &str,
) -> Result<bool, PlanError> {
    Ok(string_list(document, table_name, key)?
        .is_some_and(|values| values.iter().any(|value| value == wanted)))
}

/// Write a string field and describe the change, masking a credential.
///
/// A key that already holds another shape — a table, an array, a number — is a
/// conflict rather than a silent overwrite, so an unrelated value can never be
/// replaced by a piece of text.
fn set_text(
    table: &mut Table,
    key: &str,
    text: &str,
    prefix: &str,
    secret: bool,
    changes: &mut Vec<ConfigChange>,
) -> Result<(), PlanError> {
    let before = table.get(key);
    if before.is_some_and(|item| item.as_str().is_none()) {
        return Err(PlanError::TargetConflict);
    }
    changes.push(ConfigChange {
        key_path: format!("{prefix}.{key}"),
        before: before.map(|item| {
            if secret {
                SECRET_PLACEHOLDER.to_owned()
            } else {
                item.as_str().unwrap_or_default().to_owned()
            }
        }),
        after: if secret {
            SECRET_PLACEHOLDER.to_owned()
        } else {
            text.to_owned()
        },
        secret,
    });
    table.insert(key, value(text));
    Ok(())
}

/// Write an integer field, refusing a key that holds another shape.
fn set_number(
    table: &mut Table,
    key: &str,
    number: i64,
    prefix: &str,
    changes: &mut Vec<ConfigChange>,
) -> Result<(), PlanError> {
    let before = table.get(key);
    if before.is_some_and(|item| item.as_integer().is_none()) {
        return Err(PlanError::TargetConflict);
    }
    changes.push(ConfigChange {
        key_path: format!("{prefix}.{key}"),
        before: before.map(|item| {
            item.as_integer()
                .map(|value| value.to_string())
                .unwrap_or_default()
        }),
        after: number.to_string(),
        secret: false,
    });
    table.insert(key, value(number));
    Ok(())
}

/// Map a declared central image capability to Grok's per-model `vision` flag.
/// Unknown central metadata preserves the native value; only booleans are shown.
fn map_image_input(
    entry: &mut Table,
    model: &ProviderModel,
    prefix: &str,
    changes: &mut Vec<ConfigChange>,
) -> Result<GrokImageInput, PlanError> {
    let declaration = model.capability_overrides.image_input;
    let state = declaration
        .map(|value| value.as_state())
        .unwrap_or(model.capabilities.image_input.state);
    let source = if declaration.is_some() {
        GrokCapabilitySource::UserDeclaration
    } else if state != CapabilityState::Unknown {
        GrokCapabilitySource::Catalog
    } else {
        GrokCapabilitySource::NotStated
    };
    match state {
        CapabilityState::Supported | CapabilityState::Unsupported => {
            let supported = state == CapabilityState::Supported;
            let before = entry.get("vision");
            let previous = before.and_then(Item::as_bool);
            if before.is_some() && previous.is_none() {
                return Err(PlanError::TargetConflict);
            }
            changes.push(ConfigChange {
                key_path: format!("{prefix}.vision"),
                before: previous.map(|value| value.to_string()),
                after: supported.to_string(),
                secret: false,
            });
            entry.insert("vision", value(supported));
            Ok(GrokImageInput {
                source,
                state: GrokCapabilityState::Written,
                value: Some(supported),
            })
        }
        CapabilityState::Unknown => {
            let existing = entry.get("vision").and_then(Item::as_bool);
            Ok(GrokImageInput {
                source,
                state: if entry.contains_key("vision") {
                    GrokCapabilityState::Unchanged
                } else {
                    GrokCapabilityState::NotStated
                },
                value: existing,
            })
        }
    }
}

/// Apply the reviewed plan through the shared private writer.
///
/// The whole preview is rebuilt and every token compared before the OS key is
/// read, so an edit made after the user looked at the card stops the write with
/// `plan_stale` instead of overwriting it. Only the masked `api_key` position
/// receives the real value; everything else in the document is what the card
/// showed, including the user's own comments and unrelated tables.
///
/// # Errors
/// `PlanStale` when any token moved, `CredentialMissing`/`CredentialUnavailable`
/// when the stored key cannot be read, the plan's own refusals re-mapped, and
/// `OperationFailed` when the operation could not be recorded. A file-level
/// refusal is a result, not an error: it travels in the step's `error_code`.
pub fn apply_grok_settings(
    storage: &Storage,
    credentials: &impl CredentialStore,
    root: &Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
    reviewed: &ReviewedGrokPlan,
    backups: PathBuf,
) -> Result<ConfigApplied, ConfigApplyError> {
    if reviewed.provider_revision != provider.revision
        || reviewed.model_content_digest != snapshot.content_digest
    {
        return Err(ConfigApplyError::PlanStale);
    }
    let (plan, mut document) =
        build(storage, root, provider, snapshot).map_err(ConfigApplyError::from_plan)?;
    if reviewed.model_alias != plan.model_alias
        || reviewed.file.path != plan.file.target.path
        || reviewed.file.content_digest != plan.file.target.content_digest
        || reviewed.provider_revision != plan.source.provider_revision
        || reviewed.model_content_digest != plan.source.model_content_digest
    {
        return Err(ConfigApplyError::PlanStale);
    }
    let reference = provider.id.credential_reference();
    let secret = credentials
        .load(&reference)
        .map_err(|_| ConfigApplyError::CredentialUnavailable)?
        .ok_or(ConfigApplyError::CredentialMissing)?;
    // Grok reads the key out of this file, so the value lands as plain text and
    // the copy of the previous content in the private backup can already hold one.
    // `toml_edit` renders the string as a quoted TOML value, escaping quotes and
    // control characters, so a stored key stays one literal value.
    alias_table(&mut document, &plan.model_alias)
        .ok_or(ConfigApplyError::OperationFailed)?
        .insert("api_key", value(secret.expose()));
    let files = [PlannedFile {
        target: WriteTarget::from_plan(&plan.file.target),
        content: document.to_string().into_bytes(),
    }];
    let result = apply_private_files(
        storage,
        OperationKind::GrokSettings,
        Some(provider.id.as_str()),
        &files,
        &BackupStore::new(backups),
    )
    .map_err(ConfigApplyError::from_write)?;
    // Ownership is recorded only for a write that reached the disk: a refused or
    // rolled-back apply leaves no claim behind, so nothing is treated as
    // vibemate's own entry that the user never agreed to have written.
    if result
        .steps
        .first()
        .is_some_and(|step| step.state == StepState::Applied)
    {
        record_alias(
            storage,
            root,
            provider,
            &snapshot.model.model_id,
            &plan.model_alias,
        )?;
    }
    Ok(ConfigApplied {
        operation_id: result.operation_id,
        state: result.state,
        backup_path: result
            .steps
            .iter()
            .find_map(|step| step.backup_path.clone()),
        steps: result.steps,
        restart_required: result.state == OperationState::Applied,
    })
}

/// Reach the `[model.<alias>]` table the plan just described.
fn alias_table<'a>(document: &'a mut DocumentMut, alias: &str) -> Option<&'a mut Table> {
    document
        .get_mut("model")
        .and_then(Item::as_table_mut)
        .and_then(|models| models.get_mut(alias))
        .and_then(Item::as_table_mut)
}

/// Remember that this alias holds the selected Provider/model pair.
///
/// Only identities travel here: no key, no file content and no document text.
/// The same alias in another configuration root is a separate row, because
/// `GROK_HOME` can point a second installation at a file with the same names.
fn record_alias(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    model_id: &str,
    alias: &str,
) -> Result<(), ConfigApplyError> {
    let root = root.to_str().ok_or(ConfigApplyError::InvalidRequest)?;
    let written_at =
        crate::shared::current_unix_millis().map_err(|_| ConfigApplyError::OperationFailed)?;
    let changed = storage
        .lock()
        .map_err(|_| ConfigApplyError::StorageUnavailable)?
        .execute(
            "INSERT INTO grok_model_binding
                 (config_root,model_alias,provider_id,model_id,last_written_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(config_root,model_alias) DO UPDATE SET
                 provider_id=excluded.provider_id,
                 model_id=excluded.model_id,
                 last_written_at=excluded.last_written_at
             WHERE grok_model_binding.provider_id=excluded.provider_id",
            params![root, alias, provider.id.as_str(), model_id, written_at],
        )
        .map_err(|_| ConfigApplyError::OperationFailed)?;
    // Zero changed rows means the alias already belongs to another Provider, which
    // this write must not reassign; the failure is reported instead of quietly
    // moving the claim.
    if changed != 1 {
        return Err(ConfigApplyError::OperationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
