# Provider completion

Task status remains in [todo.md](todo.md). This plan refines P16 and the provider
portion of P35; it does not replace unrelated pending work.

## Confirmed scope

- Edit aliases on saved manual and fetched models, without changing model IDs.
- Show read-only reasoning, image-input and tool-call icons on model rows only
  when supported by verified catalog evidence. Missing evidence remains unknown;
  protocol routes and model names are not capability evidence.
- Parse and persist capability evidence separately from original row provenance.
- Refresh existing saved models from the upstream catalog while preserving aliases;
  browsing alone must not persist the whole catalog or mark unseen pages missing.
- Protect edits and future configuration previews against changed model content.
- Delete a provider with confirmation and safe credential cleanup. Future managed
  Agent references must block deletion until explicitly resolved.
- Do not implement a standalone availability check, test request, or check button.
  The maintainer will verify real operation after deployment to an Agent.

## Ordered implementation batches

### A1: Model snapshot and alias domain operation (done 2026-10-13)

Files: `models.rs`, `models/alias.rs`, `models/alias_tests.rs`, `model_error.rs`.

`get_model_snapshot` reads one saved model plus a SHA-256 digest of the serialized
record; `update_model_alias` writes only the alias when the digest still matches
inside one immediate SQLite transaction. No migration was needed. Five domain
tests cover fetched and manual rows, clearing, no-op saves, invalid labels,
malformed digests, invalid names and negative timestamps, stale snapshots at an
unchanged timestamp (including routing and metadata changes), two connections and
restart persistence. Rust fmt, Clippy and the full test suite pass.

### A2: Alias IPC and desktop boundary (done 2026-10-13)

Files: `commands/models.rs`, `lib.rs`, `lib/desktop/models.ts`, `models.test.ts`.

The two thin commands cross IPC with `model_changed` added to the stable error
codes. The boundary validates snapshot shape, exact identity and digest, and
rejects mismatched alias acknowledgments. Rust and frontend checks pass.

### A3: Alias editing UI (done 2026-10-13)

`ModelAliasAction` opens a Base UI dialog around `ModelAliasEditor`, which holds
the draft and the stale/unknown recovery states; the row action restores focus
after Escape and after a successful save. The parent's busy state blocks competing
model operations while the dialog is open, and hiding Providers releases that
guard. Behavior tests cover opening, draft blocking, save, clear, conflict reload,
unknown outcomes, failed recovery reads, language changes, Escape and late
replies. The browser preview was checked for keyboard open, Escape close, focus
return and the manual row without capability icons. A native macOS dialog check
and a screenshot remain outstanding.

### B: Capability evidence and icons (done 2026-10-13)

Re-checked against the official pages on 2026-10-13 (see
[providers.md](../integrations/providers.md)): DeepSeek documents
`effort.supported_levels`, `input_modalities` and a per-model feature table;
OpenRouter documents `reasoning`, `architecture.input_modalities` and
`supported_parameters`; Command Code's `GET /models` carries no capability field.
Parsing maps each vendor field to an unknown/supported/unsupported state with the
field recorded as its source; `provider_model.capabilities` (schema v13; v12 is an
empty placeholder reserved by the parallel Agent auto-probe worktree) stores
that JSON, and model rows show the reasoning, image and tool icons only for
supported states. Manual models and Command Code rows start unknown. Rust parser
tests cover missing, null, empty, malformed and valid omission; a repository test
rejects a definitive state without a source. Full frontend and Rust checks pass.

### C: Refresh saved model information (done 2026-10-13)

`refresh_saved_provider_models` reuses the fetch registration, credential and
snapshot steps, downloads the catalog, and calls `refresh_saved_models`: one
immediate transaction rechecks the provider revision/base URL, updates only rows
that are already saved, and never inserts or deletes. `source`, `selected` and
`alias` stay untouched; a manual row stays manual. Only a complete catalog marks a
previously listed saved row as missing, and an incomplete or cancelled download
only updates. The Models tab adds a **Refresh saved models** button, blocked by
checkbox drafts and other busy operations, which re-reads the saved list and
reports how many rows were refreshed. Two Rust tests cover update/no-insert,
alias/source preservation, complete vs incomplete absence, the page cap and
`model_fetch_stale`; the strict boundary test validates the summary contract and
the tab test covers the re-read and the draft block. Full frontend (630 tests),
Rust (268 tests), fmt and Clippy pass. Native macOS and real-credential
acceptance remain separate items.

Define an explicit saved-model refresh operation, bound to the provider snapshot
and saved-model content snapshots. Refresh only saved rows, preserve aliases and
manual provenance, and do not create every catalog row. Only a verified complete
catalog can establish absence. Partial/cancelled fetches must not mark models
missing. Verify concurrent edits, provider changes, partial pagination, failures
and repeat refreshes, then wire the UI and inspect it in preview.

### D: Provider deletion (done 2026-10-13)

`delete_provider` removes one instance, its saved models (foreign-key cascade) and
its OS credential entry. `delete_then_commit` backs up the key in memory, deletes
it, then commits the database delete in one immediate transaction: a statement
failure restores the key and reports the SQLite error; an uncertain `COMMIT`
restores the key as well and reports `delete_outcome_unknown`. A stale
confirmation fails with `revision_conflict` before the credential store is
touched, and an unreadable stored value does not block deletion. Seven Rust tests
cover success, isolation from other providers, stale revision, store failure,
statement failure, uncertain commit, unreadable backup and a legacy provider
without a reference row. The Providers list adds a destructive row action with a
confirmation dialog that names the provider and its saved-model count; a conflict
or unknown outcome reloads the list instead of claiming success. The browser
preview was checked for open, dialog content, Escape close and focus return, and
the confirm path reached `delete_provider` with the expected revision and reported
the acknowledgment. Tab containment through Base UI's focus guards behaved the
same as a canonical Base UI dialog in that preview, so it was not changed. No
Agent configuration file is read or written. Future managed-Agent reference
checks remain part of P35; this batch does not claim that unmanaged external files
were scanned. Full frontend (634 tests), Rust (275 tests), fmt and Clippy pass.

Inspect existing credential compensation before implementation. Separate domain
transaction/cleanup, thin IPC, and bilingual confirmation UI into small batches.
Require the expected provider revision, show the affected saved-model count,
reject active/conflicting operations, and preserve a recoverable state on cleanup
failure. No Agent files are modified. Add future reference checks with managed
Agent bindings, not a claim that unmanaged external files have been scanned.
Verify cleanup failures, stale confirmations, fetch/delete races and recovery.

## Checkpoints and boundaries

After each domain batch run Rust fmt, locked Clippy and tests. After each frontend
batch run `pnpm run check:frontend`; after the connected feature run the locked
native no-bundle build. Visual checks use browser preview; native credentials and
Windows/Linux behavior remain separate acceptance items. No real provider request
or user-credential access is authorized by this plan. Pending batches are not
reported as delivered features.
