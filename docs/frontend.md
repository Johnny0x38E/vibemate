# Frontend development

The frontend is React + TypeScript, rendered inside Tauri's system WebView.
React describes the interface; Rust handles local system and provider operations.
`src/AGENTS.md` contains the frontend implementation rules.

## Current code and planned layout

- `src/main.tsx` locates the HTML root and mounts React in StrictMode.
- `src/App.tsx` renders the scope screen and reads application metadata.
- `src/lib/desktop.ts` checks runtime availability, calls Rust, and validates
  the returned data. Components never import Tauri APIs directly.
- `src/App.css` contains shared design variables and app-scoped scaffold styles.
- As functionality grows, use `src/features/providers/`, `src/features/agents/`,
  `src/features/skills/`, and `src/features/mcp/` for implemented features.
  Create these folders when needed, not as empty placeholders.

A component is a function that returns JSX, React's HTML-like UI notation.
Props are its typed inputs. State is data React keeps between renders; updating
it asks React to render again. An effect coordinates with an external system.
It should not be the default place for calculations or button-click actions.

The metadata effect in `App.tsx` ignores results after cleanup. Cleanup runs
when the component unmounts and during StrictMode's development lifecycle check.
Without that guard, a previous request could update an obsolete view.

## Changes must pass automated checks

```sh
pnpm run check:frontend
```

This runs Prettier, ESLint with zero allowed warnings, TypeScript for both the
UI and Vite configuration, release-tool tests, UI behavior tests, and a production frontend build.
CI and the release workflow run the same command before continuing.

TypeScript enables strict, exact optional properties, and unchecked-index checks.
ESLint uses type-aware strict rules and React Hooks rules. It rejects unsafe
types, unhandled promises, and misplaced Tauri imports. Oxlint runs a dedicated
set of JSX accessibility checks. This avoids holding ESLint on an unsupported
major version because of a JavaScript accessibility plugin's peer constraints.
Use compatible current stable versions for both tools.

ESLint checks TypeScript frontend/config files and Node release scripts.
Formatting is handled by Prettier, so style rules must not conflict with it.

## Editing in Zed

Open the repository root in Zed and run `pnpm install --frozen-lockfile` before editing the frontend.
The project supports Node.js 24.15+ on the Node 24 line, or Node.js 26+. Zed has native Rust, TypeScript, and
TSX support; no separate extension is required for those languages.

The shared `.zed/settings.json` enables Prettier formatting on save for frontend
files and ESLint diagnostics for TypeScript, TSX, and JavaScript. Prettier reads
`.prettierrc.json`; ESLint reads `eslint.config.mjs`. The `...` entry preserves
Zed's other language servers, including its default TypeScript server.
Rust files use the language server's formatter, backed by rustfmt. The explicit
`src-tauri/Cargo.toml` path identifies the backend crate, and rust-analyzer runs
Clippy on save. `rust-toolchain.toml` selects stable Rust with rustfmt and Clippy.

Run `task: spawn` from Zed's command palette and select a `vibemate:` task to
start desktop development, preview the frontend, or run the project checks.
Tasks run from the repository root, even when a backend file is active.
The frontend preview does not provide native Tauri operations.

Editor diagnostics do not replace `pnpm run check:frontend`: the dedicated
Oxlint accessibility checks and release-tool tests still run through that command.
Keep themes, fonts, keybindings, and personal AI settings in Zed's user settings.
Commit shared project settings and tasks so contributors use the same commands.

See the official [Rust](https://zed.dev/docs/languages/rust),
[TypeScript](https://zed.dev/docs/languages/typescript),
[language configuration](https://zed.dev/docs/configuring-languages), and
[tasks](https://zed.dev/docs/tasks) documentation for editor behavior.

The pnpm 12 lockfile contains separate YAML documents for the package manager
and application. Zed's YAML diagnostics can reject this generated format, so
`.zed/settings.json` reads only `pnpm-lock.yaml` as Plain Text. Other YAML files
keep their normal language support. Validate the lockfile with
`pnpm install --frozen-lockfile`; do not merge or reformat its documents manually.

## Internationalization is planned before business screens

Phase 1 supports Simplified Chinese and English. The current scaffold still
has English literals; the early i18n tasks in `docs/plans/todo.md` migrate them
before business forms are implemented. Add both translations, accessible names,
and error messages with every subsequent feature. Rust persists language
preferences and returns safe error codes; React translates user-facing messages.
Translation consistency checks will join `check:frontend` when i18n is implemented.

## Manual review still matters

Automated checks do not enforce every architecture decision or prove a usable
interface. Review component responsibility, state transitions, runtime data
validation, comments, credentials, styling, and platform assumptions explicitly.
For UI changes, inspect loading/error/empty states, keyboard focus, long labels,
and resizing. For desktop behavior, also run the real Tauri app.

## UI behavior tests

Run `pnpm run test:ui` once, or `pnpm run test:ui src/App.test.tsx` for the
metadata behavior. Vitest shares the Vite configuration and uses jsdom to supply
a DOM inside Node.js. React Testing Library renders the real component. DOM
Testing Library is an explicit peer dependency; all four test packages are MIT.
Vitest 5 supports this project's Vite 8 and React Testing Library supports React 19.
jsdom 30 requires Node 24.15 or newer on the supported Node 24 line.

Tests import Vitest APIs explicitly, so no test globals or relaxed lint rules are
needed. Both existing TypeScript configurations and strict ESLint remain in force.
Only `src/**/*.test.{ts,tsx}` runs in Vitest; release-tool tests retain Node's runner.
Cleanup is registered explicitly because Vitest globals are disabled.

`src/App.test.tsx` replaces only `src/lib/desktop` and verifies loading to ready,
safe failure feedback, browser-only preview, and stale success/failure responses.
The lifecycle cases use StrictMode's effect cleanup and restart: the first request
settles after the second, and must not replace the current visible status.
Temporarily disabling the cleanup guard caused both lifecycle cases to fail;
restoring the original source made all five cases pass.

These DOM tests do not verify Rust, the Tauri WebView, or provider connectivity.
Add behavior tests for new forms and mutations at their desktop boundary as they
are implemented; keep synthetic credentials out of production fixtures.

## Safe IPC example

`getAppInfo()` returns typed metadata or `null` for a browser-only preview.
It uses `invoke<unknown>()` and checks the actual fields before returning an
`AppInfo`. A TypeScript generic alone cannot check data received at runtime.
More complex commands will need structured success/error results and documented
payload schemas shared with Rust. Do not expose credentials in error messages.
