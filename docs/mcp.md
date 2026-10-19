# Central MCP definitions

`src/features/mcp/` and `src-tauri/src/mcp/` store MCP server definitions
centrally. Saving, showing or enabling a definition never starts, contacts or
installs a server, and never writes an Agent configuration. Deployment to an
Agent is not implemented; see [`plans/todo.md`](plans/todo.md).

## Evidence and supported fields

Field evidence was checked on 2026-10-10 against [Pi MCP](https://pi.dev/docs/latest/mcp),
[Grok Build MCP](https://docs.x.ai/build/features/mcp-servers) and the
[MCP transports specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).

- Stdio: one executable, an ordered argument array, optional working directory
  and environment variables.
- Streamable HTTP: URL and request headers.
- OAuth, the old SSE transport, dynamic secret commands and imports from other
  tools are not supported.

## Storage and validation

Definitions have random stable IDs and revisions. Lists use ID cursor pages and
each page reports the stored total, so the interface can show a count without
reading every page. Updates reject concurrent changes.

The server identifier is separate from the display name: ASCII letters, digits,
underscores and hyphens, with hyphen/underscore collisions rejected to match Pi's
namespace rule. The identifier is derived on the first save and is not shown in
either form; edits preserve it. HTTP requires HTTPS or loopback HTTP, without
credentials, query or fragment. Header names are case-insensitive; environment
variable names are portable identifiers. Field, argument and value lengths are
bounded. Values are literal: command substitution and environment interpolation
are not supported.

Every environment variable and header value is protected by OS credential
storage, including values users consider harmless. SQLite keeps field names and
opaque references only.

## Credential compensation

- An edit without a replacement keeps the existing reference; omitting a field
  removes it. New or replaced values get fresh references, so a partial write can
  never overwrite a credential that was already there.
- Before a known failed commit, newly written credentials are deleted. A commit
  with an uncertain outcome keeps them and requires a reload before another
  mutation.
- A failed compensation reports an unknown result instead of claiming success.
- Obsolete references are queued in the same transaction and can be retried
  without removing anything still referenced.

## Deletion

Deletion confirms the definition name and affects central storage only.

1. In one immediate transaction, check identity and revision, queue every current
   credential reference and delete the definition. Obsolete references from
   earlier edits stay queued. A rollback preserves metadata and credentials.
2. After a confirmed commit, retry OS credential deletion without reading any
   value. An uncertain commit leaves every OS entry intact and requires a list
   reload.
3. A persistent pending-cleanup indicator is shown even when the library is empty.
   One request retries at most 100 removed identities, excluding every active
   reference.
4. Confirmation, cancellation with focus restoration, duplicate-submit
   prevention, errors and reconciliation all apply; counts and pages refresh after
   deletion.

No Agent registration relationships exist yet, so the list shows **Not
registered** / 「未注册」 and the delete path does not infer references from
discovery or cascade into Agent files. When deployment arrives, delete and
disable must check those references first.

## Interface

The sidebar opens the metadata list lazily. New and edit pages reuse the shared
secondary-page header, edit-form rows and `FieldSelect`. Environment variables and
headers are password rows. Each stdio argument is one literal value without shell
quoting. The form clears typed values on every submit, cancel, transport change
and hide, and clears during the hidden transition so a rapid navigation change
cannot retain them.

Non-secret drafts stay mounted while the panel is hidden, but a successful
sidebar home navigation closes the editor. An unknown write outcome blocks
another mutation until a successful reload. Deleted-credential cleanup stays
visible and retryable after a restart, even with no definitions left.

`src/lib/desktop/mcp.ts` validates exact metadata shapes and cursor ordering,
rejects unexpected secret-bearing response fields, discards raw errors and blocks
mutations in browser preview. Rust is authoritative for field validation.
Commands run blocking storage work off the async executor, reuse the credential
mutation lock and add no capabilities or dependencies.

## Verification scope

Tests cover stdio and HTTP definitions, current and obsolete credentials,
unrelated definitions, missing or stale identities, transaction rollback, restart
and cleanup retry. A recorded isolated Linux Tauri run saved and edited a
no-secret stdio definition across a restart without executing its test command.
Maintainer review accepted the real credential store, native behavior and
appearance on 2026-10-13 and the deletion flow on 2026-10-14. Windows/Linux
runtime behavior remains a release-time check.
