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
npm run check:frontend
```

This runs Prettier, ESLint with zero allowed warnings, TypeScript for both the
UI and Vite configuration, release-tool tests, and a production frontend build.
CI and the release workflow run the same command before continuing.

TypeScript enables strict, exact optional properties, and unchecked-index checks.
ESLint uses type-aware strict rules and React Hooks rules. It rejects unsafe
types, unhandled promises, and misplaced Tauri imports. Oxlint runs a dedicated
set of JSX accessibility checks. This avoids holding ESLint on an unsupported
major version because of a JavaScript accessibility plugin's peer constraints.
Use compatible current stable versions for both tools.

ESLint checks TypeScript frontend/config files and Node release scripts.
Formatting is handled by Prettier, so style rules must not conflict with it.
Use the VS Code ESLint, Oxc, and Prettier extensions for feedback while editing.

## Manual review still matters

Automated checks do not enforce every architecture decision or prove a usable
interface. Review component responsibility, state transitions, runtime data
validation, comments, credentials, styling, and platform assumptions explicitly.
For UI changes, inspect loading/error/empty states, keyboard focus, long labels,
and resizing. For desktop behavior, also run the real Tauri app.

The static scaffold does not yet have interactive feature tests. Add a UI test
runner and behavior tests when forms or mutations are implemented. Keep those
tests focused on outcomes rather than component internals.

## Safe IPC example

`getAppInfo()` returns typed metadata or `null` for a browser-only preview.
It uses `invoke<unknown>()` and checks the actual fields before returning an
`AppInfo`. A TypeScript generic alone cannot check data received at runtime.
More complex commands will need structured success/error results and documented
payload schemas shared with Rust. Do not expose credentials in error messages.
