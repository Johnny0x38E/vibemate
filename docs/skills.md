# Central Skill library

`src/features/skills/` and `src-tauri/src/skills/` implement the central Skill
library. Installing from a confirmed GitHub source is implemented. Update
comparison and deployment to an Agent are not; see
[`plans/todo.md`](plans/todo.md).

## Scope

- Import existing local Skill folders (a directory holding `SKILL.md`) into the
  central library.
- Install from a public GitHub repository at a pinned Git revision, or through
  the reviewed `skills` npx installer.
- Store managed copies in the app data directory; original folders are never
  moved, modified or deleted.
- Record a stable ID, content digest, origin path or upstream source, revision,
  fetch method and the reviewed install command.
- Page the list with a bounded cursor; an address with no fetch method is shown
  as not auto-updatable.
- Delete a library entry with a bilingual confirmation.

## Data model

Migration v10 created `skill`; v11 rebuilt it to widen the content limits; v27
added the fetch method and installer fields. Files live in
`<app-data>/skills/<id>/`. Managed copies move with the database, so a backup of
the app-data folder stays complete.

| Field                                     | Meaning                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------- |
| `id`                                      | 32 lowercase hex characters; also the managed directory name              |
| `name`                                    | Unique Skill name, 1–64 characters                                        |
| `description`                             | From the frontmatter, 1–1024 characters                                   |
| `origin_path`                             | Normalized absolute local import directory                                |
| `source_url` / `source_revision`          | Upstream source and pinned revision; empty means unknown                  |
| `fetch_method`                            | `git` or `npx`; empty means only an address is known                      |
| `installer_version` / `installer_command` | Reviewed npx version and exact command; empty for Git and unknown sources |
| `content_digest`                          | SHA-256 over the ordered relative paths and contents                      |
| `file_count` / `byte_size`                | Managed copy size, for display and bounds                                 |
| `revision`                                | Metadata revision, incremented on source edits                            |
| `imported_at` / `updated_at`              | Unix milliseconds                                                         |

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

## Remote installation

**Install from source** reviews one of two acquisition paths. Both run in a
private temporary workspace under the system temporary directory; the workspace
is removed on every outcome. The reviewed command is shown before any process
starts. Nothing inside a Skill is executed.

### Git

1. Accept a public GitHub repository as `owner/repo` or an HTTPS `github.com`
   URL, plus a tag, branch or commit using ASCII letters, digits, `.`, `_`, `/`
   and `-`. Credentials, other hosts, extra path segments, query strings and
   fragments are refused.
2. Run Git with a cleared environment, a temporary `HOME`, no system or global
   Git configuration, no terminal prompt, no LFS smudge and no submodules.
3. Fetch exactly the requested revision with depth 1, check it out detached and
   resolve `HEAD` to a full commit. The resolved commit, not a moving branch
   name, is stored as `source_revision`.
4. Refuse a checkout that contains `.gitmodules`; a nested repository is a
   second source this install did not review.

### npx

The only allowlisted installer is `skills` at the reviewed version `1.7.4`.
A different exact version is refused rather than executed. The command is:

```text
npx --yes skills@1.7.4 add <github-url> --skill <name-or-*> --copy -g -a claude-code -y
```

It runs with a temporary `HOME`, `USERPROFILE` and npm cache, and with telemetry
disabled. The installed canonical copies are read from `<home>/.agents/skills`,
then `<home>/.claude/skills` as a fallback. The installer's stdout is never shown
or logged; only its exit status, the reviewed command and the imported rows are
reported. Node.js 22.20 or later is required by the installer.

### Shared result

- A missing `git` or `npx` executable reports `installer_unavailable`; a non-zero
  exit reports `installer_failed`; a timeout stops the process.
- Discovery is bounded to 64 candidates and six directory levels. Hidden folders
  and symbolic links below the acquired root are skipped, and a link inside a
  real Skill is refused by the normal manifest inspection.
- Each candidate goes through the same `SKILL.md` frontmatter, path, link, file
  count and size checks as a local import, then the same atomic copy and digest.
- Candidates with a duplicate name or invalid manifest appear in `skipped`; the
  others are imported. A failed acquisition leaves the library unchanged.
- Remote rows have no `origin_path`. They record the canonical source URL, the
  resolved Git commit or no revision for npx, `fetch_method`, the reviewed npx
  version when applicable, and the exact command.
- The source editor can add a fetch method to an existing local import. A method
  requires a GitHub address; Git requires a revision, npx has no Git revision.
  Clearing the address clears the method. A hand-entered address without a method
  remains metadata only and is not shown as auto-updatable.

Private repositories are not supported in this slice: the install path
deliberately forwards no Git credential helper or token.

## Source semantics

- A source with `fetch_method = git` shows its address, pinned revision and
  **Git**. A source with `fetch_method = npx` shows its address and
  **skills@1.7.4**. Both are the baseline P29 will follow.
- A hand-entered address without a fetch method is labelled **Manual source** and
  remains non-updatable. Recording an address alone must never imply an update
  path.
- A local-only import shows **Local import** / 「本地导入」.
- The library requires at least one of the two, so an entirely unknown source
  cannot be stored.

Auto-update is decided by whether an explicit fetch method and its fields are
recorded, not by who installed the Skill. A locally imported Skill can gain the
same Git or npx fields in its source editor; P29 will use those fields.

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
  state, including **Git**, **skills@1.7.4** or **Manual source**. File count,
  size, digest, paths, timestamps and the recorded install command stay in the
  detail page.
- **Install from source** opens a dialog with the method, repository or source,
  revision, and an optional Skill name. **Review command** validates the fields
  and shows the exact command; only the following **Install** action runs it.
  The result names imported and skipped folders, the resolved commit or installer
  version, and the exit status. Closing after a result returns focus to the
  header button.
- The browser preview invents no command or result; it reports that installation
  needs the desktop app.
- The library total sits above the list and comes from the paged response, so it
  does not depend on how many pages were loaded.
- The detail page shows the facts, the recorded command, and edits the source
  address, revision method and method-specific fields with the usual conflict and
  unknown-outcome handling. Opening **Detail** re-reads the Skill so that edits
  start from the latest revision.
- Digests stay out of the interface: they are a machine comparison value and a
  baseline for the future update comparison.

## Verification scope

Rust tests cover path rules, unknown sources, duplicate names, parse failures,
links and limits, the untouched source directory, deterministic digests, folder
discovery and batch reporting, large bodies and headers, the IPC contract, Git
and npx request validation, acquired-tree discovery, partial imports, workspace
cleanup, source-method edits and the v27 fields. Storage tests cover the skill
migrations and preserved rows.

The reviewed public source `vercel-labs/agent-skills` was installed through both
real paths: a pinned Git commit (`063bee9…`) resolved to the same commit in the
record, and the reviewed `skills@1.7.4` npx installer produced
`vercel-composition-patterns`. A real Git fetch with a missing commit reported
`installer_failed` and left the library empty. These checks run as ignored
network tests, not in CI.

A real collection check imported all 13 Skills each from
`~/.skills-manager/skills`, `~/.grok/skills` and `~/.agents/skills` with no
skips, including an 11 MiB skill with 315 files and an 87 KiB `SKILL.md`.
Maintainer acceptance on 2026-10-13 confirmed the native picker and the deletion
dialog passed. The P26 install dialog was exercised in the browser preview at
720×560 and 520×640 with method switching, browser-preview refusal, Escape and
focus return; native installer UI, maintainer visual acceptance and Windows/Linux
native behavior remain release-time checks.
