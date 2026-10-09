# Contributing

Read `AGENTS.md` and `docs/plans/phase-1.md` before changing the project.
Keep changes focused on a single behavior. Write comments and public API
documentation in English so contributors can follow the code.

## Local checks

```sh
npm ci
npm run format:check
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri build -- --no-bundle -- --locked
```

Use `npm run format` and `cargo fmt --manifest-path src-tauri/Cargo.toml` to
apply formatting. Add tests when new behavior can lose data, misroute requests,
or change agent configuration. Use synthetic credentials and temporary config
files in tests, never real API keys or your personal agent configuration.

Describe the resulting behavior, validation, and limitations in a pull request.
Cross-platform checks run in `.github/workflows/ci.yml`. CI does not publish
installers or change user configuration. Contributions are licensed under MIT.
