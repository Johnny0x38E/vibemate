# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Changed

- Migrated development, CI, desktop commands, and release validation to pinned pnpm with a frozen dependency lockfile.

- Replaced editor extension recommendations with shared Zed settings, development tasks, and setup instructions.

### Added

- Initial Rust, Tauri 2, React, and TypeScript desktop scaffold.
- Project rules, MIT license, contributor guide, and Chinese learning guide.
- Native CI checks for macOS, Windows, and Linux.
- A tag-triggered release workflow with version-specific changelog notes.
- Strict frontend rules, type-aware linting, accessibility checks, and validated desktop metadata.

Provider connections, agent injection, skill management, and MCP management
are not implemented yet.
