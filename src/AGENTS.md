# Frontend rules

These rules apply to `src/` in addition to the root `AGENTS.md`. The maintainer
is learning React and Tauri. Favor explicit code, clear English comments, and
small changes that can be understood without prior framework experience.
Read `docs/frontend.md` before introducing a new frontend pattern.

## Structure and responsibility

- Keep `App.tsx` focused on application composition and top-level layout.
  Put feature-specific views, hooks, types, and styles under
  `src/features/<feature>/` when that feature is implemented. Shared visual
  primitives belong in `src/components/` only after reuse is demonstrated.
- Keep each component responsible for one UI concept. Extract a component or
  hook when responsibilities diverge, not merely to meet a line-count target.
  Never put provider HTTP logic, config-file operations, or credentials in JSX.
- Use current stable React function components and hooks. No class components,
  legacy lifecycle APIs, direct DOM manipulation for React-managed UI, or
  premature memoization. Keep React StrictMode enabled.
- Prefer named exports for feature components and utilities. Default exports
  are acceptable for the existing app entry and framework configuration.
- Do not add routing, a global state library, a UI framework, or a data-fetching
  framework until a requirement demonstrates the need. Explain dependency
  choices and update lockfiles in the same change.

## Types and data

- Keep strict TypeScript enabled, including unchecked-index and exact-optional
  checks. No `any`, non-null assertions, double casts, or `@ts-ignore` to bypass
  a problem. Use `unknown` for external data and narrow it with validation.
- Declare exported function inputs/outputs and component props. Use descriptive
  domain types rather than untyped dictionaries or magic strings. Do not use
  enums or namespaces where a union or ordinary module is clearer.
- Treat IPC payloads as untrusted at runtime: `invoke<T>` is not a validator.
  Validate response shapes at the desktop boundary. Use a schema library when
  shapes become complex; do not duplicate a large validation framework.
- Keep server/domain validation authoritative in Rust. Frontend validation
  improves feedback but cannot replace backend validation.

## State, effects, and async behavior

- Keep state near the component that owns it. Derive values during rendering
  instead of duplicating them in state. Lift shared state only as far as needed.
- Use discriminated unions for workflows with loading, success, error, and
  submission states; do not permit contradictory booleans such as saving and
  saved at the same time. A simple display-only scaffold may use a string.
- Effects synchronize with external systems. Do not use effects to calculate
  ordinary render values, trigger user actions, or reset state unnecessarily.
  Follow the Rules of Hooks and exhaustive dependency checks; never disable
  them to force an effect to run once.
- Handle every promise rejection. A `void` prefix is acceptable only when the
  operation already handles errors. Prevent stale async results from updating
  state; clean up subscriptions, listeners, timers, and requests where supported.
  Explain cleanup and lifecycle decisions in English comments.
- Show loading, empty, error, success, and disabled states where applicable.
  Prevent duplicate destructive submissions. Provide actionable errors and a
  retry path when recovery is possible; never silently report success on failure.

## Tauri boundary and credentials

- Import `@tauri-apps/*` only in `src/lib/desktop.ts` or modules under
  `src/lib/desktop/`. UI code calls typed wrappers, never raw command names.
  This restriction is checked by ESLint.
- Network access, credential storage, filesystem access, process launch, and
  agent mutation belong in Rust. No direct provider fetches or secrets in
  browser storage, console output, hardcoded URLs, or frontend build variables.
- Return safe error messages and credential status/references to the UI.
  Forms may temporarily hold user-entered secrets only as needed; clear them
  after submission or cancellation and never persist them in frontend storage.
- Distinguish browser preview from the desktop runtime. Do not invent successful
  mutations or substitute mock provider connections in production.
- Adding a desktop command or plugin requires reviewing the Rust implementation,
  capabilities, CSP, and test coverage. Do not broaden permissions to bypass errors.

## UI, styles, and accessibility

- Use native buttons, inputs, labels, forms, headings, and links for their actual
  purpose. Do not make a `div` clickable or add ARIA to disguise incorrect HTML.
  Label every input and icon-only action; use stable domain IDs as list keys.
- All actions must work with a keyboard. Preserve visible focus, logical tab
  order, readable contrast, and screen-reader feedback for async results.
  Dialogs must manage focus, support Escape where appropriate, and restore focus.
- Do not indicate errors or connection state by color alone. Support long model
  IDs, URLs, translated labels, resizing, zoom, and empty/large collections.
- Use shared CSS variables for recurring colors, spacing, and radii. Use CSS
  Modules for new feature/component styles. Global CSS is limited to tokens,
  resets, and app-scoped layout. Avoid arbitrary z-index values, `!important`,
  broad element selectors, and repeated inline style objects.
- Keep responsive behavior usable at the minimum desktop size. Test actual
  screens with long data and keyboard navigation; linting cannot verify layout,
  contrast, or complete accessibility. Honor reduced-motion preferences when
  adding animation. Never add decorative motion that interferes with work.

## Internationalization

- Phase 1 must support Simplified Chinese (`zh-CN`) and English (`en`). Complete
  the i18n foundation before business screens. Do not hardcode user-facing text
  in JSX, validation messages, status feedback, or accessible names.
- Add both translations with each feature. Use stable keys grouped by feature,
  complete sentences, interpolation, locale-aware plural/date/number formatting,
  and matching placeholders. Do not translate model IDs, provider brands, URLs,
  config keys, or user content.
- Default to system language, allow manual choice, and persist preferences via
  Rust; no browser storage. Language switching must preserve input/state and
  synchronize HTML lang, accessible status, and localized window titles.
- Rust returns safe structured error codes and parameters; UI translates them.
  Do not display raw internal errors or unresolved translation keys.
- Include key/placeholder completeness checks and bilingual behavior tests in
  `check:frontend`. Inspect both languages, long content, and keyboard feedback.

## Comments, tests, and completion

- Document exported components/hooks/services in English: their responsibility,
  inputs, important state transitions, and relevant side effects. Explain
  unfamiliar framework concepts in `docs/frontend.md` with concrete examples.
- Add behavior tests for new forms, error/retry states, async races, and command
  interactions using user-visible behavior. Mock the desktop boundary in UI
  tests. Avoid implementation snapshots or assertions for static labels alone.
  Introduce the UI test runner when the first interactive behavior needs it.
- Run `pnpm run check:frontend` before completing frontend changes. CI and the
  release prepare job must run this command. No warnings are accepted by ESLint.
- For visible changes, inspect the browser preview using ego-browser and check
  keyboard behavior, long content, and minimum-width layout where relevant.
  Desktop mutations also require a Tauri runtime check; a web preview cannot
  prove native behavior.
- Keep CHANGELOG entries factual and document what was actually verified.
  Never claim lint proves UX quality or all platforms have been tested locally.
- Do not weaken a lint/type rule or add an inline suppression to make a check
  pass. Correct the cause. A justified exception requires a documented decision
  and explicit user approval before relaxing a project rule.
