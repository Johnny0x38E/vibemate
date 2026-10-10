# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Changed

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

- Added six light/dark themes with independent system/light/dark brightness. Ink uses Notion-inspired neutral colors; its stored `notion` identity survives the name change. Rust atomically saves validated pairs in schema v4, with migration preservation and restart tests. Browser preview makes no persistence claim.
- Added About build metadata, MIT license and a fixed GitHub repository action. Metadata loads on first visit and remains mounted, with translated loading, preview, retry and error states. Tabs support arrows, Home and End. Rust accepts no arbitrary repository URL or executable.
- Verified the latest appearance work with 101 UI tests, eight release-tool tests, frontend checks, Rust fmt/Clippy and 33 passing tests (the existing OS-keychain smoke test remains ignored), numeric palette contrast checks and locked macOS builds with unsigned local app bundles. Final ICNS resources match the packaged icon. Actual GitHub browser opening, native Ink restart behavior, Windows/Linux runtime checks and comprehensive accessibility testing remain pending. Earlier macOS metadata and dark/Iris restart checks are recorded in `docs/frontend.md`.

- Connected the language selector to a startup gate that reads saved preferences before mounting the app, offers safe failure/retry feedback, synchronizes HTML language, and preserves child input state when switching.

- Added language-preference persistence with a schema v2 migration, safe Rust error codes, and validated desktop IPC. Browser preview explicitly disables saving. Language selection and restart recovery were verified in the macOS desktop runtime using temporary app-data.

- Added the English and Simplified Chinese translation foundation with typed keys, system-language mapping, plural and Intl formatting, and fallback tests. Existing page translation remains pending.

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
