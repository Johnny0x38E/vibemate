# Architecture

## Implemented foundation

- **Frontend.** `src/main.tsx` mounts React behind the language startup gate.
  `src/App.tsx` composes the desktop shell: Overview, the Providers page
  (`src/features/providers/`), the MCP page (`src/features/mcp/`), planned pages for Agents/Skills, and Settings
  (`src/features/settings/`). UI code calls Rust only through the typed wrappers
  in `src/lib/desktop.ts` and `src/lib/desktop/`, which validate every response.
- **Commands.** `src-tauri/src/commands.rs` holds thin Tauri commands for app
  metadata, the repository link, language and appearance preferences, provider
  instances, provider API keys, and provider model fetch/list/selection (user-triggered
  only). The Models tab uses `browse_upstream_models_page` (in-memory browse
  catalog, optional fuzzy `query`, no SQLite merge) and
  `save_provider_model_selections` (batch persist/removal); `fetch_provider_models`
  remains for full-catalog merge and tests. `src-tauri/src/lib.rs` opens storage
  and registers them.
- **Persistence.** `src-tauri/src/storage.rs` owns the private SQLite database in
  the app-data folder and its forward-only migrations (currently schema v8).
  `settings.rs` and `appearance.rs` store preferences; `providers.rs` validates
  and stores provider instances (stable random IDs, cursor pages, optimistic
  `revision` checks); `provider_secrets.rs` reads key status and replaces keys;
  `models.rs` stores fetched and manual model rows and selections per provider.
  SQLite stores only a `provider-<id>` reference per key, never the key itself.
- **Credentials.** `src-tauri/src/credentials.rs` provides the OS credential-store
  interface, key validation and the compensation helpers that keep the store and
  SQLite consistent when providers are created and keys are replaced.
- **Central MCP.** `mcp.rs` validates stdio/Streamable HTTP definitions and stores
  non-secret metadata, stable IDs and revisions. Every env/header value uses
  an immutable OS credential reference. Failed known database writes compensate
  new credentials; uncertain commits retain them and require reconciliation.
  Transactional cleanup records make obsolete credential deletion retryable.
  Reads never load values. Saves and enablement changes perform no server process
  launch, HTTP request or Agent config write. Crash-orphan recovery remains P34.
  See [central definition boundaries](plans/mcp-central.md).
- **HTTP and models.** `http_client.rs` performs bounded HTTPS GETs with cancellation
  and system-proxy support; `model_catalog.rs` and `model_fetch.rs` map the three
  phase-1 providers' list endpoints into stored catalog rows; `model_search.rs`
  scores in-memory fuzzy queries over saved rows.

Saved provider instances and selected models are configuration only: nothing writes
agent configuration or claims a verified inference connection yet. Manual model
add in the UI is the remaining P12 frontend item (P12.c.3); Rust
`add_manual_provider_model` / `delete_manual_provider_model` are ready.

Keep one Rust crate while learning the framework. Create feature modules as
behavior is implemented rather than adding empty abstractions now.

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
