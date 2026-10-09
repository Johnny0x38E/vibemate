# Phase 1: shared agent configuration

## Confirmed choices

- Name: vibemate. License: MIT.
- Desktop: current stable Rust + stable Tauri + React + TypeScript.
  Use the latest stable Rust edition and APIs; no nightly features.
  The initial scaffold uses edition 2024 and Tauri 2.
- Providers: Command Code GOAT, DeepSeek, OpenRouter.
- Agents: Pi, Grok Build.
- Shared skills with source tracking and updates; shared MCP definitions.
- Clear English code comments; beginner-readable implementation; GitHub CI.

## Current state

The desktop scaffold, read-only version command, documentation, CI workflow,
and changelog-driven draft release workflow are present. All provider and agent integrations remain unimplemented.
No credentials have been collected and no agent configuration has been changed.

## Implementation order

1. **Verify integration contracts.** Record exact product identities, official
   URLs, API protocols, authentication, and model discovery behavior. Inspect
   installed Pi and Grok Build versions, supported config locations, precedence,
   skills, and MCP support. The exact Command Code GOAT and Grok Build contracts
   need confirmation; do not invent endpoints or assume MCP support.
2. **Provider and model configuration.** Add validated provider/model records,
   OS credential references, persistence with schema migrations, connection
   checks, model discovery where supported, and manual model entries. Separate
   capabilities from request parameters; support provider-specific fields.
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
