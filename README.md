# vibemate

A desktop home for your coding agents' providers, models, skills, and MCP servers.
Built with Rust, Tauri 2, React, and TypeScript. Licensed under MIT.

## Status

The desktop app supports provider configuration with OS-stored API keys, model
fetching and selection, manual model entries, and central stdio/HTTP MCP
definitions. It also includes bilingual preferences, themes and local logging.
Agent deployment, skill management, MCP execution/deployment and inference
verification remain planned. Saving an MCP definition never starts its server.

The first release targets **Command Code, DeepSeek, and OpenRouter** with
**Pi and Grok Build**. macOS, Windows, and Linux are the target platforms.

Provider names and logos are trademarks of their respective owners and are used only to identify the providers.

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
pnpm tauri dev
```

For a frontend-only preview, run `pnpm dev`. The UI explains that the Rust
runtime is unavailable in this mode. `pnpm tauri build --no-bundle -- --locked`
builds a desktop executable without an installer.

## Find your way around

- [`AGENTS.md`](AGENTS.md): coding rules, English comments, and verification.
- [`src/AGENTS.md`](src/AGENTS.md): strict frontend rules.
- [`docs/frontend.md`](docs/frontend.md): React/Tauri boundaries and automated checks.
- [`docs/architecture.md`](docs/architecture.md): responsibilities and extension points.
- [`docs/plans/backend-modularity.md`](docs/plans/backend-modularity.md): backend module boundaries and incremental refactoring.
- [`docs/plans/phase-1.md`](docs/plans/phase-1.md): scope, implementation order, and acceptance criteria.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): local checks and contribution expectations.

React uses typed, validated IPC wrappers. Rust owns configuration persistence,
credentials and network access. Backend commands are grouped by feature under
`src-tauri/src/commands/`; Provider and MCP internals separate types, validation,
SQLite repositories and credential save/cleanup services. Modules use
`feature.rs` with a matching directory.
The app does not write Agent configuration yet. Implementation status and
remaining native/platform acceptance are recorded in
[`docs/plans/todo.md`](docs/plans/todo.md).

GitHub Actions runs frontend checks and native Rust checks/builds on all three
platforms after this repository is pushed to GitHub. Installer builds are defined in `.github/workflows/release.yml`: a stable version
tag creates a draft with notes from `CHANGELOG.md`. See
[`docs/releases.md`](docs/releases.md). OS signing is not configured yet.
