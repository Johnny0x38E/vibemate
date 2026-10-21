# Inject Provider models into Agents

Provider models are the single source of model definitions. The existing editor
already holds the required capacities, image/tool/reasoning capabilities, effort
levels and declared default. P24 consumes those definitions; it does not add a
second model editor or general request-tuning controls to Agent configuration.
Task status is maintained only in [`todo.md`](todo.md).

## Responsibility split

| Location               | Responsibility                                                                                                      |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Provider API           | Endpoint, credential reference and protocol                                                                         |
| Provider models        | Model ID, alias, context/output capacity, capability evidence, effort levels and default                            |
| Agent adapter          | Translate the saved definition into fields the installed Agent actually uses                                        |
| Agent configuration UI | Select Provider/model, review injection and, where necessary, choose an initial/default effort from declared values |
| Rust                   | Verify mapping, preserve unowned fields, rebuild approvals, back up and write atomically                            |

No new temperature/top_p, request-output-limit or reasoning-token-budget editors
are part of this work. Existing user-native parameters remain unchanged. A model's
maximum output capacity must not be relabeled as an independently chosen request
limit. Agent-specific mappings must not change the Provider's model definition.

## Thinking levels and defaults

These are separate responsibilities:

- **Available levels / mapping:** make the model's declared levels selectable
  inside the Agent when it supports a native mapping or option list. Inject the
  verified list independently of which default is selected.
- **Default:** choose an initial level from the declared model options. Prefer
  the model-declared default where it has a verified native meaning; an explicit
  injection choice may select another declared value. Do not invent a default
  for unknown or empty metadata, expand `any`, or convert effort into token counts.
- **Default-only Agent:** write a verified default channel rather than pretending
  an internal switchable list was installed. A native runtime default does not
  prove that its value can be configured externally.

Preserve user mappings and exclusions. An Agent may have built-in levels in
addition to the injected mappings; do not claim the central list constrains its
entire menu unless its native format actually enforces that restriction.

## Agent-specific work

### Claude Code

Consume the existing model identity/aliases and supported metadata. Its current
adapter points the class aliases at one selected model; separately assigning
models to multiple aliases is not implemented. Do not present that as delivered.

The adapter intentionally injects one selected Provider model: the session model
and Claude Code class aliases point to that same model. Separate per-alias model
selection is outside this single-model injection design.

The verified DeepSeek `deepseek-flash` path writes declared context capacity and
an explicit default effort through native env fields. The default picker only
uses the model's declared values and the verified native intersection. Remembered
starting choices are existing injection preferences, not model definitions;
applying or restoring does not silently save them. The env has cross-model scope.

Other custom-model recognition, capacity/image/tool mappings and default channels
must be verified individually. Native Claude effort commands do not establish
that an arbitrary injected Provider/model supports switching. Do not assert either
universal switching or universal inability to switch.

### Pi

Continue mapping declared `contextWindow/maxTokens/reasoning/input`. `maxTokens`
is a model limit. Pi has no corresponding model-definition tool-call flag; that
absence does not mean the model cannot call tools.

Inject verified `thinkingLevelMap` entries for all declared supported levels, not
only for a selected default. Current generated extended mappings are limited to
DeepSeek `deepseek-flash` on its official Chat endpoint: `xhigh→high`, `max→max`.
Existing user entries, including null exclusions, are preserved. Other bindings
remain pending rather than receiving guessed vendor values.

Default selection remains separate in
`modelThinkingLevels["<provider>/<modelId>"]`; no override preserves the saved
value. Project/CLI/resumed-session settings may take precedence. The configuration
schema must expose the fields used. Provider-declared defaults are displayed but
are not yet automatically selected; further default-flow changes need their own
review and tests.

### Grok Build

Keep the managed-alias model identity and declared `context_window` mapping.
Native effort tables use `[[model.<alias>.reasoning_efforts]]` with `value` and a
possible `default = true`. This is the target for switchable options and a model
default, not a reason to add a generic request editor. The adapter does not yet
write this array: validate values, default selection and preservation before
claiming support. Do not introduce writes to deprecated `supports_reasoning_effort`
or `reasoning_effort`, and do not use the global default to represent one alias.

The documented Grok `vision` flag now consumes an explicit central image-input
state. Independent output-capacity and tool metadata mappings remain to be
established. `max_completion_tokens` is a request setting; the existence of that
key alone does not prove an independent capability declaration.

## Evidence and implementation order

Reuse the [field matrix](../integrations/model-parameter-mapping.md) and existing
[Claude Code](../integrations/claude-code.md), [Pi](../integrations/pi.md) and
[Grok Build](../integrations/grok-build.md) research. The recorded installations
are Claude Code 2.1.304, Pi 1.3.0 and Grok Build 1.0.62; do not repeat a full survey
unless an actual conflict or known deprecation requires it.

First remove the out-of-scope request editor and fix mappings coupled to one
selected default. Then address remaining Agent-native injection gaps in reviewable
behaviors. Keep unsupported and unverified distinct; neither adds new Provider
model fields merely to fill a native configuration key.

## Acceptance and verification

For each Agent, record which existing Provider fields are consumed, the native
keys and precedence, the default behavior, and whether native switching works.
Test all declared mapped levels independently of the chosen default, unknown or
`any` metadata, user exclusions, same-ID models under different Providers, stale
previews, unowned-field preservation and history restore. A successful file write
is not proof of an effective session or a successful Provider request.

Run `pnpm run check:frontend`, Rust fmt, locked Clippy with warnings denied, locked
tests, and `pnpm run tauri build --no-bundle -- --locked` after implementation and
tests are ready. Maintainer desktop/native application and restoration, actual
Agent use, and Windows/Linux acceptance remain separate. The standalone inference
checker stays cancelled; do not start requests or create another browser space
without authorization.
