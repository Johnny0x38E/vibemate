# Model metadata and per-Agent request settings

This plan covers the remaining model information and request settings, and the
rules that decide where each value belongs. Pi already writes declared context,
maximum output, reasoning and input capability; Grok Build already writes declared
context. Per-Agent request-setting editing is not delivered yet. Implemented
model behavior is in [`../models.md`](../models.md); task status is in
[`todo.md`](todo.md).

## Confirmed responsibility split

| Location        | Responsibility                                                       |
| --------------- | -------------------------------------------------------------------- |
| Provider API    | Base URL, credential reference, protocol                             |
| Provider models | Fetch/add, alias, upstream capability evidence and source, limits    |
| Agent config    | Provider/model choice and the request options that Agent can express |
| Rust            | Check provider, model, protocol and Agent version, then build a plan |

The same model can have different settings in different Agents. Changing an
Agent's settings never changes central capability metadata or another Agent.
Unknown or manually declared capability is not verified support.

Context window, input modality, tool calls, reasoning support and maximum output
are **model facts**. Temperature, per-request output limit, reasoning effort or
reasoning token count are **request settings**. The model page owns upstream facts
and aliases; request settings belong to the target Agent's configuration flow,
including any Agent-local capability override.

Changing a capability label does not grant a capability. Parameters are validated
per Provider protocol and per Agent field. When there is no documented equivalence,
effort is not converted into a token budget. Supported raw extension fields are
stored; unknown or unsupported settings are refused with a reason.

`maxTokens` in a Pi model definition describes the model limit and is not a
per-request generation count. An Agent that cannot express a setting refuses it
rather than assuming a passthrough.

## Implementation order

The maintainer decided on 2026-10-20 to reuse the existing integration research
across minor Agent upgrades. A version change alone does not block implementation
or require a full documentation survey. Recheck a specific contract when there is
an observed conflict, a known deprecation or a failing behavior test. Unverified
settings are not silently presented as supported.

The first P24.b slice maps the saved context capacity for DeepSeek's documented
`deepseek-flash` ID without `[1m]` to
`env.CLAUDE_CODE_MAX_CONTEXT_TOKENS`. Missing capacity inherits the existing
configuration. Other IDs keep their existing window and report that this slice
cannot map them; compaction switches, model IDs and unrelated settings remain
untouched. Plan, detailed preview and apply must use the same stored model facts,
bind their digest, and preserve the normal snapshot/restore path. This slice does
not add local overrides, effort selection or request output settings.

This first slice passed mapping, skipped/invalid values, field preservation,
stale model metadata, apply/restore, strict IPC and translated-notice tests,
standard checks and an isolated macOS desktop preview/apply/restore. Current
acceptance and remaining slices are recorded in `todo.md`; the third slice below
adds an independently remembered starting choice. Additional effort/output
options, real inference and other platforms remain outstanding.

### Second slice: explicit Claude Code effort selection

For the documented DeepSeek `deepseek-flash` binding, offer only an explicitly
listed model effort that Claude Code can express (`low/medium/high/xhigh/max`),
and require declared reasoning support. Unknown, `any`, empty and unsupported
metadata do not invent choices. The model's declared default is displayed, not
selected automatically. Other bindings retain their existing behavior.

An optional selected effort belongs to the configuration request and is written
to `env.CLAUDE_CODE_EFFORT_LEVEL`, including `max`. The apply approval carries the
selection from the reviewed plan, and detailed previews rebuild that same choice.
Changing the selection clears the visible preview; switching Provider/model clears
the draft to prevent accidental reuse. Unset means **do not overwrite**: preserve
existing Agent settings, not reset them to the model default. The UI explains
that this native environment setting may affect other Claude Code models too.

The second slice persists the choice in the applied native configuration and
history, not as a separate remembered preference for every Provider/model pair.
The third slice below adds independent starting preferences in vibemate, without
claiming that a global native variable provides per-binding isolation.

Cover unsupported/unknown/any efforts, metadata changes after preview, selection
changes and switching, strict IPC, preservation, application and version restore.
Use the usual frontend/Rust/build checks and an isolated synthetic-credential
Tauri run; no model request is needed.

### Third slice: remember a Claude Code effort preference

Keep a non-secret starting choice in a Claude Code-specific SQLite table keyed
by Provider instance ID and exact upstream model ID. It is not model capability
metadata or a native per-model namespace. An explicit **Remember choice** action
saves it; selecting, previewing, applying or restoring native configuration does
not silently change it. Returning to a binding loads its saved draft. Config
approval still binds the explicit draft, not a live preference lookup.

A nullable effort means do not overwrite the native setting. Keep a fresh random
revision even when clearing an existing preference, so a stale window cannot
miss an intervening save-and-clear. Save compares both the preference revision
and the current model content digest in one immediate transaction. Validate
nonempty choices with the same adapter intersection as configuration planning.
Read returns current model metadata with the preference in one SQLite snapshot.
Missing rows do not write defaults; invalid data is an error, not inheritance.

Use a composite foreign key to the stored model, with delete cascade. Removing
a model or Provider removes its starting preference, without changing native
configuration or retained history. Merely unselecting a retained model row does
not change its preference. Do not extend these deletes into the broader P35
native-configuration reference policy in this slice.

The UI handles read/retry, save/failure/conflict and late replies after switching
bindings. An obsolete saved effort is shown explicitly as unmapped; it is not
silently downgraded or applied. The user can choose a supported effort or do not
overwrite. Saving discards the visible plan and requires preview again; it never
writes the Agent file or credentials. Preference changes in another window do
not rewrite an already-approved explicit configuration request. Model changes
still invalidate configuration approval through its existing digest.

Cover migration, restart persistence, account/model separation, compare-and-swap
races, save-and-clear, metadata changes, delete cascades, strict IPC and async UI
races. Run the normal frontend/Rust/desktop checks and isolated desktop IPC;
no real credentials or inference are needed. Task status stays only in `todo.md`.

## Evidence and gaps

The initial Provider evidence below came from public documentation. The
[2026-10-20 Agent field matrix](../integrations/model-parameter-mapping.md) also
checks installed Pi 1.3.0 schemas/implementation and Grok Build 1.0.62 bundled
manuals, with Claude Code 2.1.304 official documentation. It records native fields,
precedence, protocol restrictions and unresolved evidence. Existing evidence is
used for implementation; remaining checks are handled per field rather than
blocking every Agent on a fresh survey. No credentials were used and no model endpoint was queried.

- [Command Code Provider API](https://commandcode.ai/docs/provider): three routes
  follow the OpenAI Chat, OpenAI Responses and Anthropic Messages schemas.
  `supported_endpoints` declares a route, not image, tool or reasoning support.
- [OpenRouter model catalog](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties):
  includes `supported_parameters` and `top_provider.max_completion_tokens`.
  Catalog parameter names do not decide a per-Agent field mapping.
- [OpenRouter reasoning](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens):
  a missing `supported_efforts` is not public knowledge, `null` accepts all gateway
  efforts, and an array lists options. `supports_max_tokens` exposes a reasoning
  token budget and `mandatory` forbids disabling reasoning. Effort and token budget
  are alternatives; the target Agent must be able to express whichever is chosen.
- [DeepSeek model catalog](https://api-docs.deepseek.com/api/list-models/):
  `context_window`, `max_output_tokens`, `input_modalities`,
  `output_modalities`, `effort.supported_levels`/`default_level` and
  `api_capabilities.anthropic_messages.system_prompt_update`. Tool calls come from
  the official per-model pricing table, not the catalog.
- [DeepSeek Chat schema](https://api-docs.deepseek.com/api/create-chat-completion/):
  only an official excerpt via Context7 describes `reasoning_effort`
  (`none/low/high/max`) and that temperature does not apply in thinking mode. An
  excerpt does not prove another protocol's or Agent's mapping.

## P24: model information and request settings written to an Agent

The following is the remaining implementation target. Saving a field in the
central model editor, or a successful file write, does not mean an Agent mapping
is verified.

### Field requirements

| Information                                                      | What the central model stores                        | What the Agent configuration must do                                                                                                                 |
| ---------------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Context window                                                   | Declared total capacity and source                   | Write to the native model definition where supported; show source, old value, target field and new value; never present it as a request output limit |
| Maximum output                                                   | Declared output capacity and source                  | Map where the native model definition has such a field; a per-request output limit is separate                                                       |
| Reasoning support and efforts                                    | Evidence or user declaration, optional list or `any` | Declare capability and effort list natively; `any` or unknown is not expanded into every effort                                                      |
| Selected effort                                                  | Not a central request setting                        | Saved per Agent × Provider instance × model, with inherit/no-override; only verified values                                                          |
| Reasoning token budget, temperature, top_p, request output limit | Not model capability metadata                        | Offered only where the Agent supports it with evidence; verify range, combinations and model limits                                                  |

The Agent model configuration screen must show the source and mapping state of
each item, allow editing supported settings, preview differences and apply them.
Unsupported, unknown, missing or higher-precedence values must be explained item
by item; a chosen setting is never silently dropped. Central values are the
default; an Agent-local override is marked as such and is not written back to the
central model. An unset value does not override native behavior.

### Per-Agent verification

- **Claude Code:** verify the native channel, valid range, precedence and version
  differences separately for context declaration, output limit and effort. Writing
  credentials or model variables does not make parameters supported. Unsupported
  declarations show a reason.
- **Pi:** extend the existing `reasoning` and protocol-compatible fields with
  context, maximum output, thinking levels and default/selected values. Recheck the
  installed model and settings schemas, keep the configuration isolated per
  Provider/model, and preserve user fields.
- **Grok Build:** preserve the existing `context_window` mapping and verify
  `[[model.<alias>.reasoning_efforts]]` `value`/`default`, the actual selected
  effort and output-limit fields. The installed 1.0.62 reference deprecates
  `supports_reasoning_effort` and the single `reasoning_effort`; do not introduce
  new writes to those deprecated fields. Target the alias vibemate currently owns, not
  another of the user's entries. Distinguish the effort table, its default and
  `[models].default_reasoning_effort`.
- **Codex, OpenCode, Hermes:** read-only discovery only. A future write adapter
  must complete the same field matrix before the interface claims support. It
  must also define the Agent-list active-model read (the live model field) and
  the ownership evidence that distinguishes a vibemate-written Provider/model
  from **Own configuration**; copied from [agents.md](../agents.md), this is
  required for Claude Code and Pi and must not be retrofitted from history
  matching later.

Before implementing, build an Agent/version × Provider/protocol × model field
mapping matrix from official documentation and the installed version. Handle
display name, Provider API model ID and Agent configuration alias separately; never
reuse or guess a model ID across Providers. Parameter, central metadata and target
file changes all invalidate a preview; every write and restore follows
[the configuration rules](../configuration.md).

### Acceptance

Cover capability declaration, context/output bounds, effort list/default/selected,
unknown/`any`/unsupported, local overrides, isolation of same-named models under
different Providers, switching, preservation of unowned fields, masking, stale
previews and history restore. Use DeepSeek's `deepseek-flash` and a Command Code
model as isolation samples without mixing IDs or efforts.

Deliver each Agent in reviewable slices: Rust mapping, IPC, bilingual
configuration UI, preview/apply/restore and behavior tests. Real requests are
verified by the maintainer after deployment; the standalone inference check stays
cancelled. Actual calls and cross-platform acceptance are recorded separately.

### Verification commands

Behavior tests per batch, then the standard checks: `pnpm run check:frontend`,
`cargo fmt`, locked Clippy with warnings denied, locked tests, and the no-bundle
desktop build at checkpoints. Real inference, Windows/Linux native behavior and
later write/restore work are not proven by documentation checks or preview tests.
