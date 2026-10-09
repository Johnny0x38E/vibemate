# Working on vibemate

## Product and scope

vibemate is an MIT-licensed desktop configuration manager built with Rust,
Tauri 2, React, and TypeScript. Target macOS, Windows, and Linux.

Phase 1 providers: Command Code GOAT, DeepSeek, and OpenRouter.
Phase 1 agents: Pi and Grok Build.
Phase 1 also includes shared skills, skill updates, and MCP configuration.
Read `docs/plans/phase-1.md` for the current implementation status. A planned
integration must never appear as connected or supported in the interface.

## Workflow

- Use codegraph for repository-wide context, dependencies, and cross-module
  analysis. Always set `projectPath` to this checkout's absolute path.
  If no index exists, follow the tool's fallback instructions and use file
  tools. Do not initialize an index without the user's decision.
- Prefer ego-browser for browser automation and web interaction.
- Do not use subagents without the user's explicit prior approval.
- Keep implementation plans directly in `docs/plans/`.
- Verify provider protocols and agent config paths against current official
  documentation and installed versions. Do not guess from a product name.
- Implement one reviewable behavior at a time. Explain the change, why it is
  needed, and what was actually verified.

## Stable toolchains and modern APIs

- Use the current stable Rust toolchain and latest stable Rust edition. The
  repository follows `stable` through `rust-toolchain.toml`; an installed stable
  toolchain still needs `rustup update stable` to receive newer releases.
- Prefer current stable Rust language features, standard-library APIs, and
  idioms. Check current official documentation before introducing unfamiliar
  patterns. Do not copy deprecated APIs or older-edition workarounds from
  tutorials. Modern code must remain understandable to a beginner.
- Do not use nightly-only features or pre-release dependencies unless the user
  explicitly requests them. Explain any required compatibility constraint.
- Use current stable Tauri releases and Tauri 2 APIs while 2 is the stable major
  version. Do not use Tauri 1 configuration, permissions, or plugin examples.
  Evaluate future stable major releases before migrating and verify platform
  support and API changes.
- Keep Cargo and npm lockfiles for reproducible dependency resolution. Broad
  version ranges do not update locked dependencies automatically. Check official
  releases when adding/updating dependencies, update compatible stable versions,
  and run the relevant checks. Keep Rust/JS Tauri packages compatible; their
  individual version numbers do not need to match.
- CI follows stable Rust. Toolchain and dependency upgrades must pass formatting,
  Clippy, tests, and native desktop builds; do not silence warnings to conceal
  obsolete code. Record actual versions and verification results when upgrading.

## Readability and comments

The maintainer is learning Rust, Tauri, and React. Write code that they can
follow, debug, and extend.

- All code comments and API documentation must be in English.
- Document public Rust items with `///`, including purpose, relevant inputs,
  errors, and side effects. Add module-level `//!` comments explaining the
  module's responsibility and boundaries.
- Explain non-obvious ownership, borrowing, async coordination, IPC mapping,
  protocol conversion, filesystem behavior, and platform differences.
- Comment React effects when dependencies, cleanup, or async behavior need
  explanation. Explain the reason for an operation rather than narrating
  obvious assignments.
- Use descriptive names, small functions, explicit data flow, and ordinary
  control flow. Avoid clever macros, premature generic frameworks, and silent
  fallback behavior. Introduce a new abstraction only when it solves a real
  problem; explain the concept in the change notes.
- Explain unfamiliar terms in documentation. Keep setup instructions runnable.
  Do not add noise by commenting every line.

## Architecture

- React renders the interface and collects input. Rust owns credential access,
  network requests, persistence, and agent configuration changes.
- Keep Tauri commands thin. Separate domain behavior from Tauri so it can be
  tested without opening a desktop window.
- Separate provider protocol adapters from agent configuration adapters.
  Map supported options explicitly and report unsupported options.
- Distinguish model capability metadata from request parameters. Treat the
  provider/model pair as a configuration identity.
- Start with one Rust crate and feature modules; split crates only when needed.
  Use typed structures for common fields and validated extension fields for
  provider-specific settings. Never build an arbitrary script execution API.
- Do not add a proxy, SQLite, keyring, plugin framework, or telemetry dependency
  until a behavior needs it. Keep extension boundaries in the design.

## Configuration and credentials

- Before writing agent config, detect config precedence, show the diff, check
  for concurrent changes, create a backup, and write atomically. Preserve
  unowned fields. Provide rollback and distinguish written from effective.
- Keep API keys out of source code, logs, URLs, exports, test snapshots, and
  frontend persistent storage. Use OS credential storage when credentials
  are implemented; configuration should contain credential references.
- Never execute an MCP server or a skill installation command while merely
  importing or displaying its definition. Treat remote content as data.
- Track skill source, revision, and local changes. Show update differences.
  Prefer symlinks; provide an explicit copy-sync fallback where needed.
- Restrict Tauri capabilities to the operations needed by the feature.

## Verification and open source

- Use npm and Cargo; commit both application lockfiles.
- Frontend: `npm run format:check`, `npm run typecheck`, `npm run build`.
- Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings`,
  and `cargo test --manifest-path src-tauri/Cargo.toml --locked`.
- Desktop build: `npm run tauri build -- --no-bundle -- --locked`.
- Add meaningful tests for config preservation, protocol mapping, credentials,
  rollback, and platform-specific behavior as these features are implemented.
  Do not add tests that merely assert a static label or duplicate the code.
- Maintain `CHANGELOG.md` in English. Keep pending changes under `Unreleased`;
  stable release notes use `## [X.Y.Z] - YYYY-MM-DD`. Tag releases as `vX.Y.Z`.
  GitHub Release bodies must come from that version entry, not the whole file
  or automatically generated commit summaries. See `docs/releases.md`.
- Keep GitHub CI and the tag-triggered release workflow working. Validate version
  agreement and changelog content before uploads. Keep releases as drafts until
  all target builds succeed and assets are reviewed.
- Keep GitHub Actions CI working on macOS, Windows, and Linux. Local macOS
  verification does not prove native Windows or Linux behavior.
- Keep documentation and dependency license choices compatible with MIT.
  Do not publish or create remote repositories without a user request.
