# Rust local logging foundation

## Scope

Use `log` 0.4.34 and the official `tauri-plugin-log` 2.10.0 sinks for local
Rust diagnostics. These stable releases are MIT-compatible. No telemetry,
frontend logging, embedded log viewer, provider request logging, or credential-store
inspection is included. Settings provides explicit actions to view logs in a local
text tool and open the containing directory.

Logging is initialized in desktop setup before private storage opens. It records
application start/version, storage readiness or a safe startup failure, preference
and provider command outcomes, desktop runtime failure, and normal exit. Command
failures include the fixed command name and the safe error enum variant, including
unknown-outcome errors. They do not change the result sent to the interface.
Failures are warnings; storage/runtime failures are errors. Successful commands
are debug events, not an audit trail or proof that an agent configuration is
effective. Debug builds accept debug and higher levels; release builds accept
info and higher levels. `RUST_LOG` does not override this policy.

## Output and retention

Logs go to stderr and `vibemate.log` in Tauri's `app_log_dir()`:

| Platform | Default directory                                                                           |
| -------- | ------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Logs/dev.vibemate.desktop/`                                                      |
| Windows  | `%LOCALAPPDATA%\dev.vibemate.desktop\logs\`                                                 |
| Linux    | `$XDG_DATA_HOME/dev.vibemate.desktop/logs/`, or `~/.local/share/dev.vibemate.desktop/logs/` |

The platform paths were checked against installed Tauri 2.12.2's path resolver.
Platform/environment overrides can change them. GUI launches may have no visible
terminal; the file is the primary diagnostic source then.

Each file rotates at approximately 1 MiB, with up to three archived files plus
one active file. The limit is checked between records, not a strict disk quota:
a single oversized record can exceed it. Existing unrelated files are not pruned.
Logs append across restarts. UTC timestamps, target and severity come from the
plugin's default formatter. Archives use timestamped filenames.

If resolving, creating or opening the file fails, initialization retries with
stderr only and emits `event=log_file_unavailable fallback=stderr`. A logger
installation failure gets a fixed stderr message. Neither failure prevents the
app from opening. Later write/rotation failures follow the official sink's
best-effort behavior; there is no live UI health indicator or automatic file retry.
Settings shows only whether file logging was enabled during startup.

The implementation does not coordinate logging across multiple app processes.
Concurrent instances writing or rotating the same file are outside this
foundation's guarantees.

## Safety and extension rules

- Only the exact `vibemate` log target is accepted. Dependency logs, including
  HTTP/TLS/proxy diagnostics, are excluded at every severity.
- Use fixed event/operation names. Never format request/result structs, keys,
  credential references, headers, URLs, config values, user labels or raw errors.
- The outcome helpers accept typed `ProviderError`/`SettingsError` enums with no
  user content. Their success payloads do not need `Debug` and are never inspected.
- Startup storage errors use only the safe `Display` text, never `Debug` or
  `Error::source()`. Raw Tauri startup errors are not logged.
- The exact-target filter is not a redactor: secrets passed explicitly to an
  allowed log call would still leak. New call sites require review.
- Use `split()` to obtain the official sinks without registering its IPC plugin.
  No WebView log target, logging JavaScript package, or plugin capability is added.
  Log access commands accept no paths or executables from the frontend.
- No custom panic hook is installed. Panic payloads are not copied into log files;
  Rust's existing stderr panic handling remains outside this logging API.
- Files use platform-default permissions and are not encrypted. Review logs
  before sharing them, even though the supported events exclude user data.

Example for a new safe backend lifecycle event:

```rust
log::info!(target: "vibemate", "event=configuration_service_ready");
```

## Settings log access

Settings → General includes a Logs group with the actual absolute log file and
directory paths, selectable for copying. It offers View logs, Open log folder and
Refresh log paths. Both languages include loading, browser-preview, stderr-fallback
and safe failure feedback. Pending operations block duplicate/competing requests
with `aria-disabled` while preserving button focus.

Rust resolves the directory during setup and stores it with the startup logger
outcome. The frontend reads metadata only; it never receives log contents and
never supplies a path or executable. The commands are:

- `get_log_location`: return actual paths and `fileLoggingActive`; no filesystem
  creation. Unrepresentable/non-UTF-8 paths are rejected, not lossily displayed.
- `open_log_file`: verify the fixed `vibemate.log` is a regular file, then request
  TextEdit on macOS, Notepad on Windows, or the default association on Linux.
- `open_log_directory`: verify the directory exists, then request the default
  system file manager. Archived logs can be browsed in that folder.

Opening uses the existing `tauri-plugin-opener` 2.7.0 Rust functions without
registering its IPC plugin or expanding capabilities/CSP. OS launching runs on
the blocking pool. Missing paths are not created, existing logs are never edited
by these commands, and an older file can still be opened after stderr fallback.
The external editor may allow editing; this is not a read-only viewer.

The opener uses detached launching: an acknowledgment means the request was
dispatched, not that the tool successfully displayed the log. Therefore the UI
does not announce that the file has been opened. Linux requires a usable default
association for `.log` files. No guessed editor commands, file-selection dialog,
arbitrary shell execution or automatic launching on read is introduced.

The startup active flag is not ongoing sink-health monitoring. Refresh reads the
configured paths and the same startup flag; it does not reinitialize logging.

## Verification

Five logging tests run without a native window using Tauri's test-only mock
runtime. They cover target filtering at all levels, unchanged command results
with non-Debug success payloads, append across restarts, excluded HTTP credential
messages, a blocked file path falling back to stderr, rotation/archive pruning,
and preservation of unrelated files.

Six log-access tests cover fixed-path dispatch, no creation on metadata reads or
missing-path opens, wrong file types, sanitized opener failures, startup fallback
status, and non-UTF-8 paths on Unix. Frontend tests cover validated path responses,
no-argument open requests, browser preview, both languages, duplicate prevention,
retry/focus behavior, stale results and retaining paths/pending actions across tab
and sidebar navigation. These tests never launch an external tool.

Local macOS verification passed with Rust 1.99.0, Node.js 26.3.0 and pnpm 12.10.1:

- Rust formatting and locked all-targets Clippy with warnings denied.
- Locked Rust tests after integrating main `4e4233f`: 174 passed, including five
  logging and six log-access tests; one real
  OS-credential-store smoke test remains intentionally ignored.
- `pnpm run check:frontend`: formatting, lint, 8 release-tool tests, 14 i18n-tool
  tests, translation validation, 327 UI tests, both TypeScript checks and build.
- `pnpm run tauri build --no-bundle -- --locked`: release executable built.

Clippy found a pre-existing modulo check in `models.rs`; it was replaced with
stable `usize::is_multiple_of(2)` without changing cursor validation behavior.

Native Windows/Linux runtime verification and manual inspection after an actual
GUI launch remain pending, including TextEdit/Finder and Windows/Linux editor and
file-manager dispatch. Mock-runtime tests and a local macOS build do not prove
those behaviors.

## Evidence

- [Official log plugin documentation](https://v2.tauri.app/plugin/log/)
- [Installed plugin API](https://docs.rs/tauri-plugin-log/2.10.0/tauri_plugin_log/)
- [Tauri application log directory](https://docs.rs/tauri/2.12.2/tauri/path/struct.PathResolver.html#method.app_log_dir)

The source of the installed stable plugin was also checked for `split`, sink
initialization failure, append behavior and `RotationStrategy::KeepSome` semantics.
