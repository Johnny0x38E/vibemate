# Architecture

## Implemented foundation

- **Frontend.** `src/main.tsx` mounts React behind the language startup gate.
  `src/App.tsx` composes the desktop shell: Overview, the Providers page
  (`src/features/providers/`), the MCP page (`src/features/mcp/`), the read-only Agents page (`src/features/agents/`), a planned Skills page, and Settings
  (`src/features/settings/`). UI code calls Rust only through the typed wrappers
  in `src/lib/desktop.ts` and `src/lib/desktop/`, which validate every response.
- **Commands.** `src-tauri/src/commands.rs` declares feature-specific command
  modules under `commands/` (app, preferences, providers, models, mcp and agents).
  `commands/state.rs` holds managed-state lookups and the existing credential lock.
  These thin Tauri commands handle app metadata, the repository link, language and appearance preferences, provider
  instances, provider API keys, and provider model fetch/list/selection (user-triggered
  only). The Models tab uses `browse_upstream_models_page` (in-memory browse
  catalog, optional fuzzy `query`, no SQLite merge) and
  `save_provider_model_selections` (batch persist/removal); `fetch_provider_models`
  remains for full-catalog merge and tests. `src-tauri/src/lib.rs` opens storage
  and registers them.
- **Persistence.** `src-tauri/src/storage.rs` owns the private SQLite database in
  the app-data folder and its forward-only migrations (currently schema v9).
  `settings.rs` and `appearance.rs` store preferences; `providers.rs` preserves
  the public domain entrypoints while `providers/` separates types, templates,
  validation, repository SQL and credential save orchestration. It stores provider instances (stable random IDs, cursor pages, optimistic
  `revision` checks). Key status and replacement use the same provider types,
  SQL, and credential orchestration. `models.rs` retains the public saved-model API.
  `model_error.rs` holds the shared model IPC errors. `models/types.rs` holds records;
  `validation.rs` holds bounded cursors; `repository.rs` owns
  local lists, fuzzy search and row conversion; `selection.rs` owns manual edits
  and atomic selection/save transactions.
  SQLite stores only a `provider-<id>` reference per key, never the key itself.
- **Credentials.** `src-tauri/src/credentials.rs` provides the OS credential-store
  interface, key validation and the compensation helpers that keep the store and
  SQLite consistent when providers are created and keys are replaced.
- **Central MCP.** `mcp.rs` exposes the original domain entrypoints; `mcp/` separates
  request/record types, validation, repository SQL and credential services.
  It validates stdio/Streamable HTTP definitions and stores
  non-secret metadata, stable IDs and revisions. Every env/header value uses
  an immutable OS credential reference. Failed known database writes compensate
  new credentials; uncertain commits retain them and require reconciliation.
  Transactional cleanup records make obsolete credential deletion retryable.
  Reads never load values. Saves and enablement changes perform no server process
  launch, HTTP request or Agent config write. Crash-orphan recovery remains P34.
  See [central definition boundaries](plans/mcp-central.md).
- **Agent discovery.** `agents.rs` owns typed observations and environment/input
  contracts; `agents/discovery.rs` performs bounded filesystem discovery independent
  of Tauri. `commands/agents.rs` runs it on the blocking pool with the platform home
  and desktop environment. No SQLite or credential state is needed. The only
  content read is bounded Pi package metadata whose package name and bin target
  match the resolved entry; Agent configs and executables remain unopened for
  content. Grok filename version hints remain unverified. Presence, version
  provenance, documented capabilities and user configuration roots are separate
  observations; project/CLI/policy/session precedence is documented without
  claiming effective settings. Manual paths are window-local. No process launch,
  config write, network request or new capability permission is involved. See
  [read-only discovery](plans/agent-discovery.md) for sources and verified limits.
- **Shared helpers.** `shared.rs` holds domain-neutral text checks, random-ID format
  checks and Unix milliseconds. Provider and MCP keep their own identity types and
  map failures into their existing errors. MCP has no Provider dependency.
  The original public Provider validation and clock entrypoints remain available.
- **HTTP and models.** `model_http.rs` performs the model-list HTTPS GET: Bearer
  authentication, bounded bodies, cancellation, and system-proxy support.
  `model_identity.rs` holds model-ID grammar and protocol route checks.
  `model_catalog.rs` parses phase-1 vendor responses;
  `model_search.rs` ranks fuzzy matches. `model_fetch.rs` preserves the public
  discovery API while `model_fetch/` separates request/result types, registration
  and cancellation (`registry.rs`), revision/URL-aware sessions (`cache.rs`),
  HTTP downloads/deadlines (`download.rs`), browse-only paging/ranking (`browse.rs`),
  snapshot/status SQL (`repository.rs`), snapshot-checked catalog transactions
  (`merge.rs`) and the credential snapshot step (`service.rs`). The desktop command
  composes download and merge; tests repeat that order in a helper.
  Fetch storage no longer borrows the local model repository's lock. Credentials
  are loaded after the snapshot lock is released and dropped before merge; the
  registration survives the merge. Browsing never writes model rows. Cancellation,
  stale-fetch checks, manual/selected retention, pruning and API/result offsets
  retain their existing rules.

Saved provider instances and selected models are configuration only: nothing writes
agent configuration or claims a verified inference connection yet. Manual model
add is implemented in the UI. Isolated Linux IPC verified its add/restart/delete
using a seeded no-key provider fixture; real-provider UI and other-platform
acceptance remain separate.

Keep one Rust crate while learning the framework. Create feature modules as
behavior is implemented rather than adding empty abstractions now.

The maintainer has requested planning for GitHub/Vercel integrations, more model
providers and MCP capabilities, including GitHub configuration backups, source-based
MCP/Skill installation, potential Agent installation and API forwarding to Agents. The [backend modularity plan](plans/backend-modularity.md)
records current file sizes and dependency issues, incremental splits, and future
extension boundaries. BR1–BR5 in [todo.md](plans/todo.md) are implemented:
eight modules have separate behavior-test files, commands are grouped by feature,
and Provider/MCP responsibilities are split with shared text, identity and clock
helpers. Models/Fetch responsibilities are also separated. Catalog/infrastructure
refinement and future integrations remain planned.
The layout uses `feature.rs` plus a matching `feature/` directory. Keep cloud-service accounts separate
from model providers and central MCP definitions. Reuse credential storage and
extract HTTP transport only when a concrete second use requires it. Planned
`backup` owns snapshot/restore semantics; `sources` and `installation` own source
resolution and managed Agent/MCP/Skill installs, while `agents` retains discovery
and configuration management; `gateway` and `protocols` own local API service
lifecycle and verified forwarding. Agent traffic reaches the local API directly,
while Tauri manages its lifecycle. These modules remain unimplemented.

## Planned responsibilities

| Component           | Responsibility                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| Provider adapters   | Authentication, endpoints, model discovery, and protocol-specific fields        |
| Model configuration | Provider/model identity, capability metadata, and validated request settings    |
| Agent adapters      | Installed-version detection, config precedence, mapping, preview, and injection |
| Skill manager       | Central source tracking, revision updates, symlinks, and copy synchronization   |
| MCP manager         | Shared definitions, credential references, and agent-specific config mapping    |
| Persistence         | Versioned configuration schema and migrations; SQLite when persistence starts   |
| Credentials         | OS credential storage, accessed by Rust                                         |

React calls Rust commands; it should not contact providers or write config files
directly. Domain functions should be callable in tests without Tauri.
Provider and agent adapters have different responsibilities and should not be
combined into a provider-by-agent matrix of duplicated implementations.

Use schema-driven forms for common provider fields, plus typed adapter logic
where protocols differ. Preserve unknown agent config fields. Skills and MCP
have independent support per agent; an adapter must report those capabilities.

## Configuration flow

Read existing state → resolve precedence → validate supported options → preview
changes → recheck file state → back up → atomically write → verify → allow rollback.

A write may only take effect after an agent restarts. Report that clearly.
Environment variables and project-local overrides can supersede global config.

## Planned enablement relationships

The maintainer's home-screen sketch shows providers above a configuration
application point, agents below it, and shared skills/MCP definitions alongside.
Green nodes and links mean enabled configuration or deployment; selection has
its own visual treatment. The lines describe configuration relationships.
Visual details can be refined after the behavior is implemented.

Keep a shared resource's enabled state separate from each agent deployment's
state. Disabling one deployment preserves the central definition and other
agents' deployments. Disabling a shared resource prepares changes for its
managed deployments and reports each target result; it cannot silently remove
user-owned configuration or imply an atomic operation across all targets.

Store requested enablement separately from the last applied state. Track
configuration overrides, pending reload, unsupported mappings, and failures
explicitly. A green link must not represent an unapplied request or claim a
successful provider request. If the effective running-session state is unknown,
show that alongside the enabled configuration instead of inventing runtime proof.

Agent adapters translate a switch into the verified native operation. Pi 1.1.0
has a per-server MCP `enabled` field, but provider/model and skill deployment
need different operations. See [the Pi contract](integrations/pi.md). All
mutations use preview, concurrent-change checks, backups, and rollback.

## Later extensions

An optional localhost proxy may support routing, protocol conversion, and usage
collection. Direct configuration is preferable when the agent supports the
provider protocol. Streaming, tool calls, reasoning, and images need explicit
compatibility tests before a conversion is marked supported.

A future usage-source boundary can accept provider quota APIs, agent logs, and
proxy usage events. Retain source identity and timestamps, distinguish measured
usage from estimates, and avoid duplicate counting. Quota monitoring and activity
heatmaps are outside phase 1.
