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

Install Node.js 24 LTS, the current stable Rust through rustup, and the
[Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/).

The project follows stable Rust and uses edition 2024. Run `rustup update stable`
to refresh an existing installation before development. Tauri dependencies are
locked; compatible stable updates are validated before changing lockfiles.

```sh
npm ci
npm run tauri dev
```

For a frontend-only preview, run `npm run dev`. The UI explains that the Rust
runtime is unavailable in this mode. `npm run tauri build -- --no-bundle -- --locked`
builds a desktop executable without an installer.

## Find your way around

- [`AGENTS.md`](AGENTS.md): coding rules, English comments, and verification.
- [`docs/getting-started.zh-CN.md`](docs/getting-started.zh-CN.md): a beginner's
  guide to Rust, Tauri, React, and the files in this repository.
- [`docs/architecture.md`](docs/architecture.md): responsibilities and extension points.
- [`docs/plans/phase-1.md`](docs/plans/phase-1.md): scope, implementation order, and acceptance criteria.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): local checks and contribution expectations.

The initial screen makes one read-only call from React to Rust to display the
application version. It does not read or modify any agent configuration.

GitHub Actions runs frontend checks and native Rust checks/builds on all three
platforms after this repository is pushed to GitHub. Release signing and
installer publication will be added separately.
