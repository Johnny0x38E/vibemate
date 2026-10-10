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
- `src/components/BrandLogo.tsx` renders one SVG logo: the transparent V and rounded
  wordmark when expanded, and the V symbol alone when collapsed. The wordmark uses
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
UI and Vite configuration, release-tool tests, the translation-resource check and
its tests, UI behavior tests, and a production frontend build.
CI and the release workflow run the same command before continuing.

TypeScript enables strict, exact optional properties, and unchecked-index checks.
ESLint uses type-aware strict rules and React Hooks rules. It rejects unsafe
types, unhandled promises, and misplaced Tauri imports. Oxlint runs a dedicated
set of JSX accessibility checks. This avoids holding ESLint on an unsupported
major version because of a JavaScript accessibility plugin's peer constraints.
Use compatible current stable versions for both tools.

ESLint checks TypeScript frontend/config files and the Node release and translation scripts.
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
following system and browser preview use `resolveSystemLocale`. P09 moved every
existing navigation label, heading, state, hint, error and accessible name onto
grouped keys in both languages. Chinese uses one term set everywhere: 服务商,
Agent, 技能 and MCP 服务器. Brand names (`vibemate`, providers and agents), the
`MIT` license name, model IDs and URLs stay untranslated. The window title remains
the product name `vibemate`. The ready tree uses an explicit `I18nextProvider`.

The i18next `CustomTypeOptions` declaration constrains translation keys using the
English JSON shape, and `src/i18n/index.test.ts` asserts at the type level that
the Chinese JSON has the same shape, so `pnpm run typecheck` fails on a missing
or extra key. JSON string values are not literal types, so TypeScript alone does
not check values, interpolation names or plural forms; the resource check below does.

For example, `instance.t("desktop.brandHome", { name: "vibemate" })` translates a
whole label while preserving the supplied product name. A plural message such as
`itemCount_one`/`itemCount_other` with `{{count, number}}`, called as
`t("feature.itemCount", { count: 1200 })`, uses plural rules and the built-in
Intl number formatter. The current UI has no plural message, so the i18n tests
add temporary fixture messages to one translator instead of keeping an unused key. Dates can use `{{date, datetime}}` with explicit timezone
options where needed; standalone values can use `Intl.DateTimeFormat` and
`Intl.NumberFormat` with the resolved locale. React escapes rendered text, so
i18next interpolation escaping is disabled; never render these strings as raw HTML.

Add both translations, accessible names, and error messages with every feature.
Rust returns safe error codes; React translates messages rather than raw
internal errors. Do not use browser storage for language preferences.

### Translation resource check

```sh
pnpm run check:i18n   # validate src/locales/*.json
pnpm run test:i18n    # the checker's own tests, with passing and failing fixtures
```

`scripts/i18n-resources.mjs` uses only Node built-ins and runs in
`check:frontend` before the UI tests. It compares every locale with English and
prints each problem with its key path, then exits with status 1. It reports:

- keys missing from, or not present in, the English reference;
- empty or whitespace-only values, non-string values and empty groups;
- `{{ }}` markers that are not closed or name no parameter (`{{}}`), and
  interpolation names that differ from English. The unescaped form `{{- name}}`
  passes the same `name` parameter as `{{name}}`. Plural forms of one key are
  compared as one message, so an English singular may omit `{{count}}`;
- plural keys without every form that `Intl.PluralRules` selects for that locale
  (English `one`/`other`, Chinese `other`), or with a form no bundled locale uses;
- ordinal plural keys (`_ordinal_*`), which fail with "ordinal plurals are not
  supported yet" until the check learns ordinal rules.

The suffixes `_zero`, `_one`, `_two`, `_few`, `_many` and `_other` are reserved
for i18next cardinal plurals; do not end an ordinary key with them. `_zero` is
always optional: i18next uses `key_zero` for a count of 0 in every language, even
though `Intl.PluralRules` never selects "zero" for English or Chinese.
Chinese may keep an `_one` form because English needs it and the TypeScript
shape check requires identical keys; Chinese never selects it.

The check does not cover single-brace text such as `{name}` (i18next ignores it,
so it renders literally), conflicts between a dotted key name and a nested group
(`"a.b"` next to `"a": { "b": … }`), or whether a nested `$t(other.key)`
reference points to an existing key. Review those by hand if they are introduced.

There is deliberately no unused-key detection. Components build some keys from
typed values, such as ``t(`desktop.nav.${page}`)`` and
``t(`settings.theme.names.${palette}`)``, so a text search would report those
messages as unused or need a list of exceptions. Remove keys when the UI that used
them is removed, as P09 did for the former landing page's `app.*` group.

`LocaleStartup` does not use the translator: it must explain loading and failure
before any translator exists, so it reads `settings.startup` directly from the
bundled JSON for the system language. Those messages therefore have no English
fallback at runtime; the empty-value check is what keeps them from rendering blank.

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
These messages come from the bundled JSON rather than the translator; see
"Translation resource check" above.

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
desktop-shell and bilingual behavior. Vitest shares the Vite configuration and uses jsdom to supply
a DOM inside Node.js. React Testing Library renders the real component. DOM
Testing Library is an explicit peer dependency; all four test packages are MIT.
Vitest 5 supports this project's Vite 8 and React Testing Library supports React 19.
jsdom 30 requires Node 24.15 or newer on the supported Node 24 line.

Tests import Vitest APIs explicitly, so no test globals or relaxed lint rules are
needed. Both existing TypeScript configurations and strict ESLint remain in force.
Only `src/**/*.test.{ts,tsx}` runs in Vitest; release-tool tests retain Node's runner.
Cleanup is registered explicitly because Vitest globals are disabled.

`src/App.test.tsx` replaces only the desktop boundary modules (`src/lib/desktop`
and its settings/appearance wrappers) and composes `LocaleStartup`, the real
selector and App as `main.tsx` does. Its 18 cases cover navigation, collapse and
drag regions, and P09's bilingual behavior: a saved Chinese choice over an English
system, following a Chinese system, switching through the real selector while
keeping the page, settings tab, collapsed sidebar, unsaved input and loaded About
metadata, English fallback for an untranslated Chinese entry, and startup-read,
About-metadata and language-save failures with retry in both languages. A helper
checks visible text, `aria-label` and `title` for unresolved keys and private
diagnostics. Temporarily rendering a raw key in About's error, or allowing empty
translations, made the matching cases fail.

Component-level metadata states (loading, preview, obsolete StrictMode responses)
are covered in `AboutPanel.test.tsx`; IPC validation is covered by
`src/lib/desktop/*.test.ts`.

These DOM tests do not verify Rust, the Tauri WebView, or provider connectivity.
Add behavior tests for new forms and mutations at their desktop boundary as they
are implemented; keep synthetic credentials out of production fixtures.

## Safe IPC example

`getAppInfo()` returns typed metadata or `null` for a browser-only preview. Settings → About reads it on the first visit (I02.d).
It uses `invoke<unknown>()` and checks the actual fields before returning an
`AppInfo`. A TypeScript generic alone cannot check data received at runtime.

Failures from both `getAppInfo()` and `openProjectRepository()` reject with
`MetadataRequestError`, whose `code` is one of:

| Code               | Meaning                                                      |
| ------------------ | ------------------------------------------------------------ |
| `invalid_response` | The command answered with data of the wrong shape.           |
| `operation_failed` | IPC itself failed; `get_app_info` cannot fail in Rust.       |
| `open_failed`      | Rust's repository command could not open the system browser. |

The original runtime error is discarded rather than wrapped, so paths and OS
messages cannot reach the UI. About shows one translated message per action.

Each desktop boundary module defines its own `XRequestError` class and code union
type: `SettingsRequestError` with `SettingsErrorCode` in `settings.ts`,
`AppearanceRequestError` in `appearance.ts`, and `MetadataRequestError` with
`MetadataErrorCode` in `desktop.ts`. Appearance reuses `SettingsErrorCode`
because Rust returns the same `SettingsError` for both preference commands.
Keeping one class per module lets UI code check `instanceof` for the request it
made and keeps each code list limited to what that request can actually return.
More complex commands will need structured success/error results and documented
payload schemas shared with Rust. Do not expose credentials in error messages.

## Settings About

About mounts on its first visit and stays mounted across tab and sidebar
navigation. It uses General's full content width, with the expanded `BrandLogo`
and description above version, license and repository rows. All three rows
have a 56 px minimum height. The GitHub action is a bare 24 px icon button with
no tile background or border, a translated name, visible focus, disabled opening
state and retryable error feedback. Its Octicons MIT notice is bundled in
`assets/licenses/octicons-MIT.txt`.

`getAppInfo()` reads Rust build metadata. Loading, ready, browser preview and
error/retry states remain distinct; preview never invents a desktop version.
Effect cleanup rejects obsolete StrictMode responses. Language changes translate
feedback without rereading. Tabs use one Tab stop; arrows, Home and End select
and focus native buttons.

`openProjectRepository()` calls a Rust command with no URL or executable input.
Rust uses the [opener crate's API](https://docs.rs/tauri-plugin-opener/2.7.0/tauri_plugin_opener/fn.open_url.html)
for the fixed `https://github.com/Johnny0x38E/vibemate` address. The opener plugin
is not registered and no generic opener IPC permissions are granted. Browser
preview opens the same URL with `noopener,noreferrer`; private OS errors become
bundled feedback. The icon is from [GitHub Octicons](https://github.com/primer/octicons/blob/main/icons/mark-github-16.svg).

## Product branding

`BrandLogo` renders the expanded V/wordmark in the sidebar and About, and only
the V in the collapsed rail. The brand-home button has no hover decoration and
retains keyboard focus. Its accessible name belongs to the button; the SVG is
decorative.

The V uses forest `#324e40` and sage `#a8b8a7`, with the reference's shallow lower
turn. The wordmark uses reference letter shapes through an SVG mask: a color
matrix removes the pale background and `currentColor` supplies text color.
The original i dot is cleared before drawing the raised orange `#db915b` circle.
The lettering is raster-derived; the V is vector artwork.

The expanded/collapsed exports are in `assets/brand/`. The app icon uses an
off-white tile. The macOS master has separate transparent padding, continuous
corners and a slight downward V offset, leaving other platform icons and sidebar
geometry unchanged. [Brand asset instructions](../assets/brand/README.md) record
the masters, dimensions and locked Tauri generation commands.

## Built-in color themes

General shows all six themes as native radio circles, in this order:

| English  | 简体中文 | Persisted ID | Palette                       |
| -------- | -------- | ------------ | ----------------------------- |
| Forest   | 森林     | `forest`     | Calm green                    |
| Ink      | 纸墨     | `notion`     | White, warm gray and charcoal |
| Graphite | 石墨     | `graphite`   | Neutral monochrome            |
| Linen    | 亚麻     | `linen`      | Warm paper and clay           |
| Iris     | 鸢尾     | `iris`       | Restrained violet             |
| Ocean    | 海湾     | `ocean`      | Cool blue                     |

Each theme has light/dark values in `src/themes.css`; `App.css` maps them to
shared semantic colors. Brightness is independent of theme. System brightness
uses `prefers-color-scheme` in CSS. Circles reuse palette accents, with translated
names, selected rings and checks. Native radios provide arrow-key selection;
forced-colors mode can show the native input. Brand V colors remain fixed.

Rust validates and atomically saves brightness/theme pairs. Thin Tauri commands
run SQLite work in a blocking worker so database waits do not stall the async
executor. A missing row means
system/forest without writing defaults. Schema v3 created the singleton table;
v4 extends its CHECK constraint by rebuilding that owned table in a transaction.
Tests preserve all five prior themes, language and unrelated rows, save Ink
and reopen, reject unknown themes, and verify an invalid v3 row rolls back the
upgrade without data loss.

The persisted `notion` ID remains unchanged after renaming the display label to
Ink / 纸墨. Its neutral colors refer to Notion's [official public stylesheet](https://www.notion.so/_assets/77281-c4b3c46f690b55b2.css)
linked from the [login page](https://www.notion.so/login), observed 2026-10-10.
Hover, selection and focus/accent values are adapted to vibemate. Light secondary
text is darkened from `#7d7a75` to `#686560` for contrast. This is a color
reference, not an integration with Notion.

`AppearanceControl` applies root attributes only after an exact saved-pair
acknowledgment. Saves disable controls without showing temporary saving or
success text. Known failed writes keep the previous choice and permit retry;
uncertain saves block writes until reload. Read/loading, preview, error and
reload feedback remains. Browser preview applies local colors without claiming
they were saved. Cleanup restores root attributes; navigation retains mounted
controls, pending operations and edited input.

## Appearance acceptance and verification

The maintainer closed the current basic appearance iteration on 2026-10-10.
Visual checking is manual unless requested. Automated formatting, lint, types,
behavior tests and applicable builds remain required. This documentation cleanup
does not rerun or extend the recorded verification below.

| Scope      | Latest recorded result                                                                                                                                                                                                                           |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Frontend   | `check:frontend`: 101 UI tests, eight release-tool tests, format, lint, accessibility lint, both TypeScript configs and production build passed. The final CSS baseline adjustment also passed formatting and the native build's frontend build. |
| Rust       | fmt and Clippy passed; 33 tests passed; the existing real OS-keychain smoke test was ignored.                                                                                                                                                    |
| macOS      | Locked release builds and unsigned local app bundles passed; the final generated ICNS matches the packaged icon.                                                                                                                                 |
| Appearance | The maintainer closed the current shell, branding, settings, six palettes and Dock refinements.                                                                                                                                                  |
| Pending    | Windows/Linux runtime and native icon display; actual system-browser opening; native Ink selection followed by restart; comprehensive accessibility testing.                                                                                     |

Earlier checks retain their original scope: I02.d read version 0.1.0 in the
macOS runtime and demonstrated obsolete-response test failures with cleanup
disabled. I03 checked packaged macOS branding. I04/I05 checked real dark/Iris
selection and restart with isolated app data and inspected the original ten
light/dark palette variants. Those browser checks preceded the maintainer's
manual-only visual-check preference.

Ink token contrast was measured against canvas, sidebar, surface, hover and
selected backgrounds: light primary/secondary minima 11.41/4.74, dark minima
11.51/5.66, and light/dark focus minima 5.30/8.83. These measurements cover the
named token pairs, not all accessibility behavior.
