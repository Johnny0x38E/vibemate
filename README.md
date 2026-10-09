# vibemate

A desktop home for your coding agents' providers, models, skills, and MCP servers.
Built with Rust, Tauri 2, React, and TypeScript. Licensed under MIT.

## Status

This repository contains a runnable desktop scaffold and development rules.
Provider connections, agent injection, skill management, and MCP management
are planned and are not implemented yet.

The first release targets **Command Code GOAT, DeepSeek, and OpenRouter** with
**Pi and Grok Build**. macOS, Windows, and Linux are the target platforms.

## Start developing

Install Node.js 24 LTS (24.15 or newer), the current stable Rust through rustup, and the
[Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/).

The project follows stable Rust and uses edition 2024. Run `rustup update stable`
to refresh an existing installation before development. Tauri dependencies are
locked; compatible stable updates are validated before changing lockfiles.

Install pnpm 12.10.1 once if it is not already available:

```sh
npm install --global pnpm@12.10.1
```

The `packageManager` field fixes the version used by the project and CI.
Use pnpm for all project commands; npm is only used here to bootstrap pnpm.

```sh
pnpm install --frozen-lockfile
pnpm run tauri dev
```

For a frontend-only preview, run `pnpm run dev`. The UI explains that the Rust
runtime is unavailable in this mode. `pnpm run tauri build --no-bundle -- --locked`
builds a desktop executable without an installer.

## Find your way around

- [`AGENTS.md`](AGENTS.md): coding rules, English comments, and verification.
- [`src/AGENTS.md`](src/AGENTS.md): strict frontend rules.
- [`docs/frontend.md`](docs/frontend.md): React/Tauri boundaries and automated checks.
- [`docs/architecture.md`](docs/architecture.md): responsibilities and extension points.
- [`docs/plans/phase-1.md`](docs/plans/phase-1.md): scope, implementation order, and acceptance criteria.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): local checks and contribution expectations.

The initial screen makes one read-only call from React to Rust to display the
application version. It does not read or modify any agent configuration.

GitHub Actions runs frontend checks and native Rust checks/builds on all three
platforms after this repository is pushed to GitHub. Installer builds are defined in `.github/workflows/release.yml`: a stable version
tag creates a draft with notes from `CHANGELOG.md`. See
[`docs/releases.md`](docs/releases.md). OS signing is not configured yet.
