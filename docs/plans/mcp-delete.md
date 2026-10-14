# Central MCP deletion

## Scope

Remove one reviewed central definition after a bilingual, keyboard-accessible
confirmation. Check its revision inside the deletion transaction. No MCP server
is executed or contacted and no Agent configuration is modified.

The current schema contains no Agent-to-MCP registration relationships; list rows
already state that registration is not implemented. P35 reference checks remain
required when deployment relationships are introduced. This change does not infer
references from Agent discovery or cascade into Agent files.

## Implementation slice

1. In one immediate transaction, check identity/revision, queue all current
   credential references and delete the definition. Existing obsolete references
   stay queued. Rollback preserves metadata and current credentials.
2. After a confirmed commit, retry OS deletion without reading credential values.
   An uncertain commit leaves all OS entries intact and requires a list reload.
3. Show a persistent pending-cleanup indicator even for an empty library. Retry
   at most 100 removed identities per request, excluding all active references.
4. Add confirmation, cancellation/focus restoration, duplicate-submit prevention,
   errors and reconciliation. Refresh counts and cursor pages after deletion.

## Verification

Cover stdio/HTTP, current and obsolete credentials, unrelated definitions,
missing/stale identities, transaction rollback, restart and cleanup retry. Run
frontend checks, Rust formatting/Clippy/tests and the macOS desktop build.
Browser samples only establish UI behavior; native IPC, real credential storage
and Windows/Linux acceptance are separate checks.

The initial checkout was clean and matched fetched `origin/main` at `32fd387`.
Automated checks passed on macOS: frontend (710 UI/boundary tests), Rust formatting,
Clippy with warnings denied, Rust tests and the locked no-bundle desktop build.
A browser sample checked Chinese confirmation at 720 × 560 with long text and
initial focus on Cancel. Native mutations, real credential deletion, English
visual acceptance and other platforms remain for maintainer verification.
