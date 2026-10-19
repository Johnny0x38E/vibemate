# Model metadata and per-Agent request settings

This plan covers model information that vibemate does not write into Agent
configuration yet, and the rules that decide where each value belongs. Implemented
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

## Evidence and gaps

Only public documentation has been read; no credentials were used and no model
endpoint was queried.

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
- **Grok Build:** verify and implement `context_window`, `supports_reasoning_effort`,
  `[[model.<alias>.reasoning_efforts]]` `value`/`default`, and the actual selected
  effort and output-limit fields. Target the alias vibemate currently owns, not
  another of the user's entries. Distinguish the effort table, its default and
  `[models].default_reasoning_effort`.
- **Codex, OpenCode, Hermes:** read-only discovery only. A future write adapter
  must complete the same field matrix before the interface claims support.

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
