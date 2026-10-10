# Desktop shell design baseline

## Decision and status

P09 is paused; the desktop shell (I02) is largely complete. The maintainer's reference is
`docs/local/程序首页原型草图.png`. That private sketch is the source of the layout;
this shared document records its meaning without publishing the original image.

The maintainer confirmed these choices:

- Remove the separate native titlebar row.
- On macOS, retain real native traffic-light controls inside the sidebar's top area.
- Follow the system appearance; light and dark modes share the same structure.
- Collapse the 200 px expanded sidebar to an approximately 88 px icon rail, not a
  hidden drawer. Width is fixed; only the collapse control toggles layout.
- Remove the independent right-hand page titlebar, including its preview utilities.
- Settings uses top tabs (General and About; About may stay a placeholder until
  `get_app_info` is wired). General lists Language and Appearance as grouped
  rows with compact dropdowns, not icon-cycle controls.

The [interactive preview](desktop-shell-preview.html) is a layout artifact only:
it does not save language preferences, and its Settings UI is not kept in sync
with production (tabs and dropdowns in the app). Business integrations remain
unimplemented.

**Production (I02):** shell, macOS overlay title bar, Windows/Linux undecorated
window controls, and Settings General/About UX are implemented. Settings About now displays build metadata via `get_app_info`, with explicit
preview and retry states. Checklist: [todo.md](todo.md) I02.d.

## Layout

```text
┌──────────────────┬──────────────────────────────────────────────────────┐
│ native controls  │                                                      │
│ icon + vibemate  │               Provider nodes                         │
│                  │                    ↓                                │
│ Provider         │ Skills → configuration application point ← MCPs     │
│ Agent            │                    ↓                                │
│ Skills           │                 Agent nodes                         │
│ MCPs             │                                                      │
│                  │             reserved statistics area                 │
│                  │                                                      │
│ collapse / open  │                                                      │
│ Settings         │                                                      │
└──────────────────┴──────────────────────────────────────────────────────┘
```

- Expanded sidebar: 200 px. Collapsed icon rail: approximately 88 px, including
  enough room for all three native macOS controls. All navigation destinations
  and Settings remain available at the 720×560 minimum size.
- Reserve approximately 44 px above the brand for macOS controls. Their window
  coordinates stay fixed across collapse/expand; never move them beside a title.
- There is no independent right-hand header row, divider, or utility strip.
  The home starts with the relationship section. Other pages may use compact
  headings within their content, not a persistent titlebar.
- Put the icon-only collapse/expand toggle at the right end of the Settings row
  while expanded, and stack it above Settings, centered, in the collapsed rail
  (maintainer decision, 2026-10-10, after trying the traffic-light row). Keep its keyboard focus, accessible name, and expanded state correct. Hidden
  navigation text must not remove the icon buttons' accessible names or tooltips.
- Clicking the brand returns to the relationship home. Keep the four feature
  destinations and bottom-aligned Settings from the sketch.
- Scroll long main content independently of the sidebar. Dialogs, long labels,
  and keyboard focus must remain usable at the minimum size.
- Place language preferences and appearance inside Settings → General. Both use
  the same row pattern (label left, select right) inside one bordered group.
  Appearance is a three-option select (follow system, light, dark), alongside
  Color theme with five paired palettes shown as clickable circles. Both now persist through Rust (I04).
  Language preference still saves through Rust when not in browser preview.
  Do not show redundant success or “current mode” copy when the UI updates
  immediately; keep error and reload messaging for uncertain outcomes.
- Startup owns preference resolution, not page layout; it must not prepend a
  language section to every page.
- Use system fonts, restrained neutral surfaces, fine dividers, and compact
  controls. No marketing hero, feature-card grid, large outer gutters, promotional
  footer, decorative gradients, or new UI framework.

## Relationship home and honest states

Keep providers above the configuration application point, agents below it, and
shared skills/MCP definitions alongside. The central area represents a
configuration relationship, not a running proxy or a newly introduced service.

- Green nodes and links mean confirmed enablement, subject to the architecture's
  requested/applied/effective-state distinctions. Selection uses a separate
  neutral highlight and accessible selected/current semantics.
- Planned or unsupported items stay neutral and explicitly labeled. A design
  sample cannot make Command Code GOAT, DeepSeek, OpenRouter, Pi, or Grok Build
  look connected before their behavior exists.
- Read-only relationship graphics are not clickable-looking controls. Add real
  actions only as their tested behavior is implemented.
- Reserve the lower statistics region without inventing counts, quota, spending,
  activity, or a phase-1 telemetry feature. Show a clear unimplemented state.
- Navigation can open truthful empty/planned screens; it must not imply that a
  configuration was saved, enabled, or applied.

## Window behavior

For macOS, use Tauri 2's overlay titlebar with hidden title and appropriately
positioned native controls. Retain decorations for this option: the installed
Tauri CLI schema requires `titleBarStyle: "Overlay"` and `decorations: true` for
`trafficLightPosition`. Removing the independent titlebar row is not the same
as destroying native window controls.

Windows/Linux need their own borderless-window controls rather than macOS-only
configuration being assumed to work everywhere. Match platform placement and
semantics. Verify each platform before claiming it supported.

Implementation must also provide:

- A genuine drag region in blank chrome, never over navigation or form controls.
  Use a narrow unpainted top drag region rather than reintroducing a visible
  full-width titlebar. It must not cover the original-position native controls.
- Stable native-control coordinates and adequate clearance with the icon rail,
  including startup, navigation changes, and maximized/fullscreen states.
- Close/minimize and platform-appropriate maximize/fullscreen behavior.
- Window controls and a drag region during startup loading and read failure,
  not only after the application has successfully loaded.
- Resize edges, system shadows/corners where available, and correct behavior
  under maximization/fullscreen and differing display scales.
- Accessible names, keyboard focus, and disabled preview behavior.
- Thin typed desktop wrappers and only the permissions those operations need.
  No arbitrary window command or script execution interface.

Official references: [Tauri window customization](https://v2.tauri.app/learn/window-customization/)
and [Tauri configuration](https://v2.tauri.app/reference/config/).
Some retrieved documentation examples still use a Tauri 1 `tauri.windows` root;
implementation must use Tauri 2 `app.windows` and the installed current schema.

## Implementation and acceptance

The sole implementation checklist remains [todo.md](todo.md), under I02.
Implement one reviewable behavior at a time, with file lists split before editing.

The first draft received provisional approval, then the maintainer requested
collapse, removal of the right-hand header, and Settings tabs with grouped
Language/Appearance dropdowns (the static preview may still show older controls).
Review the updated preview expanded/collapsed in both appearances at 720×560,
including invariant native-control coordinates. After
visual approval, implement the shell, then move the language selector into Settings,
then implement the native window chrome. Keep the existing startup, save/reload,
and input-preservation behavior intact. Add both translations with each new view.

Browser checks prove layout and DOM behavior, not native dragging or window
controls. Acceptance requires actual Tauri runtime checks and screenshots.
Do not claim Windows/Linux native behavior from local macOS checks.

## Product identity

The maintainer selected logo concept 1 on 2026-10-10: a rounded V symbol in
forest/sage green and a rounded lowercase wordmark. Production SVG masters and
the selected reference are in `assets/brand/`. The V keeps its forest/sage colors in the sidebar and desktop packaging;
the app tile is off-white and the two strokes retain a separating gap.
The wordmark follows light/dark text color. This replaces the default Tauri
cyan/yellow ring icon. I03 status is tracked only in `todo.md`.

## Theme and brand refinements

Settings General includes Forest, Graphite, Linen, Iris, and Ocean, each with
light/dark colors independent of the brightness choice. All five theme accent colors appear as native radio circles with a selected
ring and check; the theme control no longer uses a dropdown. No new settings page or theme framework
is introduced. I04 stores the paired choice through Rust and permits explicitly
unsaved browser preview.

I05 combines icon and lettering into one SVG logo while expanded, and uses its
symbol-only version in the collapsed rail. The brand has no hover decoration;
keyboard focus stays visible. The selected V reference governs both states and
all generated desktop icons. The lower turn was corrected after maintainer
feedback about the earlier redraw's protruding stroke.

Final brand refinements follow the selected reference's actual lettering, with
a warm orange i dot raised for more clearance. Expanded branding uses one SVG
lockup with the transparent two-color V; the collapsed rail uses the V alone.
