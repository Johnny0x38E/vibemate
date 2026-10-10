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
  returning does not re-read the list), planned pages for Agents/Skills/MCP,
  and `SettingsView` (mounted while hidden) for the settings area.
- `src/features/providers/` contains `ProvidersView` (the list and the switch
  between its views), `ProviderPage` (secondary page shell), `ProviderForm`
  (the "Basic information" group), `ProviderIcon` (brand icons) and
  `providerButtons.module.css` (button hierarchy); see
  [Provider configurations](#provider-configurations).
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
- `src/components/Notifications.tsx` is the app-level notification host, mounted
  once in `App`. Features call `useNotify()` and pass an already translated
  message: `notify(t("…"))`. One notification shows at a time, top-right below
  the title strip, in a polite `role="status"` region that is always mounted and
  holds only the message text (the close button sits beside it in the same
  card, outside the live region), so it is announced without moving focus or
  reading the button. It dismisses itself after 3.5 s
  (`NOTIFICATION_DURATION_MS`); the timer pauses while hovered or while focus is
  inside, and keeps the time left. The close button ("Dismiss notification" /
  「关闭通知」) or Escape on it closes it and returns focus to where the user came
  from. Styling uses `--color-surface`, `--color-text`, `--color-border` and
  `--color-summary` (close icon); the entry fade only runs under
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

## Settings logs

`LogSettings` is a separate group below the General preference controls. It shows
the actual Rust-owned log file and folder paths without abbreviating them, with
wrapping and text selection for long paths. View logs / 「查看日志」 requests a
local text tool; Open log folder / 「打开日志文件夹」 requests the system file
manager. Refresh log paths / 「重新读取日志路径」 reloads metadata, not the logger.

`src/lib/desktop/logs.ts` validates both paths and the startup active flag,
rejects malformed responses, and keeps only safe error codes. Browser preview
shows no invented path or OS actions. An inactive startup file logger gets a
warning; an older log can still be opened. Raw errors are never rendered.

The component keeps read and open states separate. Each read has a generation
number, so StrictMode cleanup, refresh and unmount invalidate stale results.
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
`list | create | edit`, no router library). The list view shows
loading, empty, error-with-retry, the saved rows and a "Load more" button while
`nextCursor` is set. Each row puts the name first (foreground, semibold), then
provider · protocol and the normalized URL (muted), then the short ID (muted,
smaller), with a weak "Edit" action. Rows are keyed and edited by ID, so two rows
with the same name stay distinct. List data stays in `ProvidersView` while a
secondary view is open, so going back does not re-read it.

**Brand icons.** `ProviderIcon` maps a provider kind (never a display name) to
an official brand file in `assets/providers/`, imported through Vite like
`assets/brand/` (emitted as files, so the `img-src 'self'` CSP allows them).
Each list row shows it before the name; the detail page shows it before the
read-only provider value, and the create form beside the provider select,
following the current choice (a native `<select>` cannot hold images). The
title stays text only. The icon sits in a 20×20 box with `object-fit: contain`,
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

Secondary views share `ProviderPage`: a back link (the shared `Icon` "back"
arrow at 18px plus the short text "Back" / 「返回」, at least 32px tall) and the
page title on top. The link's `aria-label` is the fuller "Back to provider
list" / 「返回服务商列表」 (separate keys `providers.back` and
`providers.backLabel`), which starts with the visible text (WCAG 2.5.3). "New"
opens the create form directly. Its first field is the provider select, which
starts on the first template in `list_provider_templates` order with that
template's name, base URL and protocol filled in (there is no empty option).
Switching provider replaces only values that still equal the previous
template's defaults; edited values are kept, and a protocol the new provider
does not allow falls back to its default. The protocol select lists only the
template's allowed values. The provider is read-only when editing. The detail page groups fields
like the Settings page; today there are two stacked groups, "Basic information"
(name, base URL, protocol, plus the read-only provider and full ID) and "Keys"
(P11, `ProviderKeys`). Each group is its own form with its own action, so P12
(models) adds a group instead of lengthening one form; whether to switch to
Settings-style tabs then is still open (P11 kept two stacked groups, as the
maintainer asked for a separate group). Rust is the only validator: field codes appear next to their field and
other codes at form level. After a successful save the page returns to the list,
the row shows the values Rust stored (for example a trimmed name or a normalized
URL), and the list is re-read. The save is confirmed by an app-level
notification ("Saved “name”." / 「已保存「name」。」, see `Notifications`) that
survives the jump back to the list; the list page itself keeps no message.

- Hierarchy: each view has exactly one primary button ("New configuration" or
  "Save"): solid `--color-accent` with `--color-background` text. Cancel,
  recovery actions ("Save again", "Reload latest settings", "Try again") and
  "Load more" are secondary (`--color-border` outline, `--color-text`); the back
  link and row "Edit" are text buttons. "New configuration" (`plus`), row
  "Edit" (`edit`) and the back link (`back`) carry a leading 18px `Icon`
  (`aria-hidden`, centered with the label via `buttons.withIcon`); Save, Cancel
  and recovery actions stay text-only, like every form button elsewhere in the
  app, so icons mark navigation and entry points rather than every button.
  Blocked buttons use `aria-disabled`
  styling (muted `--color-summary`, or half opacity for the primary). Labels and values use
  `--color-text` at 0.8125rem and normal weight; hints and secondary values use
  `--color-summary` at 0.75rem; errors use `--color-text`, semibold, with a
  leading rule, so they never read as hints. Every focus ring is `--color-focus`.
- Focus: opening a secondary view focuses its title; Back or Cancel returns focus
  to "New configuration" or to that row's "Edit"; a successful save focuses the
  saved row's "Edit" (found by ID), or the page heading when a new row is not on
  the loaded pages. The re-read after a save re-applies that focus unless the
  user has moved it, and a superseded read applies nothing. Controls that can hold focus are never `disabled` while
  work is pending (disabling the focused element drops focus to the page):
  buttons and selects use `aria-disabled`, text fields `readOnly`, and handlers
  check the state. The back link is blocked while a save is pending, so its
  outcome cannot be hidden. "Saving…" is announced through a `role="status"`
  message.

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

**Create.** The create form ends with a required "API key" / 「API 密钥」
field (`type="password"`, `autoComplete="new-password"`, because browsers ignore
`off` on password fields, plus `spellCheck={false}`, `autoCapitalize="none"` and
`autoCorrect="off"`), with a hint that the key is kept only in the system
credential store and that the field is cleared after every attempt. The edit
form has no key field. The only frontend check is that the trimmed value is not
empty ("Enter the API key." / 「请输入 API 密钥。」, next to the field, focus moves
there, no IPC); the key is otherwise sent exactly as typed and Rust is the only
validator (`secret_invalid`, next to the field). The list never shows a key or a
"needs a key" state.

**`ProviderKeys`.** The detail page has a second group, "Keys" / 「密钥」, below
"Basic information". It is its own `<form>`, so replacing a key never saves the
basic information, never changes the provider's `revision`, and never leaves
the page. It reads the status on open (Rust reads SQLite only, so opening the
page never triggers a credential-store prompt) and shows "Current key" /
「当前密钥」 as a `role="status"` value:

- `set`: "Set · updated {time}" / 「已设置 · 更新于 {time}」, the time formatted
  with `Intl.DateTimeFormat` (medium date, short time) in the UI language;
- `missing` (only P10-era rows or a `create_outcome_unknown` case): "No key set"
  / 「未设置密钥」; the field label becomes "API key" and the button "Set key" /
  「设置密钥」, using the same replace command;
- loading, a read error (with "Read key status again" / 「重新读取密钥状态」),
  preview ("cannot read or change API keys"), and "Status unknown" / 「状态未知」.

Below it, a password field ("New API key" / 「新的 API 密钥」, same attributes as
the create field) and "Replace key" / 「替换密钥」. There is no clear or delete
action. The button is secondary because the page's one primary action stays
the basic information "Save". While replacing, the button is `aria-disabled`,
the field `readOnly`, "Replacing the key…" is announced, and the page's back
link is blocked. On success the status updates in place and the app-level
notification says "API key replaced." / 「已替换 API 密钥。」 (or "API key set." /
「已设置 API 密钥。」 for a missing key); focus stays on the button. After
`secret_outcome_unknown`, `operation_failed` or `invalid_response` the status
shows "Status unknown" with the error and a secondary "Refresh key status" /
「刷新密钥状态」. Replacing is blocked only while that refresh runs (replacing
again is the documented remedy). After the refresh the text warns that "Set" does
not prove the new key was saved. A failed refresh keeps the unknown state, shows
the read error and offers the refresh again. When a button disappears, focus
moves to "Replace key".

**Error placement.** `secret_invalid` and the empty-key message sit next to the
key field (`aria-invalid`, `aria-describedby` with the hint). The three
`credential_store_*` codes concern the device, not one field, so they appear at
the top of the form or group, before the first field. Everything else stays
below the fields as in P10. The Keys group words errors by path, so a message
never claims more than happened: replace failures use
`providers.keys.replaceErrors.*` and always say whether the key changed, and
status-read failures use `providers.keys.readErrors.*` and never suggest a
write. Other codes use the generic `providers.errors.*` text (table below).

**Key hygiene.** The key lives only in the local state of `ProviderForm` or
`ProviderKeys`: it is never put into props, i18n interpolation, notifications,
error objects, storage or the console. Each submit takes the value out of the
field before the request starts, so the field is empty whatever the result
(success, failure, unknown, or the empty-key check). The field is also cleared
on Cancel and on unmount (Back, leaving the page), and while the page is
`hidden` (another app page is shown). Hiding is handled during render rather
than in an effect, so the key does not survive one hidden frame, while the
other draft fields are kept as in P10. Only the status is shown; no mask,
prefix or length.

| Code                             | From     | Shown at                    | English                                                                                                                                                                                                         | 简体中文                                                                                                                              |
| -------------------------------- | -------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `storage_unavailable`            | Rust     | Form or list                | Local configuration storage is unavailable. Restart vibemate.                                                                                                                                                   | 本地配置存储不可用，请重启 vibemate。                                                                                                 |
| `read_failed`                    | Rust     | Form or list                | Provider settings could not be read. Try again.                                                                                                                                                                 | 无法读取服务商配置，请重试。                                                                                                          |
| `write_failed`                   | Rust     | Form or list                | The provider settings were not saved. The previous settings are unchanged. Try again.                                                                                                                           | 服务商配置未保存，之前的内容保持不变。请重试。                                                                                        |
| `operation_failed`               | Rust     | Form or list                | The operation could not finish and its result is unknown. Refresh the list before trying again.                                                                                                                 | 操作未能完成，结果未知。请先刷新列表确认后再重试。                                                                                    |
| `invalid_stored_provider`        | Rust     | Form or list                | A saved provider configuration is not recognized, possibly from a newer vibemate version. It was left unchanged.                                                                                                | 已保存的服务商配置无法识别，可能来自更新版本的 vibemate。数据未被修改。                                                               |
| `not_found`                      | Rust     | Form or list                | This provider configuration was not found. Refresh the list.                                                                                                                                                    | 找不到这个服务商配置，它可能已不存在。请刷新列表。                                                                                    |
| `revision_conflict`              | Rust     | Form or list                | This configuration was changed elsewhere. Reload it before editing.                                                                                                                                             | 该配置已在其他地方被修改。请重新加载后再编辑。                                                                                        |
| `invalid_request`                | Rust     | Form or list                | The request is invalid. Refresh and try again.                                                                                                                                                                  | 请求无效，请刷新后重试。                                                                                                              |
| `kind_not_supported`             | Rust     | Form                        | This provider is not supported.                                                                                                                                                                                 | 不支持这个服务商。                                                                                                                    |
| `protocol_not_supported`         | Rust     | Protocol                    | The selected protocol is not supported for this provider yet.                                                                                                                                                   | 这个服务商暂不支持所选协议。                                                                                                          |
| `display_name_invalid`           | Rust     | Name                        | Enter a name of 1–64 visible characters without line breaks, control characters or invisible formatting characters.                                                                                             | 名称须为 1–64 个可见字符，不能包含换行、控制字符或不可见的格式字符。                                                                  |
| `base_url_invalid`               | Rust     | Base URL                    | Enter a valid base URL without spaces, a query (?) or fragment (#), up to 2048 characters.                                                                                                                      | 请输入有效的基础 URL，不能包含空格、查询参数（?）或片段（#），长度不超过 2048。                                                       |
| `base_url_not_https`             | Rust     | Base URL                    | The base URL must start with https://.                                                                                                                                                                          | 基础 URL 必须以 https:// 开头。                                                                                                       |
| `base_url_has_credentials`       | Rust     | Base URL                    | The base URL must not contain a user name or password.                                                                                                                                                          | 基础 URL 不能包含用户名或密码。                                                                                                       |
| `extension_field_not_supported`  | Rust     | Form or list                | The settings include an extension field that is not supported yet. Nothing was saved.                                                                                                                           | 包含暂不支持的扩展字段，未保存。                                                                                                      |
| `invalid_response`               | Frontend | Form or list                | The app received an unrecognized response, so the result is unknown. Refresh to check.                                                                                                                          | 收到无法识别的响应，结果未知。请刷新后确认。                                                                                          |
| `desktop_required`               | Frontend | Form or list                | Settings cannot be saved in the browser preview. Use the vibemate desktop app.                                                                                                                                  | 浏览器预览无法保存配置，请在 vibemate 桌面应用中操作。                                                                                |
| `secret_invalid`                 | Rust     | API key field               | Enter the API key. It must not contain line breaks or other control characters, and must fit the system credential store's size limit.                                                                          | 请输入 API 密钥。密钥不能包含换行或其他控制字符，长度不能超过系统凭据库的上限。                                                       |
| `credential_store_unavailable`   | Rust     | Top of form or Keys group   | No system credential store is available on this device (for example, no Secret Service on Linux). Nothing was saved, and vibemate never stores keys as plain text.                                              | 此设备没有可用的系统凭据库（例如 Linux 上没有运行 Secret Service）。什么都没有保存，vibemate 也不会以明文保存密钥。                   |
| `credential_store_access_denied` | Rust     | Top of form or Keys group   | vibemate could not access the system credential store. It may be locked or access was refused, and nothing was changed. Unlock it or allow access, then try again.                                              | 无法访问系统凭据库，它可能已锁定或拒绝了访问，什么都没有更改。请解锁或允许访问后重试。                                                |
| `credential_store_failed`        | Rust     | Top of form or Keys group   | The system credential store reported an error and nothing was changed. If you denied access in a system prompt, try again and allow it.                                                                         | 系统凭据库报告了错误，什么都没有更改。如果你在系统提示中拒绝了访问，请重试并选择允许。                                                |
| `create_outcome_unknown`         | Rust     | Form (then refresh list)    | vibemate could not confirm whether this configuration was saved, and an unused key entry may remain in the system credential store (vibemate never reads it). Refresh the list to check before adding it again. | 无法确认这项配置是否已保存，系统凭据库中可能留有一个未被使用的密钥条目（vibemate 不会读取它）。请先刷新列表确认，再决定是否重新添加。 |
| `secret_outcome_unknown`         | Rust     | Keys group (status unknown) | vibemate could not confirm whether the key was replaced; the system credential store may already hold the new key. Refresh the key status and replace it again if unsure.                                       | 无法确认密钥是否已替换，系统凭据库中可能已经是新密钥。请刷新密钥状态；如不确定，请再次替换。                                          |

Messages in the Keys group, by path (`*` = key-specific text; others are the
generic row above):

| Path    | Code                                                             | English                                                                                                  | 简体中文                                                                    |
| ------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Replace | `read_failed`\*                                                  | The key was not changed because the saved configuration could not be read. Try again.                    | 无法读取已保存的配置，密钥没有更改。请重试。                                |
| Replace | `write_failed`\*                                                 | The key was not changed, and the previous key status is unchanged. Try again.                            | 密钥没有更改，之前的密钥状态保持不变。请重试。                              |
| Replace | `storage_unavailable`\*                                          | Local configuration storage is unavailable, so the key was not changed. Restart vibemate.                | 本地配置存储不可用，密钥没有更改。请重启 vibemate。                         |
| Replace | `invalid_request`\*                                              | The request is invalid, so the key was not changed. Go back, refresh the list and try again.             | 请求无效，密钥没有更改。请返回并刷新列表后重试。                            |
| Replace | `not_found`\*                                                    | This provider configuration was not found, so its key was not changed. Go back and refresh the list.     | 找不到这个服务商配置，密钥没有更改。请返回并刷新列表。                      |
| Replace | `operation_failed`\* (unknown)                                   | The operation could not finish and its result is unknown. Refresh the key status before trying again.    | 操作未能完成，结果未知。请先刷新密钥状态确认后再重试。                      |
| Replace | `invalid_response`\* (unknown)                                   | The app received an unrecognized response, so the result is unknown. Refresh the key status to check.    | 收到无法识别的响应，结果未知。请刷新密钥状态确认。                          |
| Replace | `desktop_required`\*                                             | API keys cannot be changed in the browser preview, so nothing was changed. Use the vibemate desktop app. | 浏览器预览无法更改 API 密钥，什么都没有更改。请在 vibemate 桌面应用中操作。 |
| Replace | `secret_invalid`, `credential_store_*`, `secret_outcome_unknown` | Generic rows above (they already say nothing was changed, or that the result is unknown).                | 同上表通用文案。                                                            |
| Read    | `read_failed`\*                                                  | The key status could not be read. Try again.                                                             | 无法读取密钥状态，请重试。                                                  |
| Read    | `not_found`\*                                                    | This provider configuration was not found; it may no longer exist. Go back and refresh the list.         | 找不到这个服务商配置，它可能已不存在。请返回并刷新列表。                    |
| Read    | `operation_failed`\*                                             | The key status could not be read because the operation did not finish. Nothing was changed. Try again.   | 操作未能完成，无法读取密钥状态，什么都没有更改。请重试。                    |
| Read    | `invalid_response`\*                                             | The app received an unrecognized key status. Nothing was changed. Try again.                             | 收到无法识别的密钥状态，什么都没有更改。请重试。                            |
| Read    | `storage_unavailable`, `invalid_request`                         | Generic rows above.                                                                                      | 同上表通用文案。                                                            |

**Tests and verification.** Rust has 24 tests matching the `providers` filter
(validation, restart re-read, same-name instances, edits keeping identity, stale
and malformed edits, cursor pages with ties and concurrent inserts, invalid stored
rows, write failure, poisoned lock, SQL constraints, v4→v5 upgrade). UI tests
after P11: 39 in `src/lib/desktop/providers.test.ts` (P10: 29; adds the key in
the create request, strict response keys and the new codes), 29 in
`src/lib/desktop/providerSecrets.test.ts` (preview without IPC, request shapes,
malformed statuses, every code, no key in errors), 33 in `ProviderForm.test.tsx`
(P10: 21; key field attributes, empty-key check, `secret_invalid` by the field,
store codes at the top, `create_outcome_unknown` retry with a retyped key,
clearing after submit, cancel and hide, including an ignored Enter after an
unknown outcome and a second submit while saving, both as the property and the
mirrored `value` attribute, no key in text, callbacks or console), 32 in
`ProviderKeys.test.tsx` (set and missing in both languages, replace and
notification, blocked state, error placement and per-path wording for replace
and read failures, unknown outcome with refresh and a failed refresh, an ignored
submit during a refresh, read retry, preview, hide, late results, no key leaks),
27 in
`ProvidersView.test.tsx` (P10: 24; Keys group placement, replacing without saving
or leaving, clearing on hide and back), 4 in `ProviderIcon.test.tsx`, 7 in
`src/components/Notifications.test.tsx` (including a live region that holds only
the text), and provider navigation, the save
notification and key clearing across app pages in `src/App.test.tsx`. The
locked no-bundle macOS release build passed. Saving in the real Tauri runtime and
confirming the values after a restart is still a pending manual check; these
tests mock the desktop boundary and do not prove real WebView IPC.

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
