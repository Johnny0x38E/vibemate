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
  catalog parsing or unknown. The maintainer confirmed the interaction on
  2026-10-14: every checkbox interaction is a user declaration (checked declares
  supported, unchecked declares unsupported), untouched fields keep the catalog
  result or unknown, one restore-default button clears all three declarations,
  and one inline entry form (shared with manual addition: Model ID and alias with
  the actions on the right, the three checkboxes below) saves alias and
  declarations atomically behind the existing content digest. User declarations
  live in their own stored field, so catalog refreshes replace catalog evidence
  without touching them. Later decisions the same day: no per-capability hint
  text or source/state labels in the form (the list icons name the source), the
  shared `ModelCheckbox` style, and editing moved out of its dialog into the
  inline form merged with manual addition.
- Refresh existing saved models from the upstream catalog while preserving aliases;
  browsing alone must not persist the whole catalog or mark unseen pages missing.
- Protect edits and future configuration previews against changed model content.
- Delete a provider with confirmation and safe credential cleanup. Future managed
  Agent references must block deletion until explicitly resolved.
- Do not implement a standalone availability check, test request, or check button.
  The maintainer will verify real operation after deployment to an Agent.

## Delivered batches (2026-10-13/14)

A1–A3 delivered model snapshot digests with conflict-safe alias editing across
the domain, IPC and UI; the native macOS dialog acceptance passed. B delivered
capability-evidence parsing and per-field icons (schema v13; v12 added the Agent
auto-probe preference table). C delivered saved-model refresh that preserves
aliases and only trusts a complete catalog to establish absence. D delivered
provider deletion with credential compensation (`delete_then_commit` and
`delete_outcome_unknown`, with the key restored on failure). E delivered manual
capability editing (2026-10-14): schema v14 stores per-field user declarations
in `capability_overrides` (a separate column, so catalog refreshes never touch
them), one inline `ModelEntryForm` (create and edit modes) saves alias and
declarations together through `update_provider_model_edit` behind the existing
digest, manual creation can declare capabilities immediately
(`add_manual_provider_model` carries the declarations), the three checkboxes
declare supported/unsupported on every interaction, one **Restore default**
clears all declarations, and the capability icons name their source (user
declaration or catalog). The editor moved from a dialog into the inline form
merged with manual addition (maintainer decision 2026-10-14), and the form
keeps no per-capability hint text or source/state labels.
Per-batch tests, counts and process notes are in Git and `CHANGELOG.md`, not
repeated here.

Open item: the P35 managed-Agent reference checks before deletion.

## Checkpoints and boundaries

After each domain batch run Rust fmt, locked Clippy and tests. After each frontend
batch run `pnpm run check:frontend`; after the connected feature run the locked
native no-bundle build. Visual checks use browser preview; native credentials and
Windows/Linux behavior remain separate acceptance items. No real provider request
or user-credential access is authorized by this plan. Pending batches are not
reported as delivered features.
