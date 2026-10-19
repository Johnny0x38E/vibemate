# Phase 1: shared agent configuration

## Confirmed scope

- MIT-licensed desktop app for macOS, Windows and Linux: stable Rust, edition
  2024, Tauri 2, React and TypeScript. Follow current stable toolchains and APIs.
- Providers: Command Code, DeepSeek and OpenRouter.
- Fixed Agent catalog and read-only detection: Claude Code, Codex, OpenCode, Pi,
  Grok Build and Nous Research Hermes Agent. All six read-only detectors are
  implemented. Configuration deployment proceeds Claude Code first, then Pi and
  Grok Build; the maintainer uses Pi during coding, so Pi is never the first
  write target.
- Shared skills with source/revision tracking and updates; shared MCP definitions
  and explicit per-Agent deployment.
- Simplified Chinese/English UI with system detection and persistent choice.
- Beginner-readable code, English code comments, native CI and draft releases.

## Implemented baseline

The scaffold, private SQLite storage, OS credential interface, i18n startup and
preferences, desktop shell, provider/key saving, model discovery/selection/manual
entries, model aliases/capability evidence/saved-model refresh, safe provider
deletion and central MCP management are implemented. Provider/model P10–P12 and
Command Code routing P15/C15 have maintainer acceptance; P15 real credential
connection acceptance is maintainer-reported, not a new agent-executed test.
Six-Agent discovery, version reading, local skill import, the central MCP
credential-store checks and the provider/model additions passed maintainer
acceptance on 2026-10-13; central MCP deletion now includes confirmation, revision protection and durable
credential cleanup retry; its acceptance is tracked in todo.md.

Six-Agent read-only discovery and independent window-local manual paths are
implemented. Automatic discovery checks entries and configuration file presence
without running executables or reading configuration contents. Optional CLI
version reading is a separate, explicitly confirmed operation, not a sandbox;
CLI provenance never establishes configuration capabilities or deployment support.
Official identification marks and accessible green/gray/red status dots are
implemented. Backend BR1–BR5 has separated commands, domain behavior,
repositories and tests.

Claude Code configuration preview, apply and restore (P18–P21) are implemented
and have maintainer acceptance: preview/apply on 2026-10-14 and native restore
on 2026-10-16. The shared write path provides change checks, private backups,
atomic replacement and recoverable operation records. Pi configuration preview, backups, apply and restore are implemented for the
compatible published Pi 1.x format (verified with 1.2.0 and 1.3.0). The maintainer accepted Pi 1.3.0 preview compatibility and readable provider names on 2026-10-16; real Provider/session acceptance remains in PI1. Grok Build
configuration preview, apply and restore (P22–P23) are implemented against the
installed 1.0.60 contract and accepted by the maintainer on 2026-10-16. Its skill
and MCP
deployment, managed Agent installation and the later compatibility checks
remain pending. Local skill
import (P25) is implemented: the system folder picker chooses a skill folder or a
folder of skill folders, each found skill is validated and copied into the
private library with a content digest and a source record, and the original
directories stay untouched. Central MCP saving does not execute or contact a
server. No planned adapter may appear connected or supported.

Detailed completed-task process records belong in Git/CI and `CHANGELOG.md`, not
in the active plans. [todo.md](todo.md) is the sole task-status source, including
remaining real-installation, credential, visual and platform acceptance items.
The macOS/Windows system-style themes passed visual acceptance on 2026-10-13;
Windows/Linux display verification is deferred to the release-time platform check.

## Current execution order

The six-Agent catalog, version reading and local skill import passed maintainer
acceptance on 2026-10-13; Windows/Linux and screen-reader verification is
deferred to the release-time platform check.

1. Deliver Pi preview, original-file backups and application together (PI1), as
   requested on 2026-10-16. Reuse the accepted file writer and history restore;
   preserve the maintainer's existing configuration. Track real Pi session
   acceptance in [todo.md](todo.md).
2. Grok Build preview, apply and restore (P22–P23) landed on 2026-10-16 against
   the installed 1.0.60 contract and closed with maintainer acceptance the same
   day, without a launcher or local bridge. Grok skill and MCP deployment follow,
   reusing the same safety rules. Central models hold metadata; request settings
   belong to the target Agent and require verified native mappings, and the
   Anthropic Messages backend stays refused until Grok documents how it sends a
   per-model key.
3. Complete the follow-up configuration work before advancing to Skill/MCP deployment:
   H1–H3 add durable snapshots before every actual rewrite (including restores),
   full history browsing, direct restoration of any retained pre-write version,
   and confirmed deletion of a record with its backups. The first H1 backend slice
   now retains independent pre-mutation snapshots for apply, restore and
   compensation (schema v20), without changing the history UI. Read-only backend
   inspection now compares interrupted mutations with actual files and verifies
   their backups; inspection alone neither settles records nor unlocks recovery.
   Explicit backend resolution now atomically records reviewed observations and
   repairs legacy results (schema v21), including completed file results whose
   legacy step result was lost. Shared cross-instance file locking excludes live
   mutations; ambiguous, changed or unreadable observations remain blocked.
   The three supported Agent history views now expose user-triggered inspection
   and explicit observation recording through strict IPC. Approval binds the full
   backend plan; recording never restores files, and any restore still needs its
   own preview and confirmation. Backend inspection and explicit cleanup now
   manage unreferenced private copies under the same cross-instance lock and a
   SQLite write transaction, protecting all legacy/revision references and leaving
   failed cleanup discoverable after restart; cleanup still has no IPC/UI entry.
   A backend-only history reader now combines original operations and independent
   restore/compensation batches, retaining reconciliation audits. Exact Agent/root
   file scopes and bounded seek cursors keep other configurations out; each page
   reads one SQLite snapshot without opening target or backup files. Existing
   history IPC/UI still uses the latest-ten, Agent-only listing; it has not been
   switched to this reader. Nothing runs automatically. Next, integrate complete
   history browsing and root isolation into IPC/UI, verify backup availability and
   detect/pin the current version. H1 as a whole is not accepted.
   P24.a–P24.d explicitly
   deliver verified per-Agent mappings and UI for reasoning capabilities/efforts,
   context windows, output metadata and supported request settings. The remaining
   history work and parameter mappings are still pending despite acceptance of
   the minimal write adapters; see [todo.md](todo.md),
   [config-apply.md](config-apply.md) and [model-parameters.md](model-parameters.md).
4. Finish the skill workflow on top of the delivered local import (P25):
   source installation, update preview and per-Agent symlink deployment, with an
   explicit copy-sync fallback and protection of local changes.
5. Deploy central MCP definitions only to verified Agent features, using the
   safe write/restore path. Unsupported features remain explicit.
6. Complete native platform and real-integration acceptance, then prepare reviewed
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
