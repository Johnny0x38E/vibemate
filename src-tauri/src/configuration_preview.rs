//! Read-only model-plan presentation shared by the three configuration adapters.
//!
//! Rebuild the native merge, compare the complete reviewed plan, then verify the
//! exact current bytes again before deriving summaries or on-demand full text.
//! Neither paths nor merged documents supplied by the frontend are trusted.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::agents::{AgentId, DiscoveryEnvironment, claude_code, grok_build, pi};
use crate::config_apply::{
    OperationKind, VersionFileSummary, VersionText, VersionTextSide, version_text,
};
use crate::config_plan::{PlanError, TargetFile, read_target_bytes};
use crate::models::ModelSnapshot;
use crate::providers::ProviderRecord;
use crate::storage::Storage;

/// Strict read request; reviewed plan fields are compared, never used as paths.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PreviewRequest {
    /// Adapter whose native configuration is being reviewed.
    pub(crate) agent: AgentId,
    /// Saved Provider instance, validated before storage access.
    pub(crate) provider_id: String,
    /// Saved upstream model identity, not a native alias.
    pub(crate) model_id: String,
    /// Discovery entry for Pi/Grok contract verification; never executed.
    pub(crate) entry_path: Option<String>,
    /// Complete sanitized plan returned by the existing preview command.
    pub(crate) reviewed: Value,
    /// Absent requests summaries only; a file index requests just that file's text.
    pub(crate) file_index: Option<usize>,
}

impl PreviewRequest {
    /// Validate fixed scope and payload bounds before opening storage or files.
    /// Returns `InvalidRequest`; never reads credentials or changes state.
    pub(crate) fn validate(&self) -> Result<(), PreviewError> {
        if !matches!(
            self.agent,
            AgentId::ClaudeCode | AgentId::Pi | AgentId::GrokBuild
        ) || crate::models::parse_provider_id(&self.provider_id).is_err()
            || !crate::model_identity::is_valid_model_id(&self.model_id)
            || self.file_index.is_some_and(|index| index > 1)
            || !self.reviewed.is_object()
            || serde_json::to_vec(&self.reviewed)
                .map_err(|_| PreviewError::InvalidRequest)?
                .len()
                > 1024 * 1024
        {
            return Err(PreviewError::InvalidRequest);
        }
        Ok(())
    }
}

/// Safe presentation failure; no document or parser diagnostics cross IPC.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreviewError {
    /// Unsupported Agent, malformed identity or out-of-range file index.
    InvalidRequest,
    /// The current native merge no longer matches the reviewed plan.
    PlanStale,
    /// Storage, serialization or the blocking task could not finish.
    OperationFailed,
}

/// One file's sanitized summary and optional on-demand text.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewFile {
    /// Absolute path selected by Rust, in the adapter's application order.
    pub(crate) target_path: String,
    /// Same field-level format and sanitization as a version restore.
    pub(crate) summary: VersionFileSummary,
    /// Present only for the one explicitly requested file.
    pub(crate) text: Option<VersionText>,
}

/// Build presentation from the exact native merge used by application.
///
/// Reads bounded native configuration files and stored metadata, never OS keys.
/// A changed plan/file is refused; no filesystem or database state is changed.
///
/// # Errors
/// Returns `PlanStale` when a native dependency or reviewed token changed,
/// `InvalidRequest` for an unsupported Agent or index, and `OperationFailed`
/// when the presentation cannot be serialized.
pub(crate) fn read_preview(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    provider: &ProviderRecord,
    model: &ModelSnapshot,
    input: &PreviewRequest,
) -> Result<Vec<PreviewFile>, PreviewError> {
    let stale = |_: PlanError| PreviewError::PlanStale;
    match input.agent {
        AgentId::ClaudeCode => {
            let (root, _) = crate::agents::claude_config_root(environment);
            let (plan, document) = claude_code::build(&claude_code::ClaudeCodePlanInput {
                config_root: root.ok_or(PreviewError::PlanStale)?,
                provider: claude_code::ProviderFacts {
                    id: provider.id.as_str().to_owned(),
                    kind: provider.kind,
                    base_url: provider.base_url.clone(),
                    revision: provider.revision,
                },
                model: claude_code::ModelFacts::from_snapshot(model),
                selected_effort: claude_code::reviewed_effort(&input.reviewed).map_err(stale)?,
            })
            .map_err(stale)?;
            check_plan(&plan, input, 1)?;
            Ok(vec![present(
                OperationKind::ClaudeCodeSettings,
                &plan.target,
                &serde_json::to_vec_pretty(&document).map_err(|_| PreviewError::OperationFailed)?,
                0,
                input.file_index,
            )?])
        }
        AgentId::Pi => {
            let root =
                pi::configuration_root(environment, input.entry_path.clone()).map_err(stale)?;
            let (plan, documents) = pi::build(storage, &root, provider, model).map_err(stale)?;
            check_plan(&plan, input, plan.files.len())?;
            plan.files
                .iter()
                .zip(documents)
                .enumerate()
                .map(|(index, (file, document))| {
                    present(
                        OperationKind::PiSettings,
                        &file.target,
                        &serde_json::to_vec_pretty(&document)
                            .map_err(|_| PreviewError::OperationFailed)?,
                        index,
                        input.file_index,
                    )
                })
                .collect()
        }
        AgentId::GrokBuild => {
            let root = grok_build::configuration_root(environment, input.entry_path.clone())
                .map_err(stale)?;
            let (plan, document) =
                grok_build::build(storage, &root, provider, model).map_err(stale)?;
            check_plan(&plan, input, 1)?;
            Ok(vec![present(
                OperationKind::GrokSettings,
                &plan.file.target,
                document.to_string().as_bytes(),
                0,
                input.file_index,
            )?])
        }
        _ => Err(PreviewError::InvalidRequest),
    }
}

fn check_plan(
    plan: &impl Serialize,
    input: &PreviewRequest,
    count: usize,
) -> Result<(), PreviewError> {
    if input.file_index.is_some_and(|index| index >= count) {
        return Err(PreviewError::InvalidRequest);
    }
    if serde_json::to_value(plan).map_err(|_| PreviewError::OperationFailed)? != input.reviewed {
        return Err(PreviewError::PlanStale);
    }
    Ok(())
}

fn present(
    kind: OperationKind,
    target: &TargetFile,
    planned: &[u8],
    index: usize,
    requested: Option<usize>,
) -> Result<PreviewFile, PreviewError> {
    let (current, bytes) =
        read_target_bytes(Path::new(&target.path)).map_err(|_| PreviewError::PlanStale)?;
    if &current != target {
        return Err(PreviewError::PlanStale);
    }
    let before = current.exists.then_some(bytes.as_slice());
    let summary = version_text::summarize(kind, before, Some(planned));
    let text = if requested == Some(index) {
        let format = version_text::format_for(kind);
        let render = |bytes: &[u8]| {
            version_text::render_text(format, bytes).unwrap_or_else(|reason| {
                VersionTextSide::Unavailable {
                    reason: reason.to_owned(),
                }
            })
        };
        Some(VersionText {
            revision_id: format!("file-{index}"),
            target_path: current.path.clone(),
            format,
            current: before.map_or(VersionTextSide::Absent, render),
            target: render(planned),
        })
    } else {
        None
    };
    Ok(PreviewFile {
        target_path: current.path,
        summary,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_plan::digest_hex;

    #[test]
    fn summaries_and_text_mask_unowned_credentials_and_refuse_changed_files() {
        let root =
            std::env::temp_dir().join(format!("vibemate-model-preview-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let bytes = br#"{"unowned":{"password":"synthetic-private"},"model":"old"}"#;
        std::fs::write(&path, bytes).unwrap();
        let target = TargetFile {
            path: path.to_str().unwrap().into(),
            exists: true,
            symlink: false,
            content_digest: Some(digest_hex(bytes)),
        };
        let after =
            br#"{"unowned":{"password":"synthetic-private"},"model":"new","apiKey":"placeholder"}"#;
        let summary = present(OperationKind::ClaudeCodeSettings, &target, after, 0, None).unwrap();
        assert!(summary.text.is_none());
        let detail = present(
            OperationKind::ClaudeCodeSettings,
            &target,
            after,
            0,
            Some(0),
        )
        .unwrap();
        let encoded = serde_json::to_string(&detail).unwrap();
        assert!(!encoded.contains("synthetic-private"));
        assert!(!encoded.contains("placeholder"));
        assert!(
            detail
                .summary
                .changes
                .iter()
                .any(|change| change.path == "model")
        );
        std::fs::write(&path, b"{}").unwrap();
        assert!(matches!(
            present(
                OperationKind::ClaudeCodeSettings,
                &target,
                after,
                0,
                Some(0)
            ),
            Err(PreviewError::PlanStale)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn strict_request_rejects_unknown_fields_unsupported_agents_and_unbounded_payloads() {
        let value = serde_json::json!({"agent":"pi", "providerId":"a".repeat(32), "modelId":"model", "entryPath":null, "reviewed":{}, "fileIndex":null});
        let mut request: PreviewRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.validate(), Ok(()));
        let mut unknown = value;
        unknown["path"] = serde_json::json!("/untrusted");
        assert!(serde_json::from_value::<PreviewRequest>(unknown).is_err());
        request.agent = AgentId::Codex;
        assert_eq!(request.validate(), Err(PreviewError::InvalidRequest));
        request.agent = AgentId::Pi;
        request.file_index = Some(2);
        assert_eq!(request.validate(), Err(PreviewError::InvalidRequest));
        request.file_index = None;
        request.reviewed = serde_json::json!({"oversized":"x".repeat(1024 * 1024)});
        assert_eq!(request.validate(), Err(PreviewError::InvalidRequest));
    }

    #[test]
    fn reviewed_plan_and_selected_file_must_match_exactly() {
        let mut request = PreviewRequest {
            agent: AgentId::Pi,
            provider_id: String::new(),
            model_id: String::new(),
            entry_path: None,
            reviewed: serde_json::json!({"token":1}),
            file_index: None,
        };
        assert_eq!(
            check_plan(&serde_json::json!({"token":1}), &request, 2),
            Ok(())
        );
        assert_eq!(
            check_plan(&serde_json::json!({"token":2}), &request, 2),
            Err(PreviewError::PlanStale)
        );
        request.file_index = Some(2);
        assert_eq!(
            check_plan(&request.reviewed, &request, 2),
            Err(PreviewError::InvalidRequest)
        );
    }
}
