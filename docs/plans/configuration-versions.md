# Independent configuration versions

## Behavior

The Agent history list shows saved configurations, not restore and compensation
operations. Each selectable row identifies the original apply's written result.
Selecting A after A → B → C restores A's written bytes, not the bytes before A.
The actual current version is marked and pinned first, including when its
original apply falls outside the chronological seek page.

## Storage and lifecycle

Schema v25 separates `config_version` and `config_version_file` from the recovery
journal. Every apply stores a complete private copy of each planned file,
including Pi files whose content did not change. These are full copies, not a
chain of differences and not references to another operation's backups.

1. Record ownership of all version copies and persist their bytes before writing
   any Agent file. A copy or metadata failure prevents target writes.
2. Publish the version only after the complete apply succeeds. Interrupted
   preparation can publish after restart only when the successful original
   operation and all private copies can be verified.
3. Preview, full-text reads and confirmation use only that version's copies.
   Recheck live digests and copy integrity under the existing mutation lock.
   Missing or damaged version bytes do not fall back to another journal record.
4. Delete the selected version's complete copies through the durable deletion
   journal, then tombstone its metadata. Keep Agent files, other versions,
   credentials, alias ownership and the audit/recovery journal unchanged.

Completed hidden restore batches do not prevent version deletion. Active and
unresolved operations still do, including when retrying a previously failed
delete. A failed delete remains visible and retryable; deletion is not secure
erasure of all historical copies because recovery logs retain their backups.

## Compatibility

Legacy applies initially have `legacy` metadata. Under the shared configuration
lock, startup or a later write locates exact, digest-verified result bytes in the
live files or old recovery snapshots and copies them into exclusively owned
version directories. This is a one-time conversion, not a continuing dependency.
Interrupted `migrating` copies resume without replacing an existing private copy.
Unrecoverable result bytes become explicitly unavailable; model names or digests
alone are never used to recreate them. Tombstoned versions do not reappear.

Existing internal pre-entry recovery APIs and the full audit-history API remain.
The Agent UI boundary consistently uses written-version semantics for preview,
confirmation and sanitized text. Old unfinished delete requests preserve their
recorded scope and deletion intent rather than silently changing their targets.

## Interface

The page returns apply-only chronological entries plus a separately loaded
`currentEntry`. The frontend validates that its identity and written file digests
agree with the live marker, pins it once, and leaves the seek cursor unchanged.
Duplicate byte-identical versions resolve deterministically to the latest
successful apply. Every file in a multi-file configuration must match.

Expanded rows show field differences, necessary unavailability/recovery errors,
one key/session warning and **Use this configuration**. They do not repeat
backup descriptions, internal Provider references, timestamps or operation
summaries. Interrupted-write inspection remains separate from selection.

## Verification

Regression coverage includes all three Agents' independent versions, Pi's
unchanged second file, switching after intermediate/previously selected version
deletion, deleting the current version without writing Agent files, stale or
deleted selections, damaged copies, failed snapshot recording, durable deletion
retry, interrupted migration and schema-v24 upgrades.

Run frontend checks, Rust formatting/Clippy/tests and the locked desktop build.
Inspect both languages, keyboard interaction, long content and the minimum
viewport with a temporary browser fixture. Use a separate Tauri application
identifier and temporary Agent roots for native checks; do not use real keys or
real Agent configuration. macOS verification does not establish Windows/Linux
behavior. Maintainer acceptance and platform checks remain in
[`todo.md`](todo.md).
