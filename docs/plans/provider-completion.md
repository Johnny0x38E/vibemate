# Provider completion

Task status remains in [todo.md](todo.md). This plan refines P16 and the provider
portion of P35; it does not replace unrelated pending work.

## Confirmed scope

- Edit aliases on saved manual and fetched models, without changing model IDs.
- Show read-only reasoning, image-input and tool-call icons on model rows only
  when supported by verified catalog evidence. Missing evidence remains unknown;
  protocol routes and model names are not capability evidence.
- Parse and persist capability evidence separately from original row provenance.
- Manual capability editing (maintainer requirement, 2026-10-13, recorded under
  P16 in [todo.md](todo.md)): the model editor offers thinking/image/tool
  checkboxes plus a restore-default action; a user selection states itself as the
  source and overrides catalog evidence, and restoring the default returns to
  catalog parsing or unknown.
- Refresh existing saved models from the upstream catalog while preserving aliases;
  browsing alone must not persist the whole catalog or mark unseen pages missing.
- Protect edits and future configuration previews against changed model content.
- Delete a provider with confirmation and safe credential cleanup. Future managed
  Agent references must block deletion until explicitly resolved.
- Do not implement a standalone availability check, test request, or check button.
  The maintainer will verify real operation after deployment to an Agent.

## Delivered batches (2026-10-13)

A1–A3 delivered model snapshot digests with conflict-safe alias editing across
the domain, IPC and UI; the native macOS dialog acceptance passed. B delivered
capability-evidence parsing and per-field icons (schema v13; v12 added the Agent
auto-probe preference table). C delivered saved-model refresh that preserves
aliases and only trusts a complete catalog to establish absence. D delivered
provider deletion with credential compensation (`delete_then_commit` and
`delete_outcome_unknown`, with the key restored on failure). Per-batch tests,
counts and process notes are in Git and `CHANGELOG.md`, not repeated here.

Open items: the P35 managed-Agent reference checks before deletion, and the
manual capability-edit UI recorded under P16 in [todo.md](todo.md).

## Checkpoints and boundaries

After each domain batch run Rust fmt, locked Clippy and tests. After each frontend
batch run `pnpm run check:frontend`; after the connected feature run the locked
native no-bundle build. Visual checks use browser preview; native credentials and
Windows/Linux behavior remain separate acceptance items. No real provider request
or user-credential access is authorized by this plan. Pending batches are not
reported as delivered features.
