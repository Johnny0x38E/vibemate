# Central Skill library

`src/features/skills/` and `src-tauri/src/skills/` implement the central Skill
library. Installing from a remote source, update comparison and deployment to an
Agent are not implemented; see [`plans/todo.md`](plans/todo.md).

## Scope

- Import existing local Skill folders (a directory holding `SKILL.md`) into the
  central library.
- Store managed copies in the app data directory; the original folders are never
  moved, modified or deleted.
- Record a stable ID, content digest, origin path, an optional upstream source and
  revision.
- Page the list with a bounded cursor; an unrecognized upstream source is shown
  as not auto-updatable.
- Delete a library entry with a bilingual confirmation.

## Data model

Migration v10 created `skill`; v11 rebuilt it to widen the content limits. Files
live in `<app-data>/skills/<id>/`. Manager copies move with the database, so a
backup of the app-data folder stays complete.

| Field                            | Meaning                                                         |
| -------------------------------- | --------------------------------------------------------------- |
| `id`                             | 32 lowercase hex characters; also the managed directory name    |
| `name`                           | Unique Skill name, 1–64 characters                              |
| `description`                    | From the frontmatter, 1–1024 characters                         |
| `origin_path`                    | Normalized absolute import directory                            |
| `source_url` / `source_revision` | User-supplied upstream source and revision; empty means unknown |
| `content_digest`                 | SHA-256 over the ordered relative paths and contents            |
| `file_count` / `byte_size`       | Managed copy size, for display and bounds                       |
| `revision`                       | Metadata revision, incremented on source edits                  |
| `imported_at` / `updated_at`     | Unix milliseconds                                               |

`origin_path IS NOT NULL OR source_url IS NOT NULL` guarantees that every record
has at least one explainable source. A digest is computed as `"vibemate-skill-v1\0"`
followed, for each file sorted by its `/`-joined relative path, by
`u64_be(path length) + path + u64_be(content length) + content`, so the same
directory hashes the same on every platform.

## Import

**Import locally** opens the system folder picker through Rust
(`tauri-plugin-dialog`, called from the blocking pool). The webview never calls
the plugin's own IPC commands, so no new capability and no `fs` permission is
granted.

1. Validate the path string (absolute, bounded, no control characters); the
   canonical path must be a directory outside the library directory.
2. Discover candidates: the selected folder itself when it holds `SKILL.md`,
   otherwise every direct child directory that does. Deeper levels are not
   searched.
3. Parse each `SKILL.md` YAML frontmatter for `name` and `description`; other keys
   are ignored. The frontmatter block is limited to 64 KiB, while the body may be
   long.
4. Walk the source with bounded traversal and build a file manifest; symbolic
   links and special files are refused.
5. Copy into `skills/.import-<random>/` while computing the digest and size. A
   source change during traversal fails the whole import.
6. Rename atomically to `skills/<id>/` and write the database row in one
   transaction (the name is unique).
7. If the database write fails, delete the managed copy; if that also fails,
   report `outcome_unknown` and require a reload.

One failing candidate does not stop the others: the command reports `imported`
and `skipped` (folder name plus a safe reason code). Cancelling the picker is not
an error.

Bounds: one path segment 1–128 bytes, total relative path ≤ 512 bytes, depth ≤ 16,
one file ≤ 16 MiB, total ≤ 64 MiB, at most 2048 files, at most 64 Skill folders
per selection. `.git` is skipped at every level. Import never executes anything
from a Skill and never reads files outside it.

## Source semantics

- A recorded source URL is displayed with its revision and marked as not yet
  auto-updatable.
- A local-only import shows **Local import** / 「本地导入」.
- The library requires at least one of the two, so an entirely unknown source
  cannot be stored.

Auto-update is decided by whether the source is explicit, not by who installed the
Skill. A locally imported Skill that later gains a source address and a verified
fetch method joins the same update flow, which is why the fetch-method field
arrives with the install work rather than now. Filling in an address today only
records and displays it. If that decision changes, the interface text must change
with it.

## Deletion

The list row's delete action opens a bilingual confirmation naming the Skill and
stating that only vibemate's managed copy is removed while the import folder
stays.

1. Validate the ID and the `expected_revision` the user saw; a stale confirmation
   returns `revision_conflict` without touching files.
2. Rename `skills/<id>/` to `skills/.delete-<id>/` atomically (continue when the
   copy is already gone).
3. Delete the row in one transaction by revision; restore the rename on refusal,
   and report `outcome_unknown` if restoring fails.
4. Delete the parked copy; failure does not change the delete result but returns
   `cleanup_pending`, and the interface keeps a retry.

`cleanup_deleted_skill_copies` removes only parked copies with no matching record.
A parked copy whose record still exists comes from an interrupted delete and
stays until that record is deleted. There are no deployment relationships yet, so
deletion cannot affect an Agent configuration; the deployment work must add a
reference check.

## Interface

- Each list row shows the name, at most two description lines and the source
  state. File count, size, digest, paths and timestamps stay in the detail page.
- The library total sits above the list and comes from the paged response, so it
  does not depend on how many pages were loaded.
- The detail page shows the facts and edits the upstream URL and revision with the
  usual conflict and unknown-outcome handling. Opening **Detail** re-reads the
  Skill so that edits start from the latest revision.
- Digests stay out of the interface: they are a machine comparison value and a
  baseline for the future update comparison.

## Verification scope

Rust tests cover path rules, unknown sources, duplicate names, parse failures,
links and limits, the untouched source directory, deterministic digests, folder
discovery and batch reporting, large bodies and headers, and the IPC contract;
storage tests cover the v11 migration and preserved rows.

A real collection check imported all 13 Skills each from
`~/.skills-manager/skills`, `~/.grok/skills` and `~/.agents/skills` with no
skips, including an 11 MiB skill with 315 files and an 87 KiB `SKILL.md`.
Maintainer acceptance on 2026-10-13 confirmed the native picker and the deletion
dialog passed. Windows/Linux native behavior remains a release-time check.
