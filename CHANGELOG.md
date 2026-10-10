# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Changed

- Simplified Chinese now uses one term set throughout the interface: 服务商, Agent, 技能 and MCP 服务器. The overview no longer mixes in "Provider". English text is unchanged; brand names, model IDs and URLs stay untranslated. Unused strings from the former landing page were removed.
- Application metadata and repository-opening failures now reach the interface only as stable codes (`invalid_response`, `operation_failed`, `open_failed`); raw runtime errors are discarded at the desktop boundary.
- Replaced the scope landing page with a desktop shell: fixed 200/88 px sidebar, Overview, honest planned feature pages, and bottom-aligned Settings. Panels stay mounted across navigation to preserve pending operations and input.
- On macOS, used an overlay title bar with native traffic lights in the sidebar and blank 44 px drag strips. Windows/Linux builds use undecorated windows with in-app window controls, including during startup.
- Replaced the Tauri icon with the forest/sage V and reference lettering. Expanded branding combines the V and wordmark; collapsed branding uses the V alone. Brand hover decoration is removed while keyboard focus remains. The raised orange i dot has clearance above its stem, with the original reference dot cleared.
- Added a separate macOS icon master with 85% artwork scale, transparent margins, continuous corners and a slight downward V offset. Other platform masters retain their dimensions.
- Reworked General into grouped rows for language, brightness and six theme circles ordered Forest, Ink (纸墨), Graphite, Linen, Iris, Ocean. Native radios preserve keyboard selection; rings and checks mark the choice. Saves disable controls without flashing saving or success text; failure and reload feedback remains.
- Reworked About to match General's width, with the expanded logo, description and equal-height metadata rows. GitHub is a bare icon with visible focus and retryable browser-opening feedback. Its Octicons MIT notice is bundled.
- Stopped Vite from inlining assets as CSP-blocked `data:` URIs.
- Recorded the maintainer's 2026-10-10 closure of the basic appearance iteration. Visual checks are manual unless requested; automated checks and native behavior verification remain separate.

- Standardized four-space indentation via `package.json` Prettier options and trimmed redundant editor config files; Zed project settings cover ESLint, Rust, tasks, and the pnpm lockfile workaround (`docs/frontend.md`).

- Raised the minimum Node.js version to 24.15 for the jsdom test environment.

- Migrated development, CI, desktop commands, and release validation to pinned pnpm with a frozen dependency lockfile. Tauri API 2.12.1 and Vite 8.3.3 satisfy pnpm's default release-age policy.

### Added

- Added a Logs group under Settings → General with actual, selectable log-file and folder paths, View logs, Open log folder and Refresh log paths. Rust opens only its fixed log file (TextEdit on macOS, Notepad on Windows, default association on Linux) or the containing folder; no arbitrary path/executable API or plugin permission is exposed. English/Chinese feedback covers browser preview, unavailable paths, startup file-logging fallback and failed requests, with duplicate prevention and retry. Native tool display remains a manual verification step.

- Added Rust local logging for startup, storage availability and preference/provider command outcomes. Logs use stderr and an approximately 1 MiB rolling file with three archives in the platform application-log directory; an unavailable file falls back to stderr without blocking startup. Only the exact application target is accepted, and command logs contain fixed operation names and safe error enum codes, not keys, URLs, configuration values or response payloads. No frontend logging permission or telemetry is added. See [logging behavior and safety rules](docs/plans/logging.md).

- Added the Providers page for saving provider configurations (name, base URL and protocol; API keys are covered by the next entry). Users create instances from the Command Code, DeepSeek and OpenRouter templates, including several of the same kind with repeated names, and edit them by stable ID; the kind cannot change. Rust validates names (1–64 characters with visible text; control characters, zero-width and bidi format characters are rejected), HTTPS base URLs (no credentials, query, fragment, inner whitespace or control characters; normalized host and one trailing slash removed; loopback and private hosts stay allowed until P13) and the protocol (Chat Completions only for now), and rejects every extension field until evidence documents one. Schema v5 stores instances with random IDs, cursor pages ordered by creation time then ID (1–100 per page), and revision checks that refuse stale edits, including an edit that loses a race with another writer. Errors are stable codes with English and Chinese messages; unknown save outcomes re-read the list before a retry, and browser preview never claims to save. The page lists configurations as rows (name first, then provider, protocol and URL, then a short ID) with a single primary "New configuration" action (plus icon) and a pencil icon on each row's "Edit"; rows show the official Command Code, DeepSeek or OpenRouter icon before the name (OpenRouter uses its light or dark file to match the theme); "New" opens the form with the provider as its first field (the first available provider is preselected with its defaults, and switching provider keeps values the user changed), and "Edit" opens a detail page whose "Basic information" group holds the name, base URL and protocol, with the provider read-only. A successful save returns to the list, moves focus to the saved row and shows a brief notification in the top-right corner that dismisses itself, pauses while hovered or focused, and can be closed. Saved settings are not used for connections or agent configuration yet. Verified with 27 Rust `providers` tests (60 Rust tests in total; the OS-keychain smoke test remains ignored), 208 UI tests, `check:frontend`, Rust fmt/Clippy and a locked no-bundle macOS release build; saving and restarting in the real desktop runtime is still a pending manual check.
- Added API keys to provider configurations. The create form now requires an "API key" (password field) that is saved together with the name, base URL and protocol; the list never shows keys or a "needs a key" state. The detail page has a separate "Keys" group that shows whether a key is set and when it was last updated (read from the app database only, so opening the page never prompts for the system credential store) and replaces it without saving the other settings or leaving the page; there is no clear action. Configurations saved before keys existed show "No key set" and a "Set key" button. The interface only checks that the key is not empty; Rust validates the rest. Invalid keys are reported next to the field, system credential store problems at the top of the form, an unconfirmed create re-reads the list (a retry needs the key typed again), and an unconfirmed replacement shows "Status unknown" with a refresh. The key lives only in the form's memory: it is cleared after every submit, on cancel, when leaving the page and while the page is hidden, and it never appears in messages, notifications or errors. Successful saves and replacements show the app notification. Browser preview reads no status and saves nothing. A failed replacement always says whether the key was changed, and a failed status read never suggests that anything changed. The notification's live region now contains only the message, so screen readers no longer read its close button with it. Verified with 296 UI tests, `check:frontend`, Rust fmt/Clippy and 92 passing Rust tests (one ignored OS-keychain smoke test); real desktop runs on macOS, Windows and Linux are still pending manual checks.
- Added `pnpm run check:i18n`, run by `check:frontend` in CI and release checks. It reports missing or extra keys, empty or non-string values, mismatched, malformed or unnamed interpolation parameters (including unescaped `{{- name}}`), plural forms missing for a locale's `Intl.PluralRules` categories, and not-yet-supported ordinal plural keys; `_zero` forms are optional. The checker has its own passing and failing fixture tests (`test:i18n`).
- Added bilingual desktop-shell tests: saved and system Chinese startup, switching language through the real selector while keeping page, tab, collapsed sidebar, unsaved input and loaded metadata, English fallback for an untranslated Chinese entry, and translated startup, About and save failures with retry. Verified with 119 UI tests (18 in `src/App.test.tsx`), 14 translation-checker tests, eight release-tool tests and the full frontend check. The maintainer reviewed the bilingual interface by hand on 2026-10-10.
- Added six light/dark themes with independent system/light/dark brightness. Ink uses Notion-inspired neutral colors; its stored `notion` identity survives the name change. Rust atomically saves validated pairs in schema v4, with migration preservation and restart tests. Browser preview makes no persistence claim.
- Added About build metadata, MIT license and a fixed GitHub repository action. Metadata loads on first visit and remains mounted, with translated loading, preview, retry and error states. Tabs support arrows, Home and End. Rust accepts no arbitrary repository URL or executable.
- Verified the latest appearance work with 101 UI tests, eight release-tool tests, frontend checks, Rust fmt/Clippy and 33 passing tests (the existing OS-keychain smoke test remains ignored), numeric palette contrast checks and locked macOS builds with unsigned local app bundles. Final ICNS resources match the packaged icon. Actual GitHub browser opening, native Ink restart behavior, Windows/Linux runtime checks and comprehensive accessibility testing remain pending. Earlier macOS metadata and dark/Iris restart checks are recorded in `docs/frontend.md`.

- Connected the language selector to a startup gate that reads saved preferences before mounting the app, offers safe failure/retry feedback, synchronizes HTML language, and preserves child input state when switching.

- Added language-preference persistence with a schema v2 migration, safe Rust error codes, and validated desktop IPC. Browser preview explicitly disables saving. Language selection and restart recovery were verified in the macOS desktop runtime using temporary app-data.

- Added the English and Simplified Chinese translation foundation with typed keys, system-language mapping, plural and Intl formatting, and fallback tests.

- Added a private SQLite configuration database in the app-data folder. It records a schema version and upgrades inside transactions. If it cannot be opened or upgraded, the app still opens, logs a safe reason, and leaves the file unchanged for later recovery.

- Added UI behavior tests for startup, preferences, About metadata, safe error feedback, browser preview and obsolete IPC responses. Frontend checks include shell navigation, collapse, settings preservation and drag-region containment.

- Documented Grok Build 1.0.50 configuration and toggle behavior, with a provider/agent capability matrix that separates configuration evidence from runtime verification.

- Documented Pi 1.1.0 configuration precedence, native MCP support, deployment toggles, and the home-screen enablement semantics.

- Recorded official provider protocols, model metadata, parameter constraints, and the first documented Pi compatibility path.

- Initial Rust, Tauri 2, React, and TypeScript desktop scaffold.
- Project rules, MIT license, contributor guide, and Chinese learning guide.
- Native CI checks for macOS, Windows, and Linux.
- A tag-triggered release workflow with version-specific changelog notes.
- Strict frontend rules, type-aware linting, accessibility checks, and validated desktop metadata.

Provider connections, agent injection, skill management, and MCP management
are not implemented yet.
