# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Changed

- Smoothed the macOS icon tile with continuous corner curves and a broader corner transition, keeping its existing Dock footprint and logo artwork.

- Reduced the macOS Dock icon artwork to 85% of its previous size with transparent padding, preserving the selected V shape and colors. Added a separate macOS SVG master so other platform icons retain their existing dimensions.

- Replaced the Color theme dropdown with five clickable palette circles shown together. Native radio inputs preserve keyboard selection; a ring and check mark identify the active choice. Colors reuse the theme tokens, while saved-choice confirmation, preview, disabled states, and failure/reload recovery are unchanged. Visual acceptance is left to the maintainer.

- Combined the reference V and actual reference lettering into one expanded SVG logo, with a symbol-only collapsed version. Removed brand hover decoration while preserving keyboard focus and return-home behavior. Corrected the V proportions and lower turn against the maintainer's selected reference, kept its forest/sage colors, and regenerated desktop icons on an off-white tile. The i dot is a raised warm orange accent.

- Replaced the default Tauri icon with vibemate's selected V identity: editable SVG masters, regenerated desktop PNG/ICO/ICNS assets, and a rounded sidebar wordmark without the old yellow dot. The app tile keeps the symbol visible in light and dark appearance.
- Verified branding with full frontend checks (80 UI and eight release-tool tests), 720×560 light/dark and collapsed-sidebar ego-browser checks, the locked macOS desktop build, and an unsigned local app bundle with the expected ICNS. Windows/Linux native icon display remains pending.

- Replaced the scope landing page with the desktop shell: a collapsible 200/88 px sidebar (fixed widths, toggle only) with the app icon and an SVG "vibemate" wordmark, an Overview destination, honest planned pages, a relationship home with reserved statistics, and Settings with language and appearance controls. The runtime status line was removed. Settings and its tabs stay mounted while hidden so pending saves and input survive navigation.
- On macOS, removed the separate native title-bar row with Tauri's overlay title bar while keeping the native traffic lights inside the sidebar. Unpainted 44 px top strips drag the window.
- On Windows and Linux, disabled native decorations and added in-app minimize, maximize or restore, and close controls in the content title strip (including the startup gate).
- Reworked Settings: top tabs for General and About, a full-width grouped list row layout for Language and Appearance dropdowns, and no success or descriptive status text when changes apply immediately. Error, preview, and reload paths for language preference are unchanged. Light-mode row hover contrast was improved.
- Stopped Vite from inlining assets as `data:` URIs, which the CSP blocks.

- Established a sketch-based desktop-shell design baseline with a local light/dark preview and collapsible navigation (`docs/plans/desktop-shell-design.md`).

- Standardized four-space indentation via `package.json` Prettier options and trimmed redundant editor config files; Zed project settings cover ESLint, Rust, tasks, and the pnpm lockfile workaround (`docs/frontend.md`).

- Raised the minimum Node.js version to 24.15 for the jsdom test environment.

- Migrated development, CI, desktop commands, and release validation to pinned pnpm with a frozen dependency lockfile. Tauri API 2.12.1 and Vite 8.3.3 satisfy pnpm's default release-age policy.

### Added

- Added five built-in color themes (Forest, Graphite, Linen, Iris, Ocean), each with light/dark palettes. Settings General saves appearance and theme together through validated Rust IPC and schema v3 SQLite storage, restoring them after restart. Browser preview can try colors locally without claiming a saved preference; failures and uncertain saves have translated recovery paths.
- Verified themes with 95 UI tests and eight release-tool tests in frontend checks, 31 passing Rust tests (the existing OS-keychain smoke test remains ignored), fmt/Clippy, a locked macOS desktop build, and real native dark/Iris selection followed by restart using isolated app-data. All ten palettes were inspected at 720×560 and shared text/focus token contrast was measured. Windows/Linux runtime acceptance remains pending.

- Added Settings About with desktop build metadata, the MIT license, and a selectable GitHub repository address. Metadata loads on first visit and stays mounted across navigation. Browser preview does not invent a desktop version; failures have translated retry feedback. Settings tabs now support arrow keys, Home, and End.
- Verified About with 80 passing UI tests and eight release-tool tests in the full frontend checks, bilingual 720×560 ego-browser previews, and real metadata IPC returning 0.1.0 in the existing macOS debug runtime. Disabling read cleanup made both obsolete-request tests fail. Windows/Linux runtime checks remain pending.

- Connected the language selector to a startup gate that reads saved preferences before mounting the app, offers safe failure/retry feedback, synchronizes HTML language, and preserves child input state when switching.

- Added language-preference persistence with a schema v2 migration, safe Rust error codes, and validated desktop IPC. Browser preview explicitly disables saving. Language selection and restart recovery were verified in the macOS desktop runtime using temporary app-data.

- Added the English and Simplified Chinese translation foundation with typed keys, system-language mapping, plural and Intl formatting, and fallback tests. Existing page translation remains pending.

- Added a private SQLite configuration database in the app-data folder. It records a schema version and upgrades inside transactions. If it cannot be opened or upgraded, the app still opens, logs a safe reason, and leaves the file unchanged for later recovery.

- Added UI behavior tests for desktop metadata loading, safe error feedback, browser preview, and effect cleanup; included them in frontend checks. The metadata tests were later removed with the runtime status line; shell tests now cover navigation, collapse, settings preservation, and drag-region containment.

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
