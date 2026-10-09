# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Changed

- Replaced the scope landing page with the desktop shell: a collapsible 188/88 px sidebar with the app icon and an SVG "vibemate" wordmark, an Overview destination, honest planned pages, a relationship home with reserved statistics, and Settings holding the language selector and an icon-only appearance cycle. The runtime status line was removed. Settings stays mounted so pending saves and input survive navigation.
- On macOS, removed the separate native title-bar row with Tauri's overlay title bar while keeping the native traffic lights inside the sidebar. Unpainted 44 px top strips drag the window; the capability adds only `core:window:allow-start-dragging`. Verified with macOS release-build screenshots and maintainer checks (maximize, minimize, drag, edge resize, double-click zoom, fullscreen, startup-gate controls/drag, long English labels, and bilingual shell navigation). Windows/Linux window controls are not yet verified or implemented.
- Restyled the language selector to match the 44 px controls, and stopped Vite from inlining assets as `data:` URIs, which the CSP blocks.

- Established a sketch-based desktop-shell design baseline with a local light/dark preview, collapsible navigation, and an icon-only appearance cycle.

- Standardized supported source and configuration indentation on four spaces, moved Prettier options into package.json, and removed redundant EditorConfig, standalone Prettier, and Tauri-local Git ignore files. Kept all Zed tasks while limiting project editor settings to project-specific integration.

- Scoped the Zed multi-document YAML workaround to the generated pnpm lockfile and kept private learning documents outside shared checks.
- Raised the minimum Node.js version to 24.15 for the jsdom test environment.

- Migrated development, CI, desktop commands, and release validation to pinned pnpm with a frozen dependency lockfile. Tauri API 2.12.1 and Vite 8.3.3 satisfy pnpm's default release-age policy.

- Replaced editor extension recommendations with shared Zed settings, development tasks, and setup instructions.

### Added

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
