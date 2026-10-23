# Skill deployment recovery (P34, non-MCP slice)

Task status stays in [todo.md](todo.md). This page records the stopping point
and the reviewed design; implementation has not started.

## Scope

Agent configuration recovery is delivered in H1. This slice adds unfinished
**Skill deploy/disable** operations to a recovery entry. MCP operations stay with
P33/feat-mcp; the same journal pattern can cover them later.

The recovery entry discovers operations and explains their state. It never
replays a deployment on startup and never rewrites an Agent file that vibemate
does not own.

## Journal

Add schema v33 (v31 is Claude Code Skill deployment and v32 is Pi MCP):

```sql
CREATE TABLE skill_deploy_operation (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('apply','disable')),
    state TEXT NOT NULL CHECK(state IN ('prepared','unknown')),
    skill_id TEXT NOT NULL,
    agent_id TEXT NOT NULL CHECK(agent_id IN ('pi','grokBuild','claudeCode')),
    config_root TEXT NOT NULL,
    target_name TEXT NOT NULL,
    target_path TEXT NOT NULL,
    mode TEXT CHECK(mode IS NULL OR mode IN ('symlink','copy')),
    skill_revision INTEGER NOT NULL,
    content_digest TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
```

- Insert `prepared` before the first filesystem change.
- Delete the row after the ownership row and filesystem agree.
- Mark `unknown` only when a write may have changed the target and rollback
  could not be proven.
- A successful operation with a failed journal delete leaves a row; recovery
  sees it as already complete and offers cleanup.
- `delete_skill` must also refuse while a journal row names the skill.

## Classification

Read the target and ownership row at list time:

| Kind    | Target             | Ownership row | State                |
| ------- | ------------------ | ------------- | -------------------- |
| apply   | absent             | present       | `needs_record_clear` |
| apply   | absent             | absent        | `not_started`        |
| apply   | matches managed    | present       | `completed`          |
| apply   | matches managed    | absent        | `needs_record`       |
| apply   | foreign/unreadable | any           | `conflict`           |
| disable | absent             | present       | `needs_record_clear` |
| disable | absent             | absent        | `completed`          |
| disable | matches managed    | present       | `not_started`        |
| disable | matches managed    | absent        | `conflict`           |
| disable | foreign/unreadable | any           | `conflict`           |

"Matches managed" means a symlink to the private library copy, or a copied tree
whose digest equals the recorded `content_digest`. The recorded digest is used so
a later central update does not make an older target look foreign.

## Actions

- **Finish**:
    - `needs_record`: insert the missing ownership row from the journal, then
      delete the journal row.
    - `needs_record_clear`: remove the stale ownership row, then delete the journal
      row.
    - `completed` / `not_started`: delete the journal row only.
    - `conflict`: refuse with `target_conflict`.
- **Dismiss**: delete the journal row only. Allowed for `completed`,
  `not_started` and `conflict`; refused for `needs_record` and
  `needs_record_clear` with a `recovery_conflict` code so a real inconsistency
  is not hidden.

Every action re-reads and re-classifies the row before changing anything. A
changed classification returns `target_changed` and the UI reloads.

## UI

A `SkillRecoveryPanel` above the Skill library list:

- loads unfinished rows on Skills mount;
- shows kind, skill, Agent, target path, journal state and classification;
- offers Finish/Dismiss according to the rules above;
- keeps errors local to the panel and reloads after an action;
- stays hidden when no unfinished row exists.

This is a Skill-specific entry. The unified Agent/MCP recovery entry remains
part of the MCP follow-up, so P34 is not complete until that entry and its
acceptance evidence exist.

## Tests

- Migrate v32 to v33; preserve unrelated rows.
- Crash after filesystem write with no ownership row -> `needs_record`; Finish
  records ownership and clears the journal.
- Crash after target removal with a stale ownership row ->
  `needs_record_clear`; Finish clears both.
- Foreign or unreadable target -> `conflict`; Finish refuses, Dismiss clears the
  journal without touching files.
- Repeated Finish is idempotent or reports `not_found`.
- `delete_skill` refuses while a journal row exists.
- Frontend panel tests for Finish, Dismiss, error and reload behavior.

## Stopping point (2026-10-23)

The design above is recorded. A partial journal table/type draft was removed so
the tree stays compiling. P35's non-MCP deletion constraints are the last
completed change. Next steps: implement the journal in `skills/deploy`, then
the recovery command and panel.
