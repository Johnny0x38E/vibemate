# Reference: skills-manager marketplace & updates

Source: [xingkongliang/skills-manager](https://github.com/xingkongliang/skills-manager)
(reviewed 2026-10-23). vibemate may reuse ideas below; it must not copy presets,
global/agent/project workspaces, tray auto-updaters, or backup sync.

## In scope for vibemate

### 1. Marketplace browse + install (skills.sh)

skills-manager treats [skills.sh](https://skills.sh) as a catalog, not as an
execution host:

- Leaderboards (`/`, `/trending`, `/hot`) and search return
  `{ source, skillId, name, installs }`.
- `source` is usually GitHub `owner/repo`. Install clones
  `https://github.com/{source}.git`, finds the matching skill directory, and
  copies it into the central library.
- Non-GitHub “site” sources use a separate well-known path; vibemate Phase 1 can
  refuse those until documented.

Worth adopting:

- Catalog UI separate from the hand-entered Git/npx install dialog.
- Install records provenance as marketplace-derived Git (`source_ref` =
  `owner/repo/skill`, resolved clone URL, HEAD commit as `source_revision`).
- Progress phases (`cloning` / `installing` / `done`) without showing raw
  installer stdout.

Do not adopt: HTML scraping fragility as the long-term contract. Prefer a
documented API if skills.sh publishes one; otherwise isolate scraping behind a
single adapter with clear failure codes.

### 2. Update tracking by remote revision (not only content re-fetch)

skills-manager’s check path is cheap and stateful:

| Field                                  | Role                                               |
| -------------------------------------- | -------------------------------------------------- |
| `source_revision`                      | Commit recorded at install / last successful apply |
| `remote_revision`                      | Latest remote HEAD from the last check             |
| `update_status`                        | `up_to_date` / `update_available` / `error` / …    |
| `last_checked_at` / `last_check_error` | Check bookkeeping                                  |

Check algorithm (Git / skillssh):

1. Off the library lock, `ls-remote` (or equivalent) resolves remote HEAD for the
   recorded clone URL + branch.
2. Under the lock, compare `source_revision` to that HEAD.
3. Equal → `up_to_date`; different → `update_available` (store `remote_revision`).
4. Monorepo nuance: if the repo moved but this skill’s subdirectory tree is
   unchanged, advance `source_revision` and stay `up_to_date` (avoid false
   positives). vibemate can defer subdirectory tree-id equality to a later slice
   if Phase 1 only installs one skill folder at a time.

Apply then fetches the new revision, replaces the managed copy, and sets both
revision fields to the new HEAD with `up_to_date`.

Worth adopting relative to current vibemate P29:

- P29 today re-acquires the **recorded** pin and digests. That is good for
  local-vs-same-pin drift and for an explicit apply preview, but it does not
  answer “has upstream moved?” for a floating branch / marketplace install.
- Add a lightweight **check** that stores `remote_revision` + status without
  downloading the tree.
- Keep the existing digest preview as the **apply preview** once the user
  chooses to update to `remote_revision` (or another explicit target).

### 3. Batch check hygiene

- Resolve each unique remote once; fan out status to skills that share it.
- Bound concurrency; never hold the DB/library lock during network I/O.
- Per-remote errors mark only that remote’s skills as `error`.

## Out of scope (explicitly not referenced)

Presets, global/agent/project/linked workspaces, agent CLI bridge for managing
skills, tray-driven auto-apply, multi-device Git backup/merge, custom tool path
editors beyond vibemate’s existing Agent discovery, and website “well-known”
installers until Phase 1 evidence exists.

## Suggested vibemate sequencing

1. **Marketplace install:** delivered — browse/search skills.sh → install into
   the central library with Git provenance at HEAD; reuse existing import/digest
   bounds; no Agent deploy until the user uses the existing deploy panel.
2. **Revision-based update check:** delivered — schema v30 fields + `check` IPC
   that only `ls-remote`s; list/detail show `update_available`. Batch remote
   fan-out and subdirectory tree-id equality remain optional follow-ups.
3. **P30 apply:** delivered — digest preview against Git `HEAD` (or npx
   re-acquire), then keep/replace with managed-copy swap and owned-deployment
   refresh.

npx-recorded skills stay on the P26/P29 path (re-run reviewed installer); they
do not use `ls-remote`.
