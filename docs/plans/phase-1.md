# Phase 1: shared agent configuration

## Confirmed scope

- MIT-licensed desktop app for macOS, Windows and Linux: stable Rust, edition
  2024, Tauri 2, React and TypeScript. Follow current stable toolchains and APIs.
- Providers: Command Code, DeepSeek and OpenRouter.
- Fixed Agent catalog and read-only detection: Claude Code, Codex, OpenCode, Pi,
  Grok Build and Nous Research Hermes Agent. The six-Agent expansion is planned,
  not implemented yet. Configuration deployment remains scoped to Pi/Grok Build.
- Shared skills with source/revision tracking and updates; shared MCP definitions
  and explicit per-Agent deployment.
- Simplified Chinese/English UI with system detection and persistent choice.
- Beginner-readable code, English code comments, native CI and draft releases.

## Implemented baseline

The scaffold, private SQLite storage, OS credential interface, i18n startup and
preferences, desktop shell, provider/key saving, model discovery/selection/manual
entries and central MCP management are implemented. Provider/model P10–P12 and
Command Code routing P15/C15 have maintainer acceptance; P15 real credential
connection acceptance is maintainer-reported, not a new agent-executed test.

Pi/Grok read-only discovery and window-local manual paths are implemented. It
checks entries and configuration file presence without running executables or
reading configuration contents. Unknown versions remain unverified. Backend
BR1–BR5 has separated commands, domain behavior, repositories and tests.

Agent writes, skill installation/update/deployment, MCP deployment, managed Agent
installation and the later inference checks are not implemented. Local skill
import (P25) is implemented: the system folder picker chooses a skill folder or a
folder of skill folders, each found skill is validated and copied into the
private library with a content digest and a source record, and the original
directories stay untouched. Central MCP saving does not execute or contact a
server. No planned adapter may appear connected or supported.

Detailed completed-task process records belong in Git/CI and `CHANGELOG.md`, not
in the active plans. [todo.md](todo.md) is the sole task-status source, including
remaining real-installation, credential, visual and platform acceptance items.
The macOS/Windows system-style themes use schema v9 but still need visual
acceptance; local or fixture-based checks do not establish other native platforms.

## Current execution order

1. Update the agreed plans, remove stale completed-task detail, commit and push
   the documentation before changing implementation.
2. Implement all six read-only detectors and the fixed catalog, including
   not-found entries and manual paths. Verify each official entry/config contract;
   preserve bounded lookup, safe errors and no-execution rules. See
   [agent-catalog.md](agent-catalog.md).
3. Build the minimal Pi configuration preview (P18), adding only the model
   information and stale-plan checks it needs. Full P16 is not a prerequisite.
   Central models hold capability metadata and aliases; request settings and
   necessary capability overrides belong to the target Agent configuration.
   Verify the installed Agent version can express each setting. See
   [model-parameters.md](model-parameters.md).
4. Add safe writes and restore for Pi, then the independently verified Grok
   adapter: precedence, diff, concurrent-change checks, private backups, atomic
   replacement, preservation of unrelated fields and rollback.
5. Finish the skill workflow on top of the delivered local import (P25):
   source installation, update preview and per-Agent symlink deployment, with an
   explicit copy-sync fallback and protection of local changes.
6. Deploy central MCP definitions only to verified Agent features, using the
   safe write/restore path. Unsupported features remain explicit.
7. Complete native platform and real-integration acceptance, then prepare reviewed
   draft release assets; do not publish automatically.

Architecture and dependency details are in [development-plan.md](development-plan.md)
and [backend-modularity.md](backend-modularity.md). Current UI rules are in
[desktop-shell-design.md](desktop-shell-design.md) and [frontend.md](../frontend.md);
provider/Agent evidence is in `docs/integrations/` and must be rechecked when used.

## Acceptance boundaries

A user can configure verified providers, choose a model and supported settings
for each compatible Agent, preview/apply/restore changes and understand rejected
combinations. Skill updates preserve local edits; MCP maps only to verified
features. Every Agent write has preview, backup and rollback.

Keep secrets out of logs, URLs, frontend persistence and exports. Real requests
require explicitly permitted test credentials; record actual versions and results.
Discovery, managed installation, configuration support, written configuration,
effective runtime state and inference success are separate claims.

## Deferred

Subscription/quota monitoring, token heatmaps, automatic routing/failover,
arbitrary executable plugins, cloud sync and release publication. GitHub backup,
source installation, Agent installation and local API forwarding remain separate
BR-series proposals until a concrete behavior and its boundaries are implemented.
