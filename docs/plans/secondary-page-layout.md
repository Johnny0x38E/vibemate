# Shared secondary-page layout

## Design

Provider create/edit, MCP create/edit and Agent details use a left-aligned title,
a bounded content area and a fixed bottom action bar. Back sits on the left;
Save, when present, sits on the right. All module and secondary pages use the
shared 16 px `--page-content-gap` between content and the preceding title or
tab strip. Scroll wrappers add no extra top padding or horizontal inset; card
edges match titles and tabs without forcing field widths to match. A fixed
16 px gap outside scrolling content keeps moving rows away from the footer
divider.

The footer action row shares the sidebar’s 44 px row height and 12 px bottom
inset. The 36 px Back/Save controls are vertically centered in that row, so their
centers align with the sidebar Settings control in both sidebar widths and at
the narrow-window breakpoint.

Provider Models retains its search/fetch toolbar, fixed list title and list-only
scroll. Its selection Save joins Back in the bottom footer. Local actions such
as adding MCP arguments or rediscovering an Agent path stay beside their fields.

## Boundaries

`SecondaryPageFooter` owns layout and the blocked Back button. Feature components
retain submission, retry, dirty-draft prompts, secret clearing and focus behavior.
Read-only Agent details reuse `ProviderPage` with one scrollable content wrapper.
Main-column scroll overrides apply only while a visible secondary page is open.
List pages, Settings, Overview and Skills are not redesigned.

## Verification

`check:frontend` passed: formatting, zero-warning lint, translation checks,
539 UI/boundary tests, eight release tests, 14 translation-checker tests,
TypeScript and the production frontend build. Vite retains its existing
single-chunk size advisory. Browser/native visual verification is left to the
maintainer; no browser or desktop runtime validation was run for this iteration.
See [sidebar-module-home.md](sidebar-module-home.md) for sidebar navigation and
[frontend.md](../frontend.md#read-only-agent-discovery) for Agent card geometry.
