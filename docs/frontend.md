# Frontend development

The frontend is React + TypeScript, rendered inside Tauri's system WebView.
React describes the interface; Rust handles local system and provider operations.
`src/AGENTS.md` contains the frontend implementation rules.

## Current code and planned layout

- `src/main.tsx` locates the HTML root and mounts React in StrictMode, with
  `LocaleStartup` withholding the App until its language preference is ready.
- `src/App.tsx` composes the desktop shell: the collapsible 200/88 px sidebar
  (brand, Overview, four feature destinations, Settings with the icon-only
  collapse toggle), the relationship home, `ProvidersView` (mounted on the first
  visit, then kept mounted while hidden, so an open form survives navigation and
  returning does not re-read the list), central MCP management, planned pages for Agents/Skills,
  and `SettingsView` (mounted while hidden) for the settings area.
- `src/features/providers/` contains `ProvidersView` (list and in-page
  navigation), `ProviderPage` (back control and title), `ProviderTabbedView`
  (API / Models tabs), `ProviderForm` (API fields in a settings-style card),
  `ProviderModels` and `ProviderModelsCreateGate`, `ProviderIcon`, and
  `providerButtons.module.css`; see [Provider configurations](#provider-configurations).
- `src/features/settings/SettingsView.tsx` uses `PageModuleHeader`, General/About
  tabs, and panels. General hosts `LanguageSelector` (with `footer={<AppearanceControl />}`)
  and a separate `LogSettings` section (group heading plus its own card), both
  composed in `src/main.tsx`. Tab switches and leaving Settings keep preference
  controls mounted so pending saves and input survive.
- `src/components/FieldSelect.tsx` is the shared single-choice dropdown (36 px
  trigger, chevron from `settingsField.field`, Base UI Select interaction and
  a portaled list). Settings and provider forms use it instead of a native
  `<select>`; CSS Modules and existing theme tokens still own its appearance.
- `src/features/settings/settingsField.module.css` shares the grouped-card row
  layout (label left, control right) for language and appearance.
- Sidebar width is fixed at 200/88 px (`--sidebar-width`, `data-collapsed`); only
  the collapse button changes layout.
- `src/components/WindowDragRegion.tsx` and `TitlebarChrome.tsx` (with
  `WindowControls` on Windows/Linux builds) implement drag strips and custom
  window chrome. Never place buttons or fields inside a drag region; an App test
  enforces this. Window IPC lives in `src/lib/desktop/window.ts`.
- `src/components/BrandLogo.tsx` renders one SVG logo: the transparent V and rounded
  wordmark when expanded, and the V symbol alone when collapsed. The wordmark uses
  a mask from the selected reference without a font dependency. `src/components/Icon.tsx` holds UI icons.
- `src/components/PageModuleHeader.tsx` is the shared module title row (Providers
  list layout): 20 px sidebar `Icon`, 1 rem semibold `h1`, optional trailing
  actions or meta on the end, a 36 px minimum row height so titles align
  vertically even without a trailing button, and 2 px top margin for clearance
  under the Windows/Linux title strip. Overview, Providers list, planned
  Agents/Skills/MCP, and Settings all use it; icons are decorative. Provider
  create/edit headers use the same 2 px offset in `ProviderPage.module.css`.
- `src/components/Notifications.tsx` is the app-level notification host, mounted
  once in `App`. Features call `useNotify()` with an already translated
  `message` and optional `tone` (`success` | `error`); a plain string defaults
  to success. One notification shows at a time, top-right below
  the title strip, in a polite `role="status"` region that is always mounted and
  holds only the message text (the close button sits beside it in the same
  card, outside the live region), so it is announced without moving focus or
  reading the button. It dismisses itself after 3.5 s
  (`NOTIFICATION_DURATION_MS`); the timer pauses while hovered or while focus is
  inside, and keeps the time left. The close button ("Dismiss notification" /
  「关闭通知」) or Escape on it closes it and returns focus to where the user came
  from. Styling uses `--color-surface`, `--color-text`, `--color-border`,
  `--color-summary` (close icon), and tone tokens `--color-success-*` /
  `--color-danger-*`; the entry fade only runs under
  `prefers-reduced-motion: no-preference`. Outside the provider, `notify` does
  nothing.
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

- As functionality grows, use `src/features/agents/`, `src/features/skills/`,
  and `src/features/mcp/` for implemented features.
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

## Shared dropdown interaction

`FieldSelect` uses the stable MIT-licensed `@base-ui/react` 1.9.0 Select. Base UI
is headless: it supplies behavior without a visual theme or a Tailwind requirement.
Only the Select entry point is imported. Existing buttons, inputs, theme radios,
layout, notifications and business state are not migrated to a new framework.
Future complex controls can use Base UI when an implemented behavior needs them.

The parent still owns `value` and `onChange`. Selecting a language or appearance
requests a save; the widget cannot treat that request as a confirmed saved value.
Base UI handles item focus, arrow keys, Home/End, character-based navigation,
Enter/Space selection and dismissal. The wrapper controls only whether the popup
is open and whether a business operation has locked the field.

`disabled` remains a native disabled button. `blocked` keeps the provider field
focusable with `aria-disabled`, uses Base UI's read-only value protection, and
refuses opening or value callbacks. A conditional state reset during the field's
own render closes an already-open menu when it becomes locked. The next render
has `open = false`, so unlocking cannot reopen an old menu. This is a guarded
update of the same component, not an effect that repeatedly derives state.

`Select.Portal` renders the menu outside the card's DOM subtree while retaining
React composition. This avoids card/scroll-container clipping. Tokens on `:root`
still supply the palette. The named `--layer-popover` token places menus above
ordinary app content. Positioning uses the trigger width, a 4 px gap and viewport
collision handling; the list scrolls within the available height. A hidden anchor
also hides its portaled popup. `alignItemWithTrigger={false}` prevents the menu
from overlapping the selected value.

Tests use MIT-licensed `@testing-library/user-event` 14.6.7 for complete pointer
and keyboard sequences. The shared helpers follow `aria-controls`, not DOM
parentage, and are asynchronous because pointer opening and focus restoration
can wait for animation frames. Callers must await them. Fourteen FieldSelect
cases cover selection, actual focus movement/restoration, Home/End, typeahead,
Escape, Tab, outside clicks, both lock transitions, labels/icons, multiple fields
and StrictMode. The Tab test supplies visible-button rectangles because jsdom
has no layout; it does not establish real viewport placement.

Verification: frozen install, `check:frontend` (405 UI/boundary tests, 14 i18n
checker tests and eight release-tool tests), Rust fmt/Clippy, 182 Rust tests
(one real OS-credential test ignored), and a locked macOS no-bundle desktop build
passed. Production license metadata for the new runtime dependencies is MIT.
The main JavaScript bundle changed from 359.95 kB / 109.65 kB gzip to
471.90 kB / 149.42 kB gzip in these local builds. No CSP, capability, translation
resource or Rust code changes were needed. Visual checks follow the maintainer's
manual-review policy; the maintainer confirmed this migration has no issues.
Dedicated screen-reader and Windows/Linux runtime verification remain pending.

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

It uses the shared `FieldSelect` (`src/components/FieldSelect.tsx`) and a live
status message. Preview is disabled
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
variables; the retry action and `FieldSelect` trigger have visible keyboard focus
styles.

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
Testing Library is an explicit peer dependency; the five test packages, including
user-event for dropdown interactions, are MIT.
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

## Model IPC boundary

`src/lib/desktop/models.ts` wraps the seven model commands registered in Rust.
It does not fetch provider URLs or read credentials. Rust remains responsible
for model validation, fuzzy search, persistence, cancellation and network limits.
The model interface is not wired to these wrappers yet.

Each response is received as `unknown`. The wrapper checks exact field sets,
nullable metadata, enum values, non-negative safe integers, provider identity,
duplicate model IDs and requested page limits before returning typed data.
Selection acknowledgments must contain exactly the requested unique IDs and
selection state. A manual-add acknowledgment must identify the normalized ID
as a selected manual row; deletion requires a null acknowledgment. Ranked search
results have a match count and no next-page cursor. A fetch summary reports list
and merge counts, not proof of valid credentials or working inference.

`ModelRequestError` keeps only Rust's model error-code allowlist, plus
`invalid_response` and `desktop_required` generated here. Unknown strings,
objects and exceptions become `operation_failed`; the original payload is
not retained. After a mutation returns `invalid_response` or `operation_failed`,
reload persisted data before retrying: IPC failure does not establish rollback.
Browser reads return `{ kind: "preview" }`, mutations/fetch reject with
`desktop_required`, and cancellation returns `{ wasRunning: false }` without IPC.

The 85 boundary tests in `src/lib/desktop/models.test.ts` replace only Tauri's
runtime check and invocation. They cover request shapes, missing/extra fields,
malformed nested metadata, mismatched acknowledgments, safe errors and preview.
They do not verify native WebView IPC, OS credential access or real providers.

## Settings logs

`LogSettings` is a separate section below the language/appearance card on the
General tab (its own heading and grouped-card row, same pattern as future General
sections). It shows
the actual Rust-owned log file path in the same grouped-card row layout as language
and appearance (label left, value right-aligned), with wrapping and text selection.
Two small icon buttons after the path request a local text tool (View logs /
「查看日志」) or the system file manager (Open log folder / 「打开日志文件夹」).
Paths are read once when the group mounts.

`src/lib/desktop/logs.ts` validates both paths and the startup active flag,
rejects malformed responses, and keeps only safe error codes. Browser preview
shows no invented path or OS actions. An inactive startup file logger gets a
warning; an older log can still be opened. Raw errors are never rendered.

The component keeps read and open states separate. Each read has a generation
number, so StrictMode cleanup and unmount invalidate stale results.
A synchronous ref blocks rapid duplicate or competing open requests; rendered
buttons use `aria-disabled` rather than dropping focus with `disabled`.
The group remains mounted across settings/sidebar navigation, preserving paths
and pending actions. It never launches a tool just because it was displayed.

Rust accepts no frontend path or executable. It checks the fixed paths and uses
the existing opener's Rust functions, with TextEdit on macOS, Notepad on Windows
and the default file association on Linux. No opener/plugin capability or CSP
change is needed. Dispatch acknowledgment is not proof that a viewer displayed
the file, so the UI makes no success claim. See
[logging access and safety rules](plans/logging.md).

Verified after integrating main `4e4233f` with 327 UI tests, the full frontend
check, Rust fmt/Clippy, 174 passing Rust tests (one ignored real credential-store test), and a locked macOS no-bundle
release build. Actual native editor/file-manager opening and visual review remain
manual checks; no external tools were launched by the automated tests.

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

## Provider configurations

P10 lets a user save provider settings; P11 adds the API key. A new
configuration takes its key in the create form, sent with the other fields, and
a saved one shows only whether a key is set and lets the user replace it (see
"API keys" below). Saved instances are not used for any connection or agent
change yet; the page says so.

**Rust.** `src-tauri/src/providers.rs` validates and stores instances in the
`provider_instance` table added by schema v5. An instance has a stable random ID
(32 lowercase hex characters from SQLite `randomblob(16)`), a fixed kind
(`command-code`, `deepseek`, `openrouter`), a trimmed display name of 1–64
characters, a normalized HTTPS base URL, a protocol, a `revision`, and creation
and update times. Names may repeat; only the ID identifies an instance. Base URLs
are parsed with the `url` crate: user names/passwords, queries, fragments and
non-HTTPS schemes are rejected; the scheme and host are lowercased, international
domains become punycode, and one trailing `/` is removed (no path is added).
Every kind currently allows only `chat_completions`, and every extension
allowlist is empty, so any extension key is rejected
(`extension_field_not_supported`). Keys are added only after official evidence.

**IPC.** Five provider commands: `list_provider_templates`, `list_providers`,
`get_provider`, `create_provider` (whose request also carries the required
`secret`), `update_provider`; and two key commands wrapped by
`src/lib/desktop/providerSecrets.ts`: `get_provider_secret_status({ providerId })`
and `replace_provider_secret({ request: { providerId, secret } })`. There is no
delete or clear command.
`src/lib/desktop/providers.ts` validates every response with a strict key set
(an extra field, such as an echoed key, is `invalid_response`) and keeps only the
safe error codes below; `providerSecrets.ts` reuses its error class and
allowlist. Lists use an opaque cursor: pass `nextCursor` back as `after`;
`limit` must be 1–100 (the page uses 20), and rows are ordered by creation time,
then ID. Unlike an offset, a cursor cannot skip or repeat rows when another
window inserts one between pages. An edit sends `id` and `expectedRevision`; a
stale revision is refused with `revision_conflict` instead of overwriting a newer
save. The kind is never part of an edit request.

**UI.** The Providers page switches views inside `ProvidersView` (state
`list | create | edit`, no router library). The list view uses `PageModuleHeader`
with the `providers` icon and "New configuration" in the trailing slot. The list
view shows loading, empty,
error-with-retry, saved rows and "Load more" while `nextCursor` is set. Each row
is two lines: line 1 has a 40 px brand icon, the official brand name, a protocol
pill and a selected-model count pill; line 2 shows the saved display name aligned
with the brand text (not the icon). Base URL is omitted on the list. A weak
"Edit" action sits on the row. Rows are keyed by stable ID. List data stays in
`ProvidersView` while a secondary view is open; going back does not re-read the
whole list, but returning from edit refreshes that row from storage so model
counts stay current.

**Brand icons.** `ProviderIcon` maps a provider kind (never a display name) to
an official brand file in `assets/providers/`, imported through Vite like
`assets/brand/` (emitted as files, so the `img-src 'self'` CSP allows them).
Each list row shows it before the name; the detail page shows it before the
read-only provider value; the create form uses `FieldSelect` so the trigger and
each menu option show the logo before the brand name. Edit page titles use the
same logo and brand name. The icon sits in a 20×20 box with `object-fit: contain`,
is decorative (`alt=""`), and unknown kinds render nothing. The files are kept
byte-for-byte as published: no recolouring or stretching, and Prettier has no
SVG parser, so checks never rewrite them. OpenRouter has a separate dark file;
both are rendered and `ProviderIcon.module.css` shows one with the same
conditions App.css uses for dark tokens (`data-appearance` and
`prefers-color-scheme`), so no script repeats the theme decision.

| Kind           | File(s)                                 | Source                                                                                     | Brand guidelines                                                                  |
| -------------- | --------------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| `command-code` | `command-code.svg` (both themes)        | `symbol.svg` from https://commandcode.ai/brand                                             | https://commandcode.ai/brand                                                      |
| `deepseek`     | `deepseek.svg` (both themes)            | https://api-docs.deepseek.com/img/favicon.svg                                              | No public brand kit; the maintainer reviewed the terms and chose to use the icon. |
| `openrouter`   | `openrouter.svg`, `openrouter-volt.svg` | `glyph-grape` (light theme) and `glyph-volt` (dark theme) from https://openrouter.ai/brand | https://openrouter.ai/brand                                                       |

Trademark note: the README states that "Provider names and logos are
trademarks of their respective owners and are used only to identify the
providers." (「服务商名称和标志的商标归各自所有者，仅用于标识服务商」). The icons
above are used on that basis only.

Create and edit use `ProviderPage` and `ProviderTabbedView`. The secondary header
is one row: a surface back control (border, 18px arrow plus "Back" / 「返回」,
`min-height` 36 px, 2 px top margin) on the left and the page title (official
brand icon and name) aligned on the right. Keys `providers.back` and
`providers.backLabel` supply the visible label and `aria-label` (WCAG 2.5.3).
Create updates the title when the provider field changes.

Tab layout (Settings-style tabs: **API** / 「API 配置」 and **Models** with
optional selected count). Panels stay mounted while
switching tabs. Create shows a Models gate until the instance is saved. API fields
live in a grouped card below a right-aligned **Save** on the tab toolbar; the
provider row follows (select when creating, read-only brand when editing), then display
name, base URL, protocol and API key. Recovery actions ("Save again", "Refresh
list", "Reload latest settings") stay at the bottom of the form when needed.
There is no form Cancel; **Back** leaves the page. One Save submits settings and
an optional replacement key together.

The Models tab follows [models-tab-ux.md](plans/models-tab-ux.md) (P12.c.5). In
brief: the default **Selected** view lists only persisted checked models (full
load, scroll inside the tab panel). A fixed list title (**Selected** / **All**)
sits above the scroll region. The toolbar has a narrow search field on the left;
**Fetch models** (secondary) and **Save** (primary, 36 px) on the right in the
selected view; upstream adds **Cancel** (while downloading) and **Back to
selected** before **Save**. **Fetch models** opens **upstream** browse with
lazy-loaded result pages via `browse_upstream_models_page` (optional fuzzy
`query`; session catalog in Rust). Checkboxes are draft until **Save**. Uncheck
on the selected list and **Save** removes rows from SQLite (`save_provider_model_selections`).
Search applies to whichever view is active. One list footer line covers upstream
scroll, loading-more, and end states. Dirty drafts block leaving with an unsaved
prompt (API tab switch and **Back** on the provider page).

The selected list now has an expandable **Add a model manually** form
(`ManualModelForm`): model ID and optional alias, followed by **Add and save**.
This explicitly saves and selects one manual row through the existing Rust
command; it does not stage an upstream checkbox draft. Pending checkbox drafts
block creation until saved or discarded. The manual row is removed by unchecking
it and using the normal selection **Save**. Route/capability metadata stays
unverified; an alias is only a display label.

Rust field errors are associated with the ID or alias input. Successful creation
clears the form and extends the selected baseline, row list and tab count without
a provider network request. Pending manual writes block fetching, checkbox saves
and the detail-page Back action. Hiding the view unmounts the form; late replies are
ignored, and the existing selected-list read reconciles persisted state on return.
An unknown write acknowledgment blocks a second submission until **Check saved
models** reads cursor pages for the exact ID. Similar IDs never confirm a write;
a failed read leaves the check retryable. Cursor reads also support valid IDs
longer than the 200-character fuzzy-search limit.

Automated form and list tests cover creation, field errors, duplicate submission,
unknown-result checks, pagination, bilingual feedback, draft protection, deletion
requests and late replies. Real Tauri add/restart/delete and visual review remain
manual checks; DOM tests do not establish native persistence.

Rust validates submitted values: field codes appear next to their field and
other codes at form level. After a successful save the page returns to the list,
the row shows the values Rust stored, and the list is re-read. The save is
confirmed by an app-level notification (`Saved name.` / 「已保存 name。」,
see `Notifications`) that survives the jump back to the list.

- Hierarchy: each view has exactly one primary button ("New configuration",
  "Save" or "Fetch models" on the Models toolbar): solid `--color-accent` with
  `--color-background` text. Recovery actions and "Load more" are secondary
  (`--color-border` outline); row "Edit" is a text button. "New configuration"
  (`plus`) and row "Edit" (`edit`) carry a leading 18px `Icon`; create/edit Back
  shows the back arrow and short label on the left of the secondary header (title
  on the right). Save and
  recovery actions stay text-only. Blocked buttons
  use `aria-disabled` styling (muted `--color-summary`, or half opacity for the
  primary). Labels and values use `--color-text` at 0.8125rem and normal weight;
  hints and secondary values use `--color-summary` at 0.75rem; errors use
  `--color-text`, semibold, with a leading rule. Every focus ring is
  `--color-focus`. Protocol and count pills on the provider list use per-palette
  `--color-chip-info-*` and `--color-chip-count-*` (defined in `themes.css`,
  resolved in `App.css`).
- Focus: opening a secondary view focuses its title; Back returns focus to "New
  configuration" or that row's "Edit"; a successful save focuses the saved row's
  "Edit" (found by ID), or the page heading when a new row is not on the loaded
  pages. Controls that can hold focus are never `disabled` while work is pending:
  buttons and selects use `aria-disabled`, text fields `readOnly`, and handlers
  check the state. Back is blocked while API save or model fetch/list work is
  pending. "Saving…" is announced through a `role="status"` message.

- `operation_failed` or `invalid_response` after a save means the outcome is
  unknown. The form re-reads the list first and offers "Save again" only after a
  successful refresh, so a second click cannot silently create a duplicate.
- `revision_conflict` blocks saving until "Reload latest settings" loads the
  newer record for review.
- `create_outcome_unknown` (Rust could not confirm a create, and an unused key
  entry may remain in the credential store) follows the same refresh-first flow.
  The key field was cleared, so the text after the refresh says that "Save
  again" needs the key typed again; the key field stays editable in this state
  while the other fields are read-only.
- Browser preview shows that nothing can be read or saved, hides "New
  configuration", and never calls IPC.

### API keys

New and edit forms both use an "API key" / 「API 密钥」 password field with
`autoComplete="new-password"`, spellchecking and automatic capitalization off.
There is no permanent key explanation, separate Keys form, visible status label
or update timestamp.

The edit form reads `getProviderSecretStatus(id)` on mount (StrictMode may repeat
the read). This reads only SQLite metadata, not the key or OS credential store.
A configured key appears as a fixed `••••••••` placeholder and a non-interactive
closed-eye icon. The input value remains empty; neither the mask nor any stored
key is fetched or submitted. An accessible name announces that a key is
configured. Typing a replacement removes the indicator.

A blank edit field preserves the existing key. Creation and legacy instances
without a key require input; empty input produces a field error and moves focus
there. The frontend otherwise sends the value exactly as typed. Rust trims and
validates it. Loading, failed status reads and browser preview block saves
without pretending a key exists. A failed read offers a retry; focus moves to
the input before the retry button disappears. Obsolete reads cannot replace a
newer state or update an unmounted form.

**One save.** `updateProvider` sends non-secret `request` fields and a separate
`secret` argument (`null` when omitted) to `update_provider`. Rust uses
`update_provider_with_secret`:

- With no replacement, update settings only; never access the credential store.
- With a replacement, validate settings, revision and key before OS access.
  Keep the old key temporarily in memory, write the new key, then update settings
  and the credential reference in one SQLite transaction. Recheck the revision
  after the OS call. No database lock spans a system credential-store prompt.
- A definite transaction failure restores the old key. A failed restoration or
  uncertain commit returns `operation_failed`, not success or a claimed rollback.
  The process-wide credential-write lock serializes writes within this app;
  separate app processes are not serialized.

A successful acknowledgment returns to the list and emits the existing saved
notification. Pending saves preserve focus with `readOnly` and
`aria-disabled`; duplicate submissions cannot start another write. Definite
errors retain draft settings but clear the typed key. Invalid keys appear beside
the field; credential-store errors appear at form level. Unknown outcomes
refresh before an explicit retry. If that attempt included a replacement key,
retry requires it to be typed again; reading metadata cannot confirm which key
the credential store holds. Revision conflicts require reloading the latest
settings for review.

Typed keys are cleared before every submit, when leaving the page, on unmount
and before a hidden frame renders. They are never passed to parent callbacks, translations,
notifications, logs or persistent frontend storage. IPC and OS credential access
necessarily receive the new value; SQLite and responses hold only references and
non-secret settings. The legacy standalone replacement command and its safe
desktop adapter remain available, but the provider interface does not call them.

Tests in `ProviderForm.edit.test.tsx` cover bilingual replacement, legacy
instances, status retry, focus, uncertain outcomes, duplicate submits and obsolete
StrictMode reads. `ProviderForm.test.tsx` also covers blank-field preservation.
Boundary tests verify the single IPC payload, strict responses and sanitized
errors. Rust `providers::tests::unified_save` uses temporary databases and an
injected credential store for coordinated commits, settings-only saves,
validation, failed credentials, transaction rollback, legacy references,
uncertain commits, concurrent edits, failed restoration and key exclusion from
the database and response.

Verified: `check:frontend` (frontend tests, translation checker and release-tool
tests), Rust fmt/Clippy, passing Rust tests and a locked macOS no-bundle desktop
build. One real OS credential-store test remains ignored. Native save/restart,
model fetch against live providers, keychain prompts, visual acceptance and
Windows/Linux runtime behavior remain manual checks.

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

## Central MCP management

`src/features/mcp/` implements P31. The sidebar opens a metadata list lazily on
first visit. New/edit pages collect display name, server identifier, transport,
stdio executable/ordered arguments/optional cwd, or Streamable HTTP URL.
Environment variables and request headers use named password rows. An empty
existing value preserves its OS reference; a replacement writes a fresh reference;
removing a row removes that field. Enter one argument per line, without shell
quoting. Saving only stores a definition. Enable/disable describes central state,
never an applied Agent config or connection.

`McpForm` clears typed values on every submit, cancel, transport change and hide.
Its hidden transition clears during rendering, before a hidden panel can retain
values through a rapid navigation change. Non-secret drafts stay mounted. Unknown
write outcomes block another mutation until a successful metadata reload. The
list supports bounded cursor pages, safe retry, and obsolete credential cleanup.
Unmount guards ignore late responses; page transitions focus the heading.

`src/lib/desktop/mcp.ts` validates exact metadata shapes and cursor ordering,
rejects unexpected secret-bearing response fields, discards raw errors, and
blocks mutations in browser preview. Rust is authoritative for field validation.
All env/header values use OS credential storage; frontend persistent storage is
never used. Commands run blocking storage work off the async executor, reuse the
credential mutation lock and add no capabilities or dependencies.

Behavior coverage includes 21 IPC-boundary cases, 11 form cases, ten manager
cases and one shell-navigation case, including hidden-value clearing, stale
replies, reconciliation and browser preview. Visual and real credential-store
acceptance remain separate from these simulated tests.

The form reuses Providers' secondary-header/back styles and button hierarchy,
with Back on the left, title on the right and Save at the top of the card. Shared
`FieldSelect`, 36 px inputs, theme tokens, notifications and list row spacing
follow the existing pages rather than introducing another control system.

Verified locally on Linux: `check:frontend` with 468 UI/boundary tests, eight
release tests and 14 translation-checker tests; Rust fmt/Clippy and 201 passing
Rust tests (one real credential-store test ignored); locked debug and release
no-bundle desktop builds. An isolated real Tauri runtime saved, edited, listed,
restarted and re-read a no-secret stdio definition; its test command never ran.
Real MCP credential-store acceptance and macOS/Windows runtime behavior remain
pending. Visual checks remain manual. Vite reports the existing single-chunk
size advisory; it is not a
verification of layout or accessibility.
