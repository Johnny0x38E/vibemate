# vibemate

A desktop home for your coding agents' providers, models, skills, and MCP servers.
Built with Rust, Tauri 2, React, and TypeScript. Licensed under MIT.

## Status

The desktop app configures providers with OS-stored API keys, fetches and selects
models with aliases, manual entries, capability evidence and context/output/effort
metadata, and keeps central stdio/HTTP MCP definitions plus a central Skill library
with local import. It discovers six Agents read-only (Claude Code, Codex, OpenCode,
Pi, Grok Build, Hermes), reads CLI versions only with consent, and writes
configuration for Claude Code, Pi and Grok Build through a previewed, backed-up,
reversible path with paginated history, restore of any retained version and
confirmed deletion.

Discovery does not execute Agents or read their configuration contents. Versions
and documented capabilities keep explicit evidence and uncertainty. Saving an MCP
definition never starts its server. Skill installation, update and deployment,
MCP deployment, per-Agent request settings for Claude Code and Grok Build, and
inference verification remain planned.

The first release targets **Command Code, DeepSeek, and OpenRouter** with
**Claude Code, Pi, and Grok Build** configuration adapters in that order. macOS,
Windows, and Linux are the target platforms.

Third-party names and logos are trademarks of their respective owners and are used only to identify them.

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

- [`docs/README.md`](docs/README.md): documentation index, separated into
  current reference pages and remaining plans.
- [`AGENTS.md`](AGENTS.md): coding rules, English comments, and verification.
- [`src/AGENTS.md`](src/AGENTS.md): strict frontend rules.
- [`docs/frontend.md`](docs/frontend.md): React/Tauri boundaries and automated checks.
- [`docs/architecture.md`](docs/architecture.md): Rust responsibilities and extension points.
- [`docs/plans/phase-1.md`](docs/plans/phase-1.md): scope, delivered baseline and remaining work order.
- [`docs/plans/todo.md`](docs/plans/todo.md): the single task-status source.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): local checks and contribution expectations.

React uses typed, validated IPC wrappers. Rust owns configuration persistence,
credentials and network access. Backend commands are grouped by feature under
`src-tauri/src/commands/`; Provider and MCP internals separate types, validation,
SQLite repositories and credential save/cleanup services. Saved models separate
queries from edit transactions; model discovery separates registration, downloads,
browse caching, snapshots and merging. Modules use
`feature.rs` with a matching directory.
Remaining verification, planned features and native/platform acceptance are
recorded in [`docs/plans/todo.md`](docs/plans/todo.md).

GitHub Actions runs frontend checks and native Rust checks/builds on all three
platforms. Installer builds are defined in `.github/workflows/release.yml`: a stable version
tag creates a draft with notes from `CHANGELOG.md`. See
[`docs/releases.md`](docs/releases.md). OS signing is not configured yet.
