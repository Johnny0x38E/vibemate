# Architecture

## Implemented foundation

The app currently has one React screen and one read-only Tauri command.
`src/main.tsx` mounts React. `src/App.tsx` renders the initial scope.
`src/lib/desktop.ts` owns the frontend IPC wrapper.
`src-tauri/src/commands.rs` exposes version information.
`src-tauri/src/lib.rs` registers commands and starts Tauri.

Keep one Rust crate while learning the framework. Create feature modules as
behavior is implemented rather than adding empty abstractions now.

## Planned responsibilities

| Component           | Responsibility                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| Provider adapters   | Authentication, endpoints, model discovery, and protocol-specific fields        |
| Model configuration | Provider/model identity, capability metadata, and validated request settings    |
| Agent adapters      | Installed-version detection, config precedence, mapping, preview, and injection |
| Skill manager       | Central source tracking, revision updates, symlinks, and copy synchronization   |
| MCP manager         | Shared definitions, credential references, and agent-specific config mapping    |
| Persistence         | Versioned configuration schema and migrations; SQLite when persistence starts   |
| Credentials         | OS credential storage, accessed by Rust                                         |

React calls Rust commands; it should not contact providers or write config files
directly. Domain functions should be callable in tests without Tauri.
Provider and agent adapters have different responsibilities and should not be
combined into a provider-by-agent matrix of duplicated implementations.

Use schema-driven forms for common provider fields, plus typed adapter logic
where protocols differ. Preserve unknown agent config fields. Skills and MCP
have independent support per agent; an adapter must report those capabilities.

## Configuration flow

Read existing state → resolve precedence → validate supported options → preview
changes → recheck file state → back up → atomically write → verify → allow rollback.

A write may only take effect after an agent restarts. Report that clearly.
Environment variables and project-local overrides can supersede global config.

## Later extensions

An optional localhost proxy may support routing, protocol conversion, and usage
collection. Direct configuration is preferable when the agent supports the
provider protocol. Streaming, tool calls, reasoning, and images need explicit
compatibility tests before a conversion is marked supported.

A future usage-source boundary can accept provider quota APIs, agent logs, and
proxy usage events. Retain source identity and timestamps, distinguish measured
usage from estimates, and avoid duplicate counting. Quota monitoring and activity
heatmaps are outside phase 1.
