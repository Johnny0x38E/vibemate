//! Read-only Agent entry discovery. No process execution, config parsing, writes,
//! credential access or network requests belong in this feature.

mod discovery;
#[cfg(test)]
mod tests;

pub use discovery::discover_agents;
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

/// Agent identities with independently verified configuration contracts.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentId {
    /// Earendil Pi, not other tools named pi.
    Pi,
    /// xAI Grok Build.
    GrokBuild,
}

/// Optional entry paths supplied explicitly by the user; never executed.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryInput {
    /// Absolute Pi entry path, or automatic lookup when absent.
    pub pi_path: Option<String>,
    /// Absolute Grok entry path, or automatic lookup when absent.
    pub grok_path: Option<String>,
}

/// Process environment snapshot, injectable for isolated filesystem tests.
pub struct DiscoveryEnvironment {
    /// Platform home resolved by Tauri, not a project directory.
    pub home: PathBuf,
    /// Desktop process PATH; relative entries are ignored.
    pub path: Option<OsString>,
    /// Explicit Pi configuration directory from the desktop environment.
    pub pi_directory: Option<OsString>,
    /// Explicit Grok configuration root from the desktop environment.
    pub grok_directory: Option<OsString>,
}

/// Safe discovery failures; I/O diagnostics and user content are discarded.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryError {
    /// A manual path is relative, too long or contains control characters.
    InvalidPath,
    /// The OS home cannot be located or represented safely.
    LocationUnavailable,
    /// The desktop blocking task could not complete.
    OperationFailed,
}

/// Filesystem observation, never a running/connected/install-authenticity state.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntryState {
    /// The selected entry resolves to a regular file.
    Found,
    /// No entry was found in the bounded search.
    NotFound,
    /// An entry exists but cannot be resolved or is not a regular file.
    Unavailable,
}

/// Evidence source for an entry; PATH lookup retains shadowing order.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntrySource {
    /// Explicit current-window input.
    Manual,
    /// First matching entry in the desktop process PATH.
    Path,
    /// Documented user installation entry.
    UserEntry,
    /// No entry located.
    None,
}

/// A version claim's provenance; neither source proves binary authenticity.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VersionEvidence {
    /// Matching Pi package name and bin target in bounded package metadata.
    PackageMetadata,
    /// Grok versioned download filename only; always unverified.
    Filename,
    /// No safe version claim was available.
    None,
}

/// Applicability of the recorded configuration contract, not deployment status.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityEvidence {
    /// Pi metadata matches the exact 1.1.0 contract documented in P02.
    Documented,
    /// Identity/version is insufficient to apply a version-specific contract.
    Unverified,
}

/// Configuration root provenance; SDK/CLI overrides are not observable here.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DirectorySource {
    /// Default path below the platform home.
    Default,
    /// Absolute desktop-process environment override.
    Environment,
    /// Override was invalid; no fallback is presented as effective.
    Unavailable,
}

/// Presence/type only; no Agent configuration content is read.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileState {
    /// Existing regular file (symlinks may resolve to a regular file).
    Present,
    /// Path is absent.
    Missing,
    /// I/O failed or path points to another type.
    Unavailable,
}

/// A fixed known configuration filename and its filesystem observation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationFile {
    /// Fixed filename from the adapter, never content extracted from a file.
    pub name: &'static str,
    /// Existence and type, without reading contents.
    pub state: FileState,
}

/// One Agent observation. Paths and version claims are bounded display data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentObservation {
    /// Stable Agent identity.
    pub id: AgentId,
    /// Entry observation, independent from configuration presence.
    pub state: EntryState,
    /// How the entry was chosen.
    pub source: EntrySource,
    /// Original absolute entry path, when a candidate exists.
    pub entry_path: Option<String>,
    /// Resolved regular-file target, only when found.
    pub resolved_path: Option<String>,
    /// Safe semantic version claim; not obtained by running the Agent.
    pub version: Option<String>,
    /// Version provenance.
    pub version_evidence: VersionEvidence,
    /// Whether the recorded version-specific contract applies.
    pub capability_evidence: CapabilityEvidence,
    /// Chosen user config root; not necessarily effective at runtime.
    pub config_directory: Option<String>,
    /// Default/environment provenance for that root.
    pub directory_source: DirectorySource,
    /// Fixed config paths checked for presence without reading content.
    pub files: Vec<ConfigurationFile>,
}
