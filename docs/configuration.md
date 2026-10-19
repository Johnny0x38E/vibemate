# Configuration writes, history and restore

This page describes the shared mechanism every Agent write uses: Claude Code,
Pi and Grok Build today, and any later adapter. Protocol details per Agent live
in [`integrations/`](integrations); the Agent catalog is in
[`agents.md`](agents.md).

## Safety rules

A write may only touch a file when it can prove the change is still the one the
user reviewed.

- A preview binds the target file digests, the Provider revision, the model
  content digest and any adapter-specific token such as a Grok alias. Any change
  makes the plan stale; the interface has to preview again.
- The last path component is never followed when it is a symbolic link, so the
  atomic replacement cannot land somewhere else.
- File permissions are preserved. A pre-existing target keeps its mode; a new
  target is created as 0600 because it may hold a credential.
- A read-only target is refused even when the directory would allow `rename`.
- Errors and logs carry stable codes and paths, never file contents or values.
- Every private copy lives in a 0700 directory with 0600 files. The interface
  receives a locatable path and a digest, never bytes.

The flow is: discover the installation, resolve precedence, read current state,
validate the supported options, build a sanitized plan, recheck the file state,
write a private backup, write a temporary file in the same directory and rename
it, verify, and allow restore.

## Preview presentation

Every configuration preview uses the same two layers:

1. **Field summary.** Per file, the field paths with added, removed, replaced or
   container changes, and sanitized old/new values. Absent, unset, empty string
   and `null` are distinct. A file that is created or deleted is stated as such.
2. **Complete text.** On request, a dialog shows the full native JSON or TOML for
   each file, read-only and highlighted, with a bounded line diff and file
   switching. The model preview compares the current file with the planned
   result; a history restore compares the current file with the retained version.

Rust builds both layers from the same native plan that apply will use, so the
frontend never assembles a configuration by itself. Text is masked for
credential-like keys across the whole document, not only the keys vibemate owns.
An unparsable document is reported as unavailable instead of being shown raw.
Original and sanitized text are each limited to 1 MiB; JSON indentation growth
that would exceed the limit is refused rather than truncated. TOML keeps the
user's comments and layout through `toml_edit`.

The detail dialog focuses its title first, traps focus, closes on Escape and
returns focus to the button that opened it. Loading, failure and retry are
explicit.

Applying stays blocked until the field summary has loaded. A stale detail read or
a stale restore is refused with `plan_stale` and writes nothing.

## Write journal

`schema v16–v23` extend the journal that makes a write recoverable.

- Before any target is replaced, an operation row and its per-file steps are
  written in a transaction. A multi-file write cannot be a filesystem
  transaction, so a failure compensates the steps already applied and records
  `compensated`, `partial` or `unknown`.
- Schema v20 adds `config_file_revision`: an independent snapshot of each file's
  bytes or absence, written before every actual rewrite, including restores and
  compensation. Snapshots for successful writes are never overwritten or pruned
  automatically.
- Schema v21 records an explicit resolution of an interrupted write with
  `config_revision_resolution`. A resolution records an observation; it never
  restores a file.
- Schema v22 marks direct version restores (`direct = 1`). They do not edit the
  source operation's legacy rollback state and are not mistaken for a missing
  step result.
- Schema v23 adds a durable delete journal (`prepared` / `failed` / `completed`)
  and exclusive backup-target rows, so a delete can be retried without hiding a
  history entry while copies that may hold secrets still exist.

Apply, manual restore, compensation and explicit resolution take a permanent
exclusive `.config-lock` file next to the database before the SQLite lock, using
the standard library `File::lock` (flock on macOS/Linux, `LockFileEx` on Windows).
This stops a second instance from treating an in-progress `prepared` record as an
interrupted one. It does not lock an external Agent or editor and does not
replace the digest checks.

## History

Each supported Agent's detail page reads a root-scoped, paginated journal through
`configuration_history_page`. The request contains only the Agent identity, a
seek cursor and a page size; Rust resolves the configuration root from the same
environment precedence as discovery, so history stays readable after an
installation or directory is removed.

- Entries are newest first. An apply operation and each restore or compensation
  batch are separate entries. A multi-file batch is never split across pages.
- Each entry lists time, kind, result, per-file state, the recorded Provider name
  (or the retained reference when the Provider was deleted), and per-file backup
  availability. Only a retained private copy with a matching digest counts as
  available; a journal reference alone does not.
- Every page also returns the live `current` configuration, compared by actual
  file digests with retained versions. A full match names one history entry and,
  for that entry, the Provider display name plus the live model ID and its saved
  alias. External edits, unreadable files and partial multi-file matches are
  reported as `unrecorded`, `unreadable` or `partial` instead of naming the last
  operation as current.
- No response contains configuration bytes, backup paths or recovery approval.
- Interrupted writes set `requiresResolution` and withdraw the legacy restore
  action.

The frontend boundary (`src/lib/desktop/history.ts`) validates the nested
response with strict Zod schemas: unknown fields, contradictory journal or
reconciliation metadata, invalid ordering, duplicate identities and mismatched
cursors reject the whole page with a safe error code. Validation diagnostics and
raw responses are discarded.

### Inspecting an interrupted write

`review_configuration_resolution` accepts only a fixed Agent identity and an
operation ID. It compares the journal with the files and private copies as they
are now, per file: matching the pre-write state, matching the intended state, both,
externally changed, or unreadable, plus backup integrity.

`resolve_configuration_operation` accepts the Agent, the operation ID and the
approval digest of the complete reviewed plan. It rechecks the approved set inside
one transaction with the shared lock and records the observation; ambiguous,
changed or unreadable observations stay blocked. Recording never restores a file,
never reads credentials and never changes configuration.

### Restoring a retained version

Any retained apply entry or restore/compensation batch can be restored to its
pre-entry state, including the oldest original state. Restoring does not require
undoing later writes one by one, and an already restored entry remains a valid
source.

- The scope is exact: the Agent's configuration root and its owned direct child
  files. A symbolic link, an unreadable or oversized target, a read-only target, a
  missing or damaged snapshot, or an unresolved interrupted write blocks the
  whole entry.
- The preview returns current digests. Confirmation rebuilds the plan under the
  shared lock and refuses any changed file, snapshot or journal entry with
  `plan_stale`.
- A restore first stores the current bytes, so it is itself a new undoable batch.
  No retained snapshot is consumed.
- Restoring can write back an old plain-text key. The dialog says so, and the
  saved OS credential is not changed.

### Deleting history

Each deletable entry, including the earliest and already restored versions, has a
bilingual confirmation that states the scope and that the deletion cannot be
undone. Deletion removes only that entry's exclusive private copies and its
journal row.

- Unfinished writes (`history_active`), interrupted revisions
  (`history_unresolved`) and apply entries that still have restore or
  compensation batches (`history_referenced`) are refused.
- Shared backup references are never removed because another entry still uses
  them.
- Database and file deletion are not one transaction. The durable state and idempotent
  retry mean an interrupted or failed delete stays discoverable and retryable
  instead of leaving an unowned copy that may hold secrets. File deletion is not
  secure erasure.
- Deletion does not change the Agent's current configuration, the Provider, the
  saved credential or the model alias ownership.

## Module layout

| Path                                  | Responsibility                                                              |
| ------------------------------------- | --------------------------------------------------------------------------- |
| `src-tauri/src/config_plan.rs`        | Target file identity and digest, plan changes, notices and refusal reasons. |
| `src-tauri/src/config_apply.rs`       | Facade for the write facade and its submodules.                             |
| `config_apply/operations.rs`          | Journal rows, step states, listing and unfinished operations.               |
| `config_apply/service.rs`             | The write sequence: checks, backup, atomic replacement and compensation.    |
| `config_apply/revisions.rs`           | Independent pre-write snapshots.                                            |
| `config_apply/history.rs`             | Root-scoped paginated reads and the current-configuration pin.              |
| `config_apply/version_restore.rs`     | Restore rules for any retained version.                                     |
| `config_apply/version_text.rs`        | Field summaries, sanitized complete text and credential masking.            |
| `config_apply/history_delete.rs`      | Durable deletion state and cleanup.                                         |
| `config_apply/cleanup.rs`             | Explicit cleanup of unreferenced private copies (backend only).             |
| `config_apply/inspection.rs`          | Read-only inspection and explicit resolution of interrupted writes.         |
| `agents/history.rs`                   | Adapter-scoped history IPC, Provider labels and the Agent file lists.       |
| `src/features/agents/RestoreView.tsx` | History UI, version restore, resolution review and deletion.                |

## Verification scope

The mechanism is covered by fault-injection tests for concurrent changes,
symbolic links, read-only targets and directories, failed replacement, partial
multi-file writes, backup and journal failures, restart, stale approvals and
deletion refusals, using temporary directories and real SQLite.

An isolated macOS Tauri run with synthetic OS credentials verified all three
Agents' A→B→C→A→C restore chain, stale reads and writes, independent snapshots
and exact-byte restoration, including Pi's absent-file state, plus a browser
review of both languages, keyboard and focus, retry, JSON/TOML and a 720×560
content viewport. This is agent-run verification. Maintainer sign-off for the
History interface's later additions and Windows/Linux native behavior remain
tracked in [`plans/todo.md`](plans/todo.md).
