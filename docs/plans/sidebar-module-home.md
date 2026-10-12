# Sidebar module home navigation

## Behavior

- Sidebar activation opens Providers/MCP/Agents lists or Settings General, even
  when the clicked module is already visible.
- Preserve module mounts and caches; do not reset by changing React keys.
- Modules own synchronous home-navigation guards through a narrow
  `ModuleNavigationHandle.openHome(): boolean` ref handle.
- Re-clicking active Providers respects its model-draft confirmation and busy
  guards. Provider API writes, MCP pending/uncertain outcomes and Agent scans
  block home navigation while their guards remain active.
- This is a destination-module request, not a global guard on leaving the current
  module. Hidden model views invalidate reads and cancel fetching; this does not
  promise that every model draft or manual form survives a module switch.
- Successful Provider/MCP home navigation abandons unsubmitted editors, following
  existing Back behavior. Agent manual paths and Settings controls stay in memory.
- MCP home navigation invalidates late edit reads.

## Verification

Implemented and covered by bilingual App navigation tests, provider pending-save
and model-draft cancellation tests, and MCP pending/uncertain/late-read tests.
`pnpm run check:frontend` passes; per-run test counts are in Git/CI. Browser and
native navigation acceptance is left to the maintainer, and no native writes are
claimed.
