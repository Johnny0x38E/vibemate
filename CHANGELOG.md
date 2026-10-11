# Changelog

User-visible changes are recorded here in English. Keep work in `Unreleased`
until a version is ready. Release headings use `## [X.Y.Z] - YYYY-MM-DD`.
GitHub release notes are extracted from the matching version section only.

## [Unreleased]

### Added

- Added read-only Pi and Grok Build entry discovery with a bilingual Agent page, explicit refresh, keyboard submission and window-local manual paths. Rust observes bounded desktop PATH / documented user entries and configuration-file presence without executing an entry or reading Agent config contents. Pi versions require matching package metadata and bin targets; Grok filename hints remain unverified. The page distinguishes entry presence, version provenance, documented capabilities, config-directory overrides and precedence without claiming a working installation or effective settings. Strict IPC validation, scan guards and lifecycle checks prevent unsafe responses and stale results. No dependencies, storage migrations, config writes or capability changes were added. Rust has 217 passing tests and one existing ignored OS-store test; full frontend checks (511 UI/boundary, eight release and 14 translation-checker tests), fmt/Clippy and locked Linux debug/release no-bundle builds passed. Isolated Tauri IPC and the actual Agent page passed with unchanged synthetic entry/config fixtures and no executable launch. Genuine Pi/Grok installations, macOS/Windows and visual acceptance remain pending.

- Added central MCP management with bilingual create/edit forms, stdio and Streamable HTTP definitions, bounded cursor pages and central enable/disable controls. All env/header values use OS credential storage; SQLite schema v8 stores metadata and references only. Revisions reject stale edits, immutable references preserve existing values after failed saves, and obsolete credential cleanup is retryable. Forms clear typed values on submit and navigation; uncertain outcomes require reload. Saving definitions never starts or contacts a server or writes Agent config. Agent deployment, OAuth, old SSE and imports remain deferred.

- MCP verification on Linux: full frontend checks (468 UI/boundary tests, eight release tests, 14 translation-checker tests), Rust fmt/Clippy and 201 passing tests (one real credential-store test ignored), plus locked debug/release no-bundle desktop builds. Isolated real Tauri save/edit/list/restart/read passed and the stdio test command did not execute. Native MCP credential-store acceptance, visual review and other-platform runtime checks remain pending. Current forms use the shared fixed bottom Back/Save footer.

- Added an expandable manual-model form in the Models tab’s added and upstream lists, with model ID, optional alias and an explicit "Add and save" action. Rust stores a selected manual row without a provider request; pending checkbox drafts block addition, and uncheck + Save removes manual rows through the existing atomic batch. Field errors retain editable input, duplicate submits are blocked, and unknown save outcomes require an exact-ID check across saved cursor pages before retrying. Successful additions update the selected baseline and count; hidden views ignore late replies. Verified on Linux with pinned pnpm 12.10.1: full frontend checks, 425 UI/boundary tests (15 new manual-form/list tests), 14 translation-checker tests and eight release tests. Real Tauri add/restart/delete, visual review and other-platform runtime verification remain pending.

- Provider edit pages use **API** and **Models** tabs (Settings-style keyboard navigation). API holds URL, protocol, and key; Models holds search, upstream browse, and draft selection. Both panels stay mounted while switching tabs.
- Provider **Models** tab (P12.c.5): default **Added** list (only persisted choices), **Save** for draft checkboxes, and **Fetch models** for lazy upstream browse without merging the full catalog until Save. Uncheck + Save removes rows from local storage.
- Rust commands `browse_upstream_models_page` and `save_provider_model_selections`, plus typed frontend wrappers in `models.ts`, with strict response validation and browser-preview guards.

### Changed

- Reorganized Rust modules without changing provider, model, or MCP behavior. Model IPC errors now live in `model_error`, and model-ID and route checks live in `model_identity`, so saved-model storage and vendor parsing no longer own each other's rules. Provider key status and replacement are part of the provider module. The model-list HTTP client is `model_http`. The desktop fetch command still composes the snapshot, download, and merge itself; tests keep an equivalent helper. Command names, error codes, SQL, lock order, credentials, and schema are unchanged.

- Standardized Chinese UI directory wording to “路径”, including Agent configuration/default/user paths and the MCP working-path field. Domain fields and filesystem behavior are unchanged.

- Split Agent configuration directory and file observations into their own detail card. Removed visible configuration-file/manual-path headings while retaining accessible section/form names, and set the gaps between all three detail cards to 10 px.

- Set Agent detail/manual-path card padding to 8 px vertically and 16 px horizontally, matching detail-row vertical spacing and other lists’ horizontal inset, with no extra nested inset on configuration-file rows.

- Standardized editable form rows to a shared 64 px minimum height with vertically centered labels, including Agent manual paths, Provider API, MCP and settings rows. Agent read-only detail rows match model-list rows with a 44 px minimum height and 8 px vertical padding. Wrapped values and expanded controls can grow without clipping.

- Simplified Agent detail status and version values into model-row-style capsule badges (“Found” / “已发现” and the version number). Moved discovery source and version evidence to hover/keyboard hints, keeping filename-derived versions explicitly unverified without the long inline description.

- Extended auxiliary icon buttons and hover/keyboard hints to MCP argument/env/header addition, Agent path rediscovery and log file/folder opens. Addition labels identify the row type; rediscovery retains form submission and blocked-action protection.

- Replaced MCP argument and env/header row removal text with compact trash-icon buttons, localized accessible names and hover/keyboard tooltips. Kept save, navigation and recovery labels visible; blocked removals remain focusable without changing data.

- Unified text-input focus with the model search field’s 2 px inset outline across Provider API, manual model, MCP and Agent path inputs. Focus color/width and non-input controls are unchanged.

- Standardized saved-model list wording as “added” / “已添加”, including counts, filters, search and empty/end messages. The upstream return action is now “View added models” / “查看已添加”, and upstream lists also offer first-row manual addition, covered by bilingual save-and-return tests. Checkbox actions and unsaved drafts retain selection wording.

- Aligned fixed footer button centers with sidebar Settings by sharing its row height and bottom inset. Standardized title/tab-to-content spacing to one 16 px token across module and secondary pages, removing additional scroll-wrapper top padding.

- Unified Provider create/edit, MCP create/edit and Agent detail pages with left-aligned titles and fixed bottom navigation: Back on the left and Save, where applicable, on the right. Reduced the title-to-content gap. Provider Models keeps its search/fetch toolbar and independent list scroll; selection Save moves to the footer. Lists and other modules keep their previous scrolling behavior. Frontend checks passed with 539 UI/boundary tests; browser/native visual verification is deferred to the maintainer at their request.

- Reworked the Agent module into a card list (official mark, name, version) with a detail view for paths and configuration-file presence, bilingual copy and shorter action labels (**Refresh**, **Details**, **Back to list**). Manual entry paths and rescan live on the detail view.

- Completed BR5 Models/Fetch modularization: kept the original public APIs while separating saved-model types/cursors/query conversion/edit transactions and fetch registration/cancellation, revision-aware browse caching, downloads, snapshot/status SQL, merge transactions and orchestration. Fetch no longer borrows `models::lock`. Existing SQL, IPC names/order, payloads/errors, schema v8, scheduling, lock scope, credential release, selected/manual retention and cancellation rules remain unchanged; frontend styles and interactions are untouched. Added five public-API HTTP-fixture regressions for browse reuse/search/pagination, raw API versus result offsets, revision/base-URL invalidation, failed-page retry and cancellation. All 202 existing test cases remain; Rust has 206 passing tests and one ignored OS-store test. Rust fmt/all-targets Clippy, full frontend checks and locked debug/release Linux desktop builds passed. Isolated native IPC verified model save/select/batch-delete, invalid cursor/missing-key error handling, registration release and persistence after restart. Real OS credential-store and other-platform acceptance remain separate.

- Completed backend modularity BR1–BR4: extracted eight modules' inline tests; grouped native commands by feature; separated Provider templates/types/validation/SQL/save orchestration and MCP types/validation/SQL/credential cleanup; shared only text checks, random-ID format and clock reads. MCP now owns its identity type. Modules use `feature.rs` plus a matching directory. All 29 command names/order, wire data, errors, schema v8, credential references, locks and compensation rules remain compatible; frontend behavior and capabilities are unchanged. The original Rust inventory still has 201 passing tests and one ignored real OS-store test. Rust fmt/Clippy, full frontend checks (468 UI/boundary, eight release and 14 translation-checker tests), and locked debug/release Linux desktop builds passed. Isolated Tauri IPC verified preferences, seeded no-key provider editing, manual-model selection/deletion/restart, missing-key fetch/cancellation and MCP save/edit/restart without executing its command. Real credential-store and other-platform acceptance remain pending. Updated architecture, implementation status and future integration boundaries.

- Reworked the Providers list into two-line rows (brand icon, protocol and selected-model pills, then display name) without base URL; returning from edit refreshes that row from storage.
- Provider create and edit share tabbed API / Models pages with a left-aligned brand title and fixed bottom Back/Save footer. Form Cancel is removed; Back is the navigation exit. Save and fetch outcomes use toned notifications with icons.
- Moved model **Fetch models** beside the search field (36 px toolbar row). Secondary provider pages use a shared 16 px title/tab-to-content gap.
- Provider **Models** tab scrolls inside the tab panel; upstream pages load on demand while browsing; end-of-list hints reflect added vs upstream browse state (English and Simplified Chinese).
- Upstream model search uses `browse_upstream_models_page` with a fuzzy `query` over the same session cache as the full browse list (ranked result pages for every vendor). Append loads show a footer status instead of a top loading banner that shifted the list.
- Maintainer verified the P12.c.5 Models tab flow (selected → upstream browse → save → deselect on selected) for Command Code, DeepSeek, and OpenRouter in Tauri on macOS (2026-10-11). Manual model add (P12.c.3) remains open.
- Main module pages share `PageModuleHeader` (sidebar nav icon, 1 rem title, 36 px minimum row height, optional trailing slot). Overview uses the nav label ("Overview" / 「概览」).
- Settings → General keeps logs in a separate titled section below language and appearance (unchanged behavior).
- Replaced the hand-written shared dropdown with Base UI Select 1.9.0 (MIT), keeping 36 px fields, provider icons, CSS Modules and theme tokens; menus are portaled with viewport-aware positioning.
- Settings → General logs show only the log file path with compact icon actions for View logs and Open log folder (Refresh log paths removed).
- The Providers list and edit form no longer display provider instance IDs; edit page titles use the brand icon and official name instead of the saved display name in quotes.
- Unified provider settings and API key editing under one Save button. Existing keys show only as a fixed mask and closed-eye indicator; an empty field preserves the key, a new value replaces it.

- Simplified Chinese now uses one term set throughout the interface: 服务商, Agent, 技能 and MCP 服务器. The overview no longer mixes in "Provider". English text is unchanged; brand names, model IDs and URLs stay untranslated. Unused strings from the former landing page were removed.
- Application metadata and repository-opening failures now reach the interface only as stable codes (`invalid_response`, `operation_failed`, `open_failed`); raw runtime errors are discarded at the desktop boundary.
- Replaced the scope landing page with a desktop shell: fixed 200/88 px sidebar, Overview, honest planned feature pages, and bottom-aligned Settings. Panels stay mounted across navigation to preserve pending operations and input.
- On macOS, used an overlay title bar with native traffic lights in the sidebar and blank 44 px drag strips. Windows/Linux builds use undecorated windows with in-app window controls, including during startup.
- Replaced the Tauri icon with the forest/sage V and reference lettering. Expanded branding combines the V and wordmark; collapsed branding uses the V alone. Brand hover decoration is removed while keyboard focus remains. The raised orange i dot has clearance above its stem, with the original reference dot cleared.
- Added a separate macOS icon master with 85% artwork scale, transparent margins, continuous corners and a slight downward V offset. Other platform masters retain their dimensions.
- Reworked General into grouped rows for language, brightness and six theme circles ordered Forest, Ink (纸墨), Graphite, Linen, Iris, Ocean. Native radios preserve keyboard selection; rings and checks mark the choice. Saves disable controls without flashing saving or success text; failure and reload feedback remains.
- Reworked About to match General's width, with the expanded logo, description and equal-height metadata rows. GitHub is a bare icon with visible focus and retryable browser-opening feedback. Its Octicons MIT notice is bundled.
- Stopped Vite from inlining assets as CSP-blocked `data:` URIs.
- Recorded the maintainer's 2026-10-10 closure of the basic appearance iteration. Visual checks are manual unless requested; automated checks and native behavior verification remain separate.

- Standardized four-space indentation via `package.json` Prettier options and trimmed redundant editor config files; Zed project settings cover ESLint, Rust, tasks, and the pnpm lockfile workaround (`docs/frontend.md`).

- Raised the minimum Node.js version to 24.15 for the jsdom test environment.

- Migrated development, CI, desktop commands, and release validation to pinned pnpm with a frozen dependency lockfile. Tauri API 2.12.1 and Vite 8.3.3 satisfy pnpm's default release-age policy.

- Added a Logs group under Settings → General with the log file path, View logs and Open log folder actions. Rust opens only its fixed log file or the containing folder; no arbitrary path API is exposed.

- Added Rust local logging for startup, storage availability and preference/provider command outcomes. Logs use stderr and an approximately 1 MiB rolling file with three archives in the platform application-log directory; an unavailable file falls back to stderr without blocking startup. Only the exact application target is accepted, and command logs contain fixed operation names and safe error enum codes, not keys, URLs, configuration values or response payloads. No frontend logging permission or telemetry is added. See [logging behavior and safety rules](docs/plans/logging.md).

- Added the Providers page for Command Code, DeepSeek and OpenRouter templates: validated fields, cursor paging, revision checks, bilingual error codes, and in-page create/edit. Saved settings are not used for connections or agent configuration yet.
- Added API keys in the OS credential store with SQLite references only; keys never appear in UI copy, responses or logs.
- Added `pnpm run check:i18n`, run by `check:frontend` in CI and release checks. It reports missing or extra keys, empty or non-string values, mismatched, malformed or unnamed interpolation parameters (including unescaped `{{- name}}`), plural forms missing for a locale's `Intl.PluralRules` categories, and not-yet-supported ordinal plural keys; `_zero` forms are optional. The checker has its own passing and failing fixture tests (`test:i18n`).
- Added bilingual desktop-shell tests: saved and system Chinese startup, language switching with preserved page state, English fallback for missing keys, and translated failure/retry flows.
- Added six light/dark themes with independent system/light/dark brightness. Ink uses Notion-inspired neutral colors; its stored `notion` identity survives the name change. Rust atomically saves validated pairs in schema v4, with migration preservation and restart tests. Browser preview makes no persistence claim.
- Added About build metadata, MIT license and a fixed GitHub repository action. Metadata loads on first visit and remains mounted, with translated loading, preview, retry and error states. Tabs support arrows, Home and End. Rust accepts no arbitrary repository URL or executable.
- Connected the language selector to a startup gate that reads saved preferences before mounting the app, offers safe failure/retry feedback, synchronizes HTML language, and preserves child input state when switching.

- Added language-preference persistence with a schema v2 migration, safe Rust error codes, and validated desktop IPC. Browser preview explicitly disables saving. Language selection and restart recovery were verified in the macOS desktop runtime using temporary app-data.

- Added the English and Simplified Chinese translation foundation with typed keys, system-language mapping, plural and Intl formatting, and fallback tests.

- Added a private SQLite configuration database in the app-data folder. It records a schema version and upgrades inside transactions. If it cannot be opened or upgraded, the app still opens, logs a safe reason, and leaves the file unchanged for later recovery.

- Added UI behavior tests for startup, preferences, About metadata, safe error feedback, browser preview and obsolete IPC responses. Frontend checks include shell navigation, collapse, settings preservation and drag-region containment.

- Documented Grok Build 1.0.50 configuration and toggle behavior, with a provider/agent capability matrix that separates configuration evidence from runtime verification.

- Documented Pi 1.1.0 configuration precedence, native MCP support, deployment toggles, and the home-screen enablement semantics.

- Recorded official provider protocols, model metadata, parameter constraints, and the first documented Pi compatibility path.

- Initial Rust, Tauri 2, React, and TypeScript desktop scaffold.
- Project rules, MIT license, contributor guide, and Chinese learning guide.
- Native CI checks for macOS, Windows, and Linux.
- A tag-triggered release workflow with version-specific changelog notes.
- Strict frontend rules, type-aware linting, accessibility checks, and validated desktop metadata.

Inference connections, Agent injection, skill management and MCP deployment
are not implemented yet.

### Fixed

- Sidebar module clicks now open each module home rather than its previous editor/detail page, including re-clicking the current module. Providers/MCP/Agents return to lists and Settings to General without remounting cached modules. Preserved model-draft cancellation, pending-write protection, MCP uncertain-outcome protection, Agent paths and Settings controls; late MCP edit reads cannot reopen the editor after returning home.

- Fixed the Agent rediscovery icon button being narrowed by an old text-button width override. Restored a left-label/right-control layout for the manual entry path, with the input and square rediscovery button kept together.

- Replaced the manual-model disclosure triangle with a plain plus/minus icon aligned with model rows. The icon has no checkbox-like box or selected-state fill, avoiding confusion with partial selection while retaining native disclosure keyboard behavior.

- Moved manual model addition into a dedicated first row in the added and upstream lists, with a left-aligned disclosure and inline model ID / optional alias inputs and save action. Removed visible labels and the explanatory paragraph while retaining accessible labels, draft blocking and recovery feedback.

- Added a fixed 16 px gap between scrolling secondary-page content and the bottom action-bar divider, without moving the footer buttons.

- Removed the extra horizontal inset from Provider API, MCP and Agent detail scroll content so their card edges match Settings and align with titles/tabs. Input and dropdown widths are unchanged.

- Removed the internal MCP server identifier from the edit form; neither create nor edit displays it. Creation still generates it automatically, and edits preserve the saved identifier when the display name changes.

- MCP list rows now show “Not registered” / 「未注册」 instead of a future-registration placeholder. Central enablement does not imply Agent registration; bilingual behavior tests cover this distinction.

- Fixed Agent pages staying visible under other modules when CSS module layouts overrode the HTML `hidden` attribute.

- Kept the TLS-failure test server open while draining client handshake bytes, preventing an early socket close from surfacing as a connection failure on Windows before rustls rejects the response. Production error classification and the strict TLS assertion are unchanged; regression tests cover wrapped TLS causes, transport errors and refused HTTPS connections.
