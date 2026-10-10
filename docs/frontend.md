# Frontend development

The frontend is React + TypeScript, rendered inside Tauri's system WebView.
React describes the interface; Rust handles local system and provider operations.
`src/AGENTS.md` contains the frontend implementation rules.

## Current code and planned layout

- `src/main.tsx` locates the HTML root and mounts React in StrictMode, with
  `LocaleStartup` withholding the App until its language preference is ready.
- `src/App.tsx` composes the desktop shell: the collapsible 200/88 px sidebar
  (brand, Overview, four feature destinations, Settings with the icon-only
  collapse toggle), the relationship home, planned pages, and `SettingsView`
  (mounted while hidden) for the settings area.
- `src/features/settings/SettingsView.tsx` renders the settings title, General/About
  tabs, and panels. General hosts `LanguageSelector` (with `footer={<AppearanceControl />}`)
  composed in `src/main.tsx`. Tab switches and leaving Settings keep preference
  controls mounted so pending saves and input survive.
- `src/features/settings/settingsField.module.css` shares the grouped-card row
  layout (label left, 36 px select right) for language and appearance.
- Sidebar width is fixed at 200/88 px (`--sidebar-width`, `data-collapsed`); only
  the collapse button changes layout.
- `src/components/WindowDragRegion.tsx` and `TitlebarChrome.tsx` (with
  `WindowControls` on Windows/Linux builds) implement drag strips and custom
  window chrome. Never place buttons or fields inside a drag region; an App test
  enforces this. Window IPC lives in `src/lib/desktop/window.ts`.
- `src/components/BrandLogo.tsx` renders one SVG logo: the V tile and rounded
  wordmark when expanded, and the V tile alone when collapsed. The wordmark uses
  a mask from the selected reference without a font dependency. `src/components/Icon.tsx` holds UI icons.
- `src/lib/desktop.ts` checks runtime availability, calls Rust, and validates
  the returned data. Components never import Tauri APIs directly.
- `src/App.css` contains shared design variables and app-scoped scaffold styles.

On macOS the window uses Tauri's overlay title bar (`titleBarStyle: "Overlay"`,
`hiddenTitle`, `trafficLightPosition` in `tauri.conf.json`): the native traffic
lights stay, but there is no separate title-bar row. The top 44 px of the sidebar
and of the content column are drag regions centered on the traffic-light row.
Windows and Linux builds use undecorated windows with in-app minimize, maximize,
and close controls in the content-column title strip (`TitlebarChrome`). macOS
keeps the overlay title bar and native traffic lights in the sidebar.
Vite emits every asset as a file (`assetsInlineLimit: 0`) because the CSP's
`img-src 'self'` blocks the `data:` URIs Vite would otherwise create.

- As functionality grows, use `src/features/providers/`, `src/features/agents/`,
  `src/features/skills/`, and `src/features/mcp/` for implemented features.
  Create these folders when needed, not as empty placeholders.

A component is a function that returns JSX, React's HTML-like UI notation.
Props are its typed inputs. State is data React keeps between renders; updating
it asks React to render again. An effect coordinates with an external system.
It should not be the default place for calculations or button-click actions.

Effects that wait for IPC, such as the language startup read in `LocaleStartup`,
ignore results after cleanup. Cleanup runs when the component unmounts and during
StrictMode's development lifecycle check. Without that guard, a previous request
could update an obsolete view.

## Changes must pass automated checks

```sh
pnpm run check:frontend
```

This runs Prettier, ESLint with zero allowed warnings, TypeScript for both the
UI and Vite configuration, release-tool tests, UI behavior tests, and a production frontend build.
CI and the release workflow run the same command before continuing.

TypeScript enables strict, exact optional properties, and unchecked-index checks.
ESLint uses type-aware strict rules and React Hooks rules. It rejects unsafe
types, unhandled promises, and misplaced Tauri imports. Oxlint runs a dedicated
set of JSX accessibility checks. This avoids holding ESLint on an unsupported
major version because of a JavaScript accessibility plugin's peer constraints.
Use compatible current stable versions for both tools.

ESLint checks TypeScript frontend/config files and Node release scripts.
Formatting is handled by Prettier, so style rules must not conflict with it.

## Editing in Zed

Open the repository root, run `pnpm install --frozen-lockfile`, then use
`task: spawn` → a `vibemate:` task for dev, preview, or `check:frontend`.
Node.js 24.15+ (24 line) or 26+; Zed covers Rust/TypeScript/TSX without extra
extensions.

Shared repo config only:

- `.zed/settings.json` — ESLint language servers, `src-tauri/Cargo.toml` for
  rust-analyzer/Clippy on save, and `pnpm-lock.yaml` as Plain Text (pnpm’s
  multi-document lockfile is not normal YAML).
- `.zed/tasks.json` — same commands as CI (see tasks in repo).
- `package.json` → `prettier.tabWidth: 4` for formatted JS/TS/JSON/CSS; Rust
  uses `rustfmt` via `rust-toolchain.toml`. Themes, format-on-save, and
  keybindings stay in user settings.

Editor diagnostics do not replace `pnpm run check:frontend`.

## Internationalization foundation

P07 provides i18next 26.4.2 and react-i18next 17.0.16 (both MIT), bundled English
and Simplified Chinese resources, and `src/i18n/index.ts`. A resource is a JSON
object whose stable keys identify complete UI messages, grouped by feature.
Provider brands, model IDs, URLs, and user content stay outside translations.

`resolveSystemLocale(systemLanguage)` maps the Chinese language subtag, including
traditional Chinese variants, to `zh-CN`; all other languages resolve to `en`.
`createAppI18n(locale)` returns a promise for a ready, independent translator.
Importing the module neither detects a language nor reads or saves preferences.
Resources are bundled rather than fetched, with English as the fallback.

P08 connects Rust persistence, validated IPC, the startup gate, and the language
selector. Saved English/Chinese choices override the captured system language;
following system and browser preview use `resolveSystemLocale`. The existing App
still has English literals, which P09 will translate. The window title remains
the product name `vibemate`. The ready tree uses an explicit `I18nextProvider`.

The i18next `CustomTypeOptions` declaration constrains translation keys using the
English JSON shape. JSON string values are not literal types, so TypeScript alone
does not enforce interpolation names or required parameters. Tests compare keys
and placeholders across both resources; P09 will extend validation to empty
translations and required plural forms. Both checks and behavior tests use the
existing `test:ui` command, already included in `check:frontend`.

For example, `instance.t("desktop.brandHome", { name: "vibemate" })` translates a
whole label while preserving the supplied product name.
`instance.t("app.areaCount", { count: 1200 })` uses plural rules and the built-in
Intl number formatter. Dates can use `{{date, datetime}}` with explicit timezone
options where needed; standalone values can use `Intl.DateTimeFormat` and
`Intl.NumberFormat` with the resolved locale. React escapes rendered text, so
i18next interpolation escaping is disabled; never render these strings as raw HTML.

Add both translations, accessible names, and error messages with every feature.
Rust will return safe error codes; React translates messages rather than raw
internal errors. Do not use browser storage for language preferences.

Official references: [i18next configuration](https://www.i18next.com/overview/configuration-options),
[TypeScript](https://www.i18next.com/overview/typescript),
[formatting](https://www.i18next.com/translation-function/formatting), and
[React instance provider](https://react.i18next.com/latest/i18nextprovider).

## Language preference storage and IPC

`src-tauri/src/settings.rs` owns the three validated choices: `system`, `zh-CN`,
and `en`. Schema v2 adds a singleton `locale_preference` table to the existing
app-data SQLite database. A missing row means follow system; reading it does not
write a default. A read error or invalid stored value is reported instead of
silently switching to system. A single UPSERT atomically inserts or replaces the
choice; a failed SQLite write leaves the previous committed choice unchanged.

The thin commands in `src-tauri/src/commands.rs` run database work through Tauri's
blocking worker pool. This keeps SQLite's lock wait off the window thread and
async executor. Errors serialize as fixed codes, not SQL, paths, or raw platform
messages. No new plugin or capability permission is required for these app-owned
commands; the existing local-content CSP and capability configuration are unchanged.

`src/lib/desktop/settings.ts` validates Rust responses at runtime:

- `getLocalePreference()` returns either `{ kind: "desktop", preference }` or
  `{ kind: "preview" }`. Preview does not invent a saved preference.
- `saveLocalePreference(preference)` rejects preview and requires the response to
  match the requested choice before reporting success.
- `SettingsRequestError.code` identifies a safe failure for the UI to translate.
  Unknown runtime errors are discarded. A task/transport failure or malformed
  acknowledgment has an unknown saved outcome: reload the preference before
  claiming rollback or displaying a confirmed saved choice.

The Rust tests use temporary databases and cover reopen, v1 migration preservation,
read/write failures, invalid values, SQL constraints, and poisoned locks. Boundary
tests replace Tauri core APIs and cover preview, all choices, malformed responses,
mismatched acknowledgments, and error sanitization. These tests alone do not prove real
WebView IPC. P08 also exercised selection and restart in the macOS Tauri runtime
using a temporary app-data directory; Windows/Linux acceptance remains pending.

## Language selector component

`src/features/settings/LanguageSelector.tsx` receives the validated startup
snapshot and a system language tag. Mount it beneath the initialized
`I18nextProvider`; do not remount the provider or application when language changes.
The snapshot initializes local selection once, so unrelated parent rerenders do
not discard a confirmed choice. The selector does not read startup preferences
or own the document language; those belong to `LocaleStartup`.

It uses a native labeled select and a live status message. Preview is disabled
with an explicit explanation. A pending save also disables the select, while a
synchronous request guard prevents two writes before React renders that state.
Only a confirmed save changes the translator. Explicit English/Chinese choices
win over the system tag; choosing system resolves it through `resolveSystemLocale`.
Sibling forms remain mounted and retain their unsubmitted input.

Known storage/write failures retain the previous choice and allow another attempt.
An unknown saved outcome blocks new writes and offers **Reload saved preference**.
A failed reload stays blocked and can be retried. If persistence succeeded but the
translator failed, the saved choice is retained and the message explicitly says
that only the interface change failed; reloading retries application of that choice.
The component never displays raw exception text or claims an uncertain rollback.

Effect cleanup cannot cancel an IPC write, but late save/read results cannot
change the shared translator after this component unmounts. Tests exercise this
observable behavior rather than only checking that the unmounted DOM is empty.
CSS Modules keep the styles local while using the existing shared color/spacing
variables; the retry action and native select have visible keyboard focus styles.

Component tests use real React and i18next instances with the desktop boundary
replaced. Earlier isolated browser fixtures exercised explicitly labeled synthetic
IPC outcomes, not native persistence. P08 also checked the production browser
preview in both languages at 720×560, and the actual macOS Tauri dropdown with
Tab, Space, arrow keys, and Enter. Native screenshots confirmed visible focus and
bilingual save feedback. Saving English then restarting restored English despite
a Chinese system language; follow-system selection also survived a restart.

## Startup language gate

`LocaleStartup` captures the system language for the window's lifetime. It first
reads the validated preference, then awaits an independent translator, and only
then mounts the selector and App. Startup never saves a default. Until the saved
choice is known, bundled system-language messages explain loading or failure;
reading failure and translator initialization failure have separate messages and
a keyboard-accessible retry. Private exception text is never rendered.

The read effect has its own cleanup flag for each attempt. This is important in
StrictMode: a response from its discarded first effect must not replace the ready
view or turn it into an error. A synchronous retry guard prevents duplicate reads.
Language changes update the existing translator, not the startup state, so the
provider and child inputs are not remounted.

A short layout effect synchronizes the document's `lang` before paint, then
subscribes to i18next's `languageChanged` event. Its cleanup removes that exact
listener. The document element is outside React's managed root; changing this
attribute is an external synchronization step, not direct manipulation of JSX.
The brand-only window title needs no localized title command or extra permission.

Startup tests cover waiting, saved-choice priority, preview, safe failures/retry,
translator initialization failure, obsolete StrictMode responses, late unmounted
reads, and input/HTML-language preservation while a save is pending. Removing the
cleanup protection made both obsolete-response cases fail; restoring it passed.
The i18next factory wrapper retains real initialization except for one explicit
external-service failure; the startup module itself is not mocked.

The macOS runtime was also started with an intentionally invalid preference in
its throwaway database. It showed only the startup error/retry view and left the
invalid value untouched. After explicitly repairing the test fixture, Tab/Enter
retry re-read the preference and opened the App. This was not a production mock
or a change to the user's real app-data. The temporary app-data and test processes
were removed afterward. Native input preservation is covered by DOM tests with
real child inputs; the current App scaffold has no editable form.

## Manual review still matters

Automated checks do not enforce every architecture decision or prove a usable
interface. Review component responsibility, state transitions, runtime data
validation, comments, credentials, styling, and platform assumptions explicitly.
For UI changes, inspect loading/error/empty states, keyboard focus, long labels,
and resizing. For desktop behavior, also run the real Tauri app.

## UI behavior tests

Run `pnpm run test:ui` once, or `pnpm run test:ui src/App.test.tsx` for the
metadata behavior. Vitest shares the Vite configuration and uses jsdom to supply
a DOM inside Node.js. React Testing Library renders the real component. DOM
Testing Library is an explicit peer dependency; all four test packages are MIT.
Vitest 5 supports this project's Vite 8 and React Testing Library supports React 19.
jsdom 30 requires Node 24.15 or newer on the supported Node 24 line.

Tests import Vitest APIs explicitly, so no test globals or relaxed lint rules are
needed. Both existing TypeScript configurations and strict ESLint remain in force.
Only `src/**/*.test.{ts,tsx}` runs in Vitest; release-tool tests retain Node's runner.
Cleanup is registered explicitly because Vitest globals are disabled.

`src/App.test.tsx` replaces only `src/lib/desktop` and verifies loading to ready,
safe failure feedback, browser-only preview, and stale success/failure responses.
The lifecycle cases use StrictMode's effect cleanup and restart: the first request
settles after the second, and must not replace the current visible status.
Temporarily disabling the cleanup guard caused both lifecycle cases to fail;
restoring the original source made all five cases pass.

These DOM tests do not verify Rust, the Tauri WebView, or provider connectivity.
Add behavior tests for new forms and mutations at their desktop boundary as they
are implemented; keep synthetic credentials out of production fixtures.

## Safe IPC example

`getAppInfo()` returns typed metadata or `null` for a browser-only preview. Settings → About reads it on the first visit (I02.d).
It uses `invoke<unknown>()` and checks the actual fields before returning an
`AppInfo`. A TypeScript generic alone cannot check data received at runtime.
More complex commands will need structured success/error results and documented
payload schemas shared with Rust. Do not expose credentials in error messages.

## About metadata panel

`AboutPanel` reads `getAppInfo()` on its first mount, which Settings delays until
About is first selected. It remains mounted across tab and sidebar navigation,
so language switching translates feedback without repeating the request.
A discriminated union represents loading, ready, preview, and error states.
The ready version comes from Rust build metadata; preview never substitutes the
JavaScript package version. Unknown IPC exceptions become bundled error feedback
with a retry button. Each read effect has a cleanup flag so obsolete StrictMode
requests cannot replace current results.

The panel displays the expanded brand, MIT license, and an icon button for the
repository verified against this checkout's Git remote. Activating it opens the
fixed repository in the system browser. Settings tabs use one Tab stop with
Left/Right, Home, and End selecting and focusing native tab buttons. Preference
controls and loaded About metadata remain mounted while hidden.

I02.d verification on macOS: `check:frontend` passed 80 UI tests and eight
release-tool tests, including five About cases and three Settings tab cases.
Temporarily disabling read cleanup failed both obsolete success/failure cases;
the source was restored. ego-browser inspected English and Chinese at 720×560,
including keyboard focus and long preview text without horizontal overflow.
The existing macOS debug binary, placed in a temporary app bundle and connected
to the current Vite frontend, returned version 0.1.0 through real WebView IPC.
No new Rust command or desktop permission was added. Windows/Linux runtime
verification and native error injection were not performed.

## Product branding

`assets/brand/` contains the selected V concept and editable SVG masters.
The app icon is a forest/sage V on an off-white tile. The sidebar references the transparent
`mark.svg` master matching the desktop icon silhouette. `BrandLogo` uses the reference letter shapes and
`currentColor` for light/dark contrast; it no longer uses Tauri's yellow dot.
See `assets/brand/README.md` for the palette and the locked Tauri CLI icon command.

I03 verification on macOS: all 80 UI tests and eight release-tool tests passed
in `check:frontend`; the native locked desktop build and an unsigned local `.app`
bundle succeeded. The bundle's ICNS matches the generated source. The packaged
app was opened and its sidebar inspected. ego-browser checked the new brand at
720×560 in light/dark appearance, expanded/collapsed navigation, loaded image
assets, no horizontal overflow, and keyboard activation of the brand-home button.
Windows/Linux native icon display remains unverified.

## Built-in color themes

Settings → General has an Appearance dropdown and a Color theme radio group.
All five themes are shown together as clickable colored circles.
Forest is the original calm green, Graphite is neutral monochrome, Linen uses
warm paper/clay, Iris has violet accents, and Ocean uses cool blue accents.
Each has both light and dark values in `src/themes.css`. `App.css` resolves those
palette values into the existing semantic colors, so screens and focus feedback
share the same palette. System mode still follows `prefers-color-scheme` in CSS;
no React media-query subscription is needed. The brand icon retains its identity.

`src-tauri/src/appearance.rs` owns validated brightness/palette pairs. Schema v3
adds one constrained singleton table without changing language data. A missing
row returns system/forest without writing a default. One UPSERT saves both
fields atomically. The thin commands use the blocking worker pool and existing
safe settings error codes; no dependency, permission or CSP change was needed.
`src/lib/desktop/appearance.ts` validates runtime values and requires an exact
save acknowledgment. Browser preview reads no saved choice and cannot invoke a
save. It can still try colors locally, with an explicit unsaved-preview message.

`AppearanceControl` reads on mount, disabling changes until the pair is known.
The default CSS is used while that read is pending. A confirmed save updates the
root `data-appearance` and `data-theme`; unknown outcomes block more writes and
offer a reload. Known failed writes retain the confirmed pair. Each read ignores
obsolete effect responses; cleanup restores the document attributes. Language
switching and navigation keep controls and input mounted.

I04/I05 verification on macOS: `check:frontend` passed 95 UI tests and eight
release-tool tests. Rust tests passed 31 cases with the existing real-keychain
smoke test ignored; formatting and all-target Clippy passed. The default locked
release desktop build passed. A temporary app identifier isolated real WebView
selection and restart: dark/Iris was restored, while system language was kept.
The temporary app and its data were removed after verification. All ten theme
palettes were inspected at 720×560, with English and Chinese feedback, system
scheme changes, and native dropdown keyboard selection. Text/summary tokens
exceeded 4.5:1 and focus tokens exceeded 3:1 against the five shared surfaces.
These measurements cover those token pairs, not a comprehensive accessibility
audit. Windows/Linux runtime acceptance remains pending.

The brand button has no hover background or other hover decoration. A visible
keyboard focus outline remains. The i dot is a warm orange accent with extra clearance above its stem.
Expanded/collapsed artwork is exported as
`assets/brand/logo-expanded.svg` and `logo-collapsed.svg`. Its V silhouette was
corrected against the selected reference rather than retaining the earlier
protruding lower turn. The logo is one SVG graphic in both states; its accessible
name and return-home behavior come from the enclosing native button.

The logo's SVG mask uses the actual selected reference letter shapes. A color
matrix makes dark pixels opaque and the pale background transparent; the current
text color is then painted through that mask. The reference dot is cleared and
a separate orange circle is placed higher. This preserves the reference type
without guessing a font family or maintaining approximate hand-drawn letters.
The lettering is raster-derived within the SVG lockup; the V is vector artwork.

I06 replaces the theme dropdown with native radio inputs styled as color circles.
Each circle uses its own palette's accent token from `themes.css`; the palette
values remain defined in one place. Translated accessible names and title hints
identify the colors. A ring and check indicate the selected theme without relying
on color alone. Native radios keep keyboard arrow navigation and single-selection
semantics; pending/uncertain saves disable all choices. The existing persistence,
preview and error/reload workflow is unchanged. Windows high-contrast mode can
show the native radio instead of the styled circle.

I06 was checked with UI behavior tests and `check:frontend`. Per the maintainer's
instruction, visual inspection is manual: check circle spacing, selected/focus
rings, theme colors, and wrapping in the minimum window. No browser screenshots
or automated visual review were performed for this change.

The About panel uses the same full-width settings surface as General. Its
header renders the expanded `BrandLogo`, followed by aligned version, license,
and repository rows. The GitHub Octicons mark is an icon button with a
translated accessible name, pending state, and retryable failure feedback.
Octicons is MIT-licensed; its notice is in `assets/licenses/octicons-MIT.txt`.

`openProjectRepository` is the desktop boundary for explicit activation. Rust
accepts no URL arguments and calls the stable Tauri opener crate's `open_url`
with the fixed repository address. The opener plugin is not registered and no
opener IPC permissions are granted: only the app's fixed command is exposed.
Browser preview opens that same address in a tab with `noopener,noreferrer`.
Visual acceptance is performed by the maintainer.

The repository opener uses the [official Rust `open_url` API](https://docs.rs/tauri-plugin-opener/2.7.0/tauri_plugin_opener/fn.open_url.html). The GitHub mark comes from [GitHub Octicons](https://github.com/primer/octicons/blob/main/icons/mark-github-16.svg).

The built-in palette `notion` (Ink / 纸墨), shown second after Forest, is inspired by
Notion's official public light/dark CSS observed on 2026-10-10:
[theme stylesheet](https://www.notion.so/_assets/77281-c4b3c46f690b55b2.css),
linked from its [login page](https://www.notion.so/login). Its primary text,
canvas, sidebar and border colors reuse those neutral reference values. Hover,
selection and neutral focus/accent values are adapted to vibemate; light
secondary text is darkened from `#7d7a75` to `#686560` for contrast. This is an
inspired palette, not a claim of identical rendering or a Notion integration.

SQLite v4 rebuilds the owned appearance preference table within the existing
migration transaction to extend its theme CHECK constraint. Saved brightness,
existing themes, language and unrelated tables are preserved. Tests upgrade all
five previous choices, save Notion and reopen, reject unknown themes, and verify
that an invalid v3 row rolls back the migration without losing the original row.
Visual acceptance remains with the maintainer.

Notion palette token contrast was checked numerically against canvas, sidebar,
surface, hover and selected backgrounds: light primary/secondary text minima
11.41/4.74, dark primary/secondary minima 11.51/5.66, and focus minima
5.30/8.83. These values do not replace visual acceptance.

Theme saves retain the disabled controls and confirmed-choice behavior but
render no temporary saving message, avoiding a flashing feedback row. Read,
preview, failed-save and unconfirmed-save feedback remain available. The stored
`notion` ID is unchanged so previously saved choices still load as Ink / 纸墨.

The About repository action is a bare 24 px GitHub icon button. With no tile
background, border or oversized button box, its row uses the same 56 px minimum
height as version and license. Hover uses opacity; keyboard focus stays visible.
