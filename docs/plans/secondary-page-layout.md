# Shared secondary-page layout

## Design

Provider create/edit, MCP create/edit and Agent details use a left-aligned
header row and a bounded content area. **Back** is an icon-only control on the
left of the header (accessible name + tooltip name the destination list).
**Save**, when present, sits on the right of the same header row. Nested editors
publish Save through `SecondaryPageActions` so the active tab owns the trailing
slot. All module and secondary pages use the shared 16 px `--page-content-gap`
between content and the preceding title or tab strip. Scroll wrappers add no
extra top padding or horizontal inset; card edges match titles and tabs without
forcing field widths to match.

The header row matches the 36 px `PageModuleHeader` control height. Windows and
Linux window controls stay in the separate 44 px `TitlebarChrome` strip above
the content column; page actions never enter that chrome.

Provider Models retains its search/fetch toolbar and list-only scroll. Selection
Save joins the shared header. Local actions such as adding MCP arguments or
rediscovering an Agent path stay beside their fields.

## Boundaries

`ProviderPage` owns the header Back control and the trailing-actions host.
Feature components retain submission, retry, dirty-draft prompts, secret
clearing and focus behavior, and publish Save via `SecondaryPageActions`.
Read-only Agent details reuse `ProviderPage` with one scrollable content wrapper
and no trailing actions. Main-column scroll overrides apply only while a visible
secondary page is open. List pages, Settings, Overview and Skills are not
redesigned by this layout.

## Verification

Automated checks and maintainer visual review are recorded with the change that
lands this layout. See [sidebar-module-home.md](sidebar-module-home.md) for
sidebar navigation and [frontend.md](../frontend.md) for feature notes.
