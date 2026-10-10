# Central MCP definitions

P31 is moved ahead of Agent deployment at the maintainer's request. It depends
on private storage and credentials, not Agent detection. P32/P33 still need their
Agent adapters and safe config preview/write/rollback foundations.

## Evidence and supported fields

Official field evidence reviewed on 2026-10-10 against [Pi MCP](https://pi.dev/docs/latest/mcp),
[Grok Build MCP](https://docs.x.ai/build/features/mcp-servers), and the
[MCP transports specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).
Stdio stores one executable, an ordered argument array, optional cwd and env.
Streamable HTTP stores URL and headers. No server is executed, contacted or
installed by definition reads, saves or enablement changes. No Agent config is
written. OAuth, old SSE, dynamic secret commands and imports are deferred.

## Storage and editing

Definitions have random stable IDs and revisions. Lists use ID cursor pages;
updates reject concurrent changes. A separate server identifier uses ASCII
letters, digits, underscores and hyphens; hyphen/underscore collisions are
rejected to match Pi's namespace rule. Display names remain bilingual user text.
HTTP uses HTTPS or loopback HTTP, without credentials, query or fragment. Header
names are case-insensitive; env names are portable identifiers. Bounded field,
argument and value lengths avoid unlimited payloads. Values are literal: command
substitution and environment interpolation in env/header values are not supported.

Every env/header value is protected by OS credential storage, including values
that users consider non-sensitive. SQLite keeps field names and opaque references.
Reads never open the credential store and never return secret values. An edit
with no replacement retains a field's existing reference; omission removes it.
New or replaced values get fresh references, so partial writes never overwrite
existing credentials. Before a known failed database commit, newly written
credentials are deleted. A commit with uncertain outcome retains them and
requires reload. Failed compensation reports an unknown result. Obsolete
references are recorded transactionally and deletion can be retried without
removing any currently referenced entry. Process-crash orphan discovery remains
part of the later operation-recovery work; this feature does not claim a
cross-resource transaction.

The form uses password inputs, clears secret values after every submit and hide,
and cannot save again after an unknown outcome until the list or definition is
reloaded. Enablement labels describe the central definition, not a connection or
an applied Agent configuration. Native and visual acceptance remain distinct
from simulated behavior tests. Task status lives only in [todo.md](todo.md).
