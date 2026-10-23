# Architecture

## Delivered foundation

### Frontend

`src/main.tsx` mounts React behind the language startup gate. `src/App.tsx`
composes the desktop shell: the Overview, Providers (`src/features/providers/`),
MCP (`src/features/mcp/`), Agents (`src/features/agents/`), Skills
(`src/features/skills/`) and Settings (`src/features/settings/`) modules. UI code
calls Rust only through the typed wrappers in `src/lib/desktop.ts` and
`src/lib/desktop/`, which validate every response at runtime. See
[frontend.md](frontend.md).

### Commands

`src-tauri/src/commands.rs` declares feature command modules (`app`,
`preferences`, `providers`, `models`, `mcp`, `skills`, `agents`, `pi`, `grok`,
`configuration_preview`) and shared managed state. `commands/state.rs` holds
storage lookups, the credential lock and the apply/restore helpers.

Commands only validate IPC input and schedule work. Provider and model commands
handle instances, API keys and user-triggered model discovery. The Models tab uses
`browse_upstream_models_page` (in-memory browse catalog, optional fuzzy `query`,
no SQLite merge), `save_provider_model_selections` (batch persist/removal),
`refresh_saved_provider_models` and `update_provider_model_edit`. Agent commands
run discovery, the explicit version probe, and the preview/apply/history/restore/
resolution/delete path for the three write adapters. `src-tauri/src/lib.rs` opens
storage, registers plugins and managed state, and registers every command.

### Persistence

`src-tauri/src/storage.rs` owns the private SQLite database in the app-data
folder and its forward-only migrations, currently schema v32. Each feature owns
its SQL:

- `providers.rs` keeps the public entrypoints while `providers/` separates types,
  templates, validation, repository SQL and credential orchestration. Instances
  have stable random IDs, cursor pages and optimistic `revision` checks. SQLite
  stores only a `provider-<id>` reference per key.
- `models.rs` and `models/` hold records, bounded cursors, local lists, fuzzy
  search, manual edits and atomic selection/save transactions. Model capability
  evidence, user declarations, context window, output limit and thinking effort
  are stored in the Provider model definition consumed by Agent adapters.
- `mcp.rs` and `mcp/` validate stdio/Streamable HTTP definitions and store
  non-secret metadata, stable IDs and revisions; every value is an immutable OS
  credential reference.
- `skills.rs` and `skills/` own the central library, import validation and managed
  copies under the app-data folder.
- `settings.rs` and `appearance.rs` store language, theme/brightness and the
  automatic version-probe preference.
- `config_apply/` owns the configuration journal, snapshots, history, restore and
  deletion. See [configuration.md](configuration.md).

### Credentials

`src-tauri/src/credentials.rs` provides the OS credential-store interface, key
validation and the compensation helpers that keep the store and SQLite consistent
when providers and MCP definitions are created, edited or deleted. Credentials are
read only at the moment a command needs them, outside the SQLite lock.

### Reference-aware deletion

`src-tauri/src/references.rs` reads Pi provider bindings and Grok model bindings
before a Provider or model is removed. It classifies each row against the live
Agent file as active, stale or unverified. Active/unverified rows block deletion;
a stale row can be cleared explicitly without rewriting the Agent file. Skill
deletion uses its deployment rows and the same active/stale distinction. The
module only reads Agent configuration; it never writes it.

### Agent discovery and version probing

`agents.rs` owns typed observations and environment/input contracts;
`agents/discovery.rs` performs bounded filesystem discovery independent of Tauri;
`agents/version_probe.rs` runs the fixed `--version` after explicit or
automatically consented confirmation. Presence, version provenance, documented
capabilities, configuration roots and effective settings are separate
observations. See [agents.md](agents.md).

### HTTP and models

`model_http.rs` performs the model-list HTTPS GET: Bearer authentication, bounded
bodies, cancellation and system-proxy support. `model_identity.rs` holds model-ID
grammar and protocol route checks; `model_catalog.rs` parses the phase-1 vendor
responses; `model_search.rs` ranks fuzzy matches. `model_fetch.rs` keeps the public
API while `model_fetch/` separates request/result types, registration and
cancellation, revision/URL-aware sessions, HTTP downloads, browse-only paging,
snapshot/status SQL, snapshot-checked catalog transactions and the credential
snapshot step. Credentials are loaded after the snapshot lock is released and
dropped before the merge; browsing never writes model rows.

### Shared helpers

`shared.rs` holds domain-neutral text checks, random-ID format checks and Unix
milliseconds. Provider and MCP keep their own identity types and map failures into
their own errors. Keep one Rust crate, using `feature.rs` plus a matching
directory, and split crates only when a real need appears.

## Configuration flow

Read existing state → resolve precedence → validate supported options → preview
changes → recheck file state → back up → atomically write → verify → allow
rollback. A write may take effect only after an Agent restarts, so the interface
states that. Environment variables and project-local overrides can supersede
global configuration, and each adapter reports the higher-precedence sources it
located.

## Enablement relationships

The main column keeps a read-only current-configuration pin and separate
deployment state per Agent. Disabling one deployment preserves the central
definition and the other Agents' deployments. Store requested enablement
separately from the last applied state, and track overrides, pending reload,
unsupported mappings and failures explicitly. A green state must not represent an
unapplied request or claim a successful provider request. When the effective
running-session state is unknown, show that next to the enabled configuration.

Agent adapters translate a switch into the verified native operation. Every
mutation uses preview, concurrent-change checks, backups and rollback. Skill
deployment to Claude Code, Pi and Grok Build and Pi MCP deployment are
implemented; Grok MCP deployment remains; see [plans/todo.md](plans/todo.md).

## Planned boundaries

Module boundaries and the reasoning for future GitHub/Vercel integrations, source
installation, Agent installation and local API forwarding are in
[plans/backend-modularity.md](plans/backend-modularity.md). Those modules
(`backup`, `sources`, `installation`, `gateway`, `protocols`) do not exist and must
not appear in the interface.

| Component           | Responsibility                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| Provider adapters   | Authentication, endpoints, model discovery, and protocol-specific fields        |
| Model configuration | Provider/model identity and capability metadata; the source for Agent injection |
| Agent adapters      | Installed-version detection, config precedence, mapping, preview, and injection |
| Skill manager       | Central source tracking, revision updates, symlinks, and copy synchronization   |
| MCP manager         | Shared definitions, credential references, and agent-specific config mapping    |

React calls Rust commands; it does not contact providers or write config files.
Domain functions stay callable in tests without Tauri. Provider and Agent adapters
have different responsibilities and are never combined into a provider-by-agent
matrix of duplicated implementations. Preserve unknown Agent config fields. Skills
and MCP have independent support per Agent, and an adapter must report those
capabilities.

## Later extensions

An optional localhost proxy may support routing, protocol conversion and usage
collection. Direct configuration is preferable when the Agent supports the
provider protocol. Streaming, tool calls, reasoning and images need explicit
compatibility tests before a conversion is marked supported.

A future usage-source boundary can accept provider quota APIs, Agent logs and
proxy usage events. Retain source identity and timestamps, distinguish measured
usage from estimates, and avoid double counting. Quota monitoring and activity
heatmaps are outside phase 1.
