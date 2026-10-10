# Base UI Select migration

## Scope and decision

Replace only the interaction inside `src/components/FieldSelect.tsx` with
Base UI Select. Keep the public props, provider icons, existing CSS Modules,
36 px trigger, six theme palettes and all Rust-owned business behavior.
The maintainer approved this scope after discussing headless component libraries.
Task status is recorded only in [todo.md](todo.md).

Use the stable `@base-ui/react` 1.9.0 release, not a prerelease. npm metadata
identifies the official `mui/base-ui` repository and MIT license, with React
17/18/19 peer support. Date libraries are optional peers and are not needed.
Keep pnpm's release-age policy and frozen-lockfile workflow unchanged.
Add stable MIT-licensed `@testing-library/user-event` 14.6.7 as a development-only
dependency: standalone synthetic clicks do not represent the pointer press and
animation-frame focus timing used by Select. Tests must await complete interactions.

## Implementation order

1. Add the pinned dependency and lockfile. Verify its installed license and
   Select type declarations against the current official documentation.
2. Replace the manual popup, key handlers and outside-click listener with Select.
   Preserve controlled values: the parent commits a saved setting, not the widget.
   Preserve native disabled controls and focusable blocked provider fields.
3. Update shared test helpers to locate portaled options through the trigger's
   accessible popup relationship, not its DOM parent. Add real focus/keyboard,
   outside dismissal, icon, label and lock-transition regression coverage.
4. Run the complete frontend check and applicable native build checks. Record
   limitations separately from automated results; do not claim a DOM test proves
   native WebView or screen-reader behavior.

## Popup and state boundaries

The popup renders through a portal to avoid ancestor `overflow: hidden` clipping.
A portal moves DOM output outside the field while keeping React composition.
Theme tokens live on the document root, so portaled content inherits them.
Positioning should remain below the trigger by default, with viewport collision
handling and bounded scrolling. The selected item must not overlap the trigger.
A named `--layer-popover` token in `src/App.css` controls its stacking level; the
positioner's hidden-anchor attribute hides a popup whose source page is hidden.

Base UI owns focus, keyboard navigation, typeahead and dismissal. FieldSelect
owns its existing public value callback and a small open-state boundary so a
field becoming disabled or blocked cannot leave an interactive menu behind.
No new translation strings, IPC commands, capabilities or network calls are needed.

## Verification boundaries

Follow the repository's manual appearance-review policy; do not run visual browser
or screenshot checks without a separate request. Desktop WebView keyboard,
minimum-window popup placement, theme appearance and assistive technology checks
remain manual acceptance unless actually exercised. Dependencies do not remove
our responsibility for accessible labels and business-state tests.

## Sources

- [Base UI Select](https://base-ui.com/react/components/select)
- [Base UI styling](https://base-ui.com/react/handbook/styling)
- [Official repository](https://github.com/mui/base-ui)
- npm metadata and installed type declarations for `@base-ui/react@1.9.0`.
