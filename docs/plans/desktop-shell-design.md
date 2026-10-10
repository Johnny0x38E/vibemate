# Desktop shell design baseline

## Decision and status

The maintainer closed the current basic appearance iteration on 2026-10-10.
The sole task checklist is [todo.md](todo.md): I02–I14 record the shell, branding,
settings, themes and Dock refinements. P09 remains the next unfinished task.
Provider and agent behavior remains unimplemented.

The private sketch `docs/local/程序首页原型草图.png` supplied the original layout.
The [interactive preview](desktop-shell-preview.html) is a historical layout
artifact; its controls and persistence do not represent the current app.
Current implementation details are in [frontend.md](../frontend.md).

## Layout

The sidebar has a fixed 200 px expanded width and an 88 px collapsed rail.
All navigation destinations and Settings remain available at the 720×560
minimum window size. Collapse is controlled by the toggle; dragging does not
change sidebar width.

- Reserve the top 44 px for native macOS traffic lights and blank drag areas.
  Their coordinates stay fixed across sidebar collapse and expansion.
- Use the expanded V/wordmark logo in the sidebar, and the V symbol in the
  collapsed rail. Clicking the brand returns home. The brand has no hover
  decoration; keyboard focus remains visible.
- There is no separate right-hand titlebar or utility strip. Pages place their
  headings inside the main content, which scrolls independently of the sidebar.
- Settings stays at the bottom. Its collapse control sits beside it when
  expanded and above it in the collapsed rail. Keep accessible names and
  tooltips when navigation text is hidden.
- Use system fonts, shared theme tokens, fine dividers and compact controls.
  Preserve long labels, keyboard focus and minimum-window usability.

## Settings

General and About use top tabs and the same full content width. Both remain
mounted while hidden, preserving pending requests, selected preferences and
loaded metadata across navigation.

General places Language, Appearance and Color theme inside one grouped surface.
Language and brightness use dropdowns. Theme choices are native radio circles
with a selected ring and check, in this order:

| Position | English  | 简体中文 | Persisted ID |
| -------- | -------- | -------- | ------------ |
| 1        | Forest   | 森林     | `forest`     |
| 2        | Ink      | 纸墨     | `notion`     |
| 3        | Graphite | 石墨     | `graphite`   |
| 4        | Linen    | 亚麻     | `linen`      |
| 5        | Iris     | 鸢尾     | `iris`       |
| 6        | Ocean    | 海湾     | `ocean`      |

Each theme has light and dark palettes, with brightness independent of the
selected theme. Ink uses white/warm-gray and charcoal neutrals inspired by
Notion. Rust saves both choices together in schema v4. Controls are disabled
while saving; no temporary saving or success text flashes below them. Read,
preview, error and uncertain-save/reload feedback remains available.

About has the expanded product logo and description above version, MIT license
and repository rows. The three rows share a 56 px minimum height. The repository
entry is a bare 24 px GitHub icon with no tile background or border. It opens
only the fixed project repository and retains keyboard focus, pending and
retryable error feedback. Preview never invents a desktop version.

## Product identity

The selected V uses forest `#324e40` and sage `#a8b8a7`; its lower turn follows
the selected reference without an extended tail. The expanded wordmark uses
the reference's letter shapes, with a raised orange `#db915b` i dot. The original
i dot is removed from the lettering mask. Wordmark text follows the theme;
the V retains its brand colors.

The app icon uses an off-white `#f7f8f7` tile. The separate macOS master scales
artwork to 85%, leaving a 408 px tile on its 512 px canvas. Continuous corner
transitions span 160 px before scaling. The V has a slight downward optical
offset of 6.8 px on the canvas. Other platforms retain their original master.
See [brand assets](../../assets/brand/README.md) for files and regeneration.

## Relationship home and honest states

Providers sit above the configuration application point, agents below it, with
shared skills/MCP definitions alongside. This is a configuration relationship,
not a running proxy. Planned screens remain labeled as planned. No diagram,
node color or statistic may imply a connected integration, enabled agent,
spending, quota or telemetry behavior before that behavior exists.

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

## Verification and future changes

Appearance checks belong to the maintainer. Do not run browser or screenshot
visual checks unless requested. Automated formatting, lint, types, behavior
tests and applicable builds remain required. The latest recorded checks and
remaining native verification are in [frontend.md](../frontend.md).

The maintainer's appearance confirmation does not establish Windows/Linux
runtime behavior, native browser launching or a comprehensive accessibility
audit. Keep those limitations separate from completed visual refinements.
