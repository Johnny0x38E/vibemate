# Frontend development

The frontend is React + TypeScript, rendered inside Tauri's system WebView.
React describes the interface; Rust handles local system and provider operations.
`src/AGENTS.md` contains the frontend implementation rules.

## Current code and planned layout

- `src/main.tsx` locates the HTML root and mounts React in StrictMode, with
  `LocaleStartup` withholding the App until its language preference is ready.
- `src/App.tsx` composes the desktop shell: the collapsible 188/88 px sidebar
  (brand, Overview, four feature destinations, Settings with the icon-only
  collapse toggle), the relationship home, planned pages, and a Settings section
  that stays mounted while hidden so pending saves and input survive navigation.
- `src/components/WindowDragRegion.tsx` marks unpainted strips that drag the
  window (`data-tauri-drag-region`, permission `core:window:allow-start-dragging`).
  Never place buttons or fields inside one; an App test enforces this.
- `src/components/BrandWordmark.tsx` draws the "vibemate" wordmark as SVG paths,
  so the brand needs no font file. `src/components/Icon.tsx` holds UI icons.
- `src/lib/desktop.ts` checks runtime availability, calls Rust, and validates
  the returned data. Components never import Tauri APIs directly.
- `src/App.css` contains shared design variables and app-scoped scaffold styles.

On macOS the window uses Tauri's overlay title bar (`titleBarStyle: "Overlay"`,
`hiddenTitle`, `trafficLightPosition` in `tauri.conf.json`): the native traffic
lights stay, but there is no separate title-bar row. The top 44 px of the sidebar
and of the content column are drag regions centered on the traffic-light row.
Windows and Linux still use native decorations until their own controls exist.
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

Open the repository root in Zed and run `pnpm install --frozen-lockfile` before editing the frontend.
The project supports Node.js 24.15+ on the Node 24 line, or Node.js 26+. Zed has native Rust, TypeScript, and
TSX support; no separate extension is required for those languages.

The shared `.zed/settings.json` contains only project-specific ESLint language
server entries, Rust project discovery/Clippy, and the lockfile type workaround.
The `...` entry preserves Zed's other language servers, including its default
TypeScript server. Save-time formatting and formatter preferences come from
Zed's user settings or defaults; the repository does not duplicate personal
preferences such as turning Rust format-on-save off.

The `prettier` field in `package.json` sets `tabWidth: 4` for frontend code,
configuration, and supported document formats, so CLI checks do not depend on
anyone's global editor settings. Other Prettier defaults remain unchanged,
including LF line endings and the 80-column print width. Separate `.editorconfig`
and `.prettierrc.json` files are not needed. Editor indentation while typing comes
from user settings or defaults; formatting enforces the repository's four spaces.
ESLint reads `eslint.config.mjs`; its rules and Oxlint accessibility checks remain
separate from formatting.

The explicit `src-tauri/Cargo.toml` path identifies the backend crate, and
rust-analyzer runs Clippy on save. `rust-toolchain.toml` selects stable Rust with
rustfmt and Clippy. Rust source formatting is owned by rustfmt, not Prettier;
generated lockfiles retain their tool-generated layout.

Run `task: spawn` from Zed's command palette and select a `vibemate:` task to
start desktop development, preview the frontend, or run the project checks.
Tasks run from the repository root, even when a backend file is active.
The frontend preview does not provide native Tauri operations.

Editor diagnostics do not replace `pnpm run check:frontend`: the dedicated
Oxlint accessibility checks and release-tool tests still run through that command.
Keep themes, fonts, keybindings, and personal AI settings in Zed's user settings.
Commit shared project settings and tasks so contributors use the same commands.

See the official [Rust](https://zed.dev/docs/languages/rust),
[TypeScript](https://zed.dev/docs/languages/typescript),
[language configuration](https://zed.dev/docs/configuring-languages), and
[tasks](https://zed.dev/docs/tasks) documentation for editor behavior.

The pnpm 12 lockfile contains separate YAML documents for the package manager
and application. Zed's YAML diagnostics can reject this generated format, so
`.zed/settings.json` reads only `pnpm-lock.yaml` as Plain Text. Other YAML files
keep their normal language support. Validate the lockfile with
`pnpm install --frozen-lockfile`; do not merge or reformat its documents manually.

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

`getAppInfo()` returns typed metadata or `null` for a browser-only preview. The
shell no longer shows this metadata; it is kept for the planned Settings "About"
section (todo I02.d).
It uses `invoke<unknown>()` and checks the actual fields before returning an
`AppInfo`. A TypeScript generic alone cannot check data received at runtime.
More complex commands will need structured success/error results and documented
payload schemas shared with Rust. Do not expose credentials in error messages.
