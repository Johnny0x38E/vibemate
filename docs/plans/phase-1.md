# Phase 1: shared agent configuration

## Confirmed choices

- Name: vibemate. License: MIT.
- Desktop: current stable Rust + stable Tauri + React + TypeScript.
  Use the latest stable Rust edition and APIs; no nightly features.
  The initial scaffold uses edition 2024 and Tauri 2.
- Providers: Command Code, DeepSeek, OpenRouter.
- Agents: Pi, Grok Build.
- Shared skills with source tracking and updates; shared MCP definitions.
- Phase-1 UI internationalization: Simplified Chinese (`zh-CN`) and English (`en`),
  system-language detection, persistent manual selection, and translation checks.
- Clear English code comments; beginner-readable implementation; GitHub CI.

## Current state

The scaffold, private configuration storage, credential-storage foundation,
Simplified Chinese/English startup preferences, desktop shell and basic appearance
are implemented. The maintainer closed the current appearance iteration on
2026-10-10: expanded/collapsed branding, macOS Dock refinements, six light/dark
themes and full-width General/About settings. Theme order is Forest, Ink,
Graphite, Linen, Iris, Ocean; schema v4 preserves existing preferences.

CI and changelog-driven draft release workflows are present. Integration
contracts are documented in `docs/integrations/`. P10–P11 deliver the Providers
page with API keys and unified save. P12 adds Rust-side model fetch, storage,
search and IPC. The tabbed Models UI, create gate, dual-view browse/save and
manual model form are implemented per [models-tab-ux.md](models-tab-ux.md).
The maintainer verified browse/save for three providers in Tauri (2026-10-11);
manual-model native acceptance remains pending.

P31 central MCP management is implemented ahead of Agent deployment at the
maintainer's request: structured stdio/Streamable HTTP definitions, protected
env/header values, editing and central enablement. No MCP server is launched or
contacted, and no Agent config is written. Agent deployment, skills and inference
checks remain pending. [todo.md](todo.md) is the sole task-status source; current
UI rules and verification are in [desktop-shell-design.md](desktop-shell-design.md)
and [frontend.md](../frontend.md).

## Execution plan

The detailed development plan is [development-plan.md](development-plan.md).
The single task checklist is [todo.md](todo.md). These files expand the same
phase-1 scope; this overview does not contain a second execution checklist.
Implementation is authorized. The pnpm migration is recorded as P00, and
P01 provider evidence is in [providers.md](../integrations/providers.md).
Provider configuration and model discovery are implemented; Agent deployment
and inference verification remain pending. Documented contracts do not establish
a working inference connection or runtime compatibility.

## Implementation order

1. **Verify integration contracts.** Record exact product identities, official
   URLs, API protocols, authentication, and model discovery behavior. Inspect
   installed Pi and Grok Build versions, supported config locations, precedence,
   skills, and MCP support. The exact Command Code and Grok Build contracts
   need confirmation; do not invent endpoints or assume MCP support.
2. **Provider and model configuration.** Add validated provider/model records,
   OS credential references, persistence with schema migrations, a model list
   fetch that runs only when the user asks (it reads the system proxy and can be
   cancelled at once), ticked model selections, and manual model entries as a
   supplement. A minimal user-triggered inference check comes later. Separate
   capabilities from request parameters; support provider-specific fields.
   Central MCP definition management (P31) was brought forward before Agent
   injection. Its implementation does not unblock P32/P33 until safe Agent writes
   and rollback exist.

3. **Agent injection.** Implement the first verified agent adapter end to end,
   then the second. Detect installations, preview changes, preserve unrelated
   fields, check concurrent edits, back up, write atomically, and restore.
4. **Skills.** Track GitHub and installer-origin skills, source URLs, revisions,
   and local modifications. Inspect source/update differences before installation.
   Inject with symlinks where supported and copy synchronization otherwise.
5. **MCP.** Validate shared stdio/HTTP definitions as supported by each agent,
   resolve credential references in Rust, preview config changes, and inject.
6. **Cross-platform acceptance.** Validate path handling, symlink permissions,
   config preservation, rollback, credentials, and native builds on all targets.

## Acceptance criteria

A user can configure each verified provider, select a model and supported
parameters, and apply it to each compatible agent. Unsupported combinations
have a clear explanation. Global skills can be synchronized and updated without
silently overwriting local edits. MCP configurations map only to supported agent
features. Every agent write has a preview, backup, and rollback path.

Keep secrets out of logs and exports. Never display a planned adapter as working.
Use native CI builds; perform real integration checks only with explicitly
provided test credentials and report their actual results.

## Deferred

Subscription/quota monitoring, token heatmaps, automatic routing/failover,
arbitrary third-party executable plugins, cloud sync, and release publication.
A local proxy is optional and only introduced for a demonstrated requirement.
