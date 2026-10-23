# 模型字段与 Agent 参数映射

P24.a 的调查记录，核实于 2026-10-20；Pi 部分于 2026-10-22 按 1.4.0 复核。P24.b 分批实现下述 DeepSeek 上下文容量映射及显式 effort 选择；其余参数不因本页的原生通道证据自动视为已实现，也不代表真实请求兼容。

维护者随后决定复用已有调研推进实现，不因小版本更新重新调查全部字段。以下未核实项保留为逐字段限制，只有遇到明确冲突、已知弃用或验证失败时再针对性查证，不作 P24.b–d 的整体前置阻塞。
任务状态只在 [todo](../plans/todo.md) 维护；字段归属见 [model-parameters](../plans/model-parameters.md)。

## 证据范围与状态

本机 macOS Apple Silicon 的实际版本：

| Agent       | 本次版本证据                                                                    | 可读实现证据                                                                               |
| ----------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Claude Code | `claude --version` → `2.1.304 (Claude Code)`；Homebrew Cask 入口                | 官方现行文档；安装二进制仅核对字段名，不把字符串存在当作语义证明                           |
| Pi          | `pi --version` → `1.4.0`（1.3.0 证据保留）                                      | 该安装包的 schema、模型解析器、启动选择器和协议请求构建源码；1.4.0 与 1.3.0 的档位逻辑一致 |
| Grok Build  | `grok --version` → `grok 1.0.67 (2fe064390d07)`；`.metadata_version` → `1.0.67` | 本机随版本解包的用户手册与内嵌默认模型目录；未证明公开 main 与二进制源码一致               |

本轮补充固定版本读取已返回 Claude Code 2.1.308；上表 2.1.304 是已有映射调查基线，
没有将新版本号当作新增兼容性验证。Pi/Grok 隔离样本仍使用表内版本。

- **supported**：原生字段及其记录的语义有证据；不等于 Provider 接受该参数。
- **unsupported**：没有相应的独立声明通道，或已核实该通道忽略此字段；不能换成另一个含义的字段。
- **unverified**：字段、范围、模型匹配或协议行为证据不足；暂不提供写入选项。

未知值保持未知。`any` 不展开成全部档位；同名 effort 不证明跨协议等价。
表中 supported 均受版本、模型和协议条件限制。没有读取用户配置或凭据、启动 Agent 会话、连接 MCP 或发送推理请求。

## Claude Code 2.1.304

目标是用户 `settings.json`。适配器写入 `env` 的身份与类别模型键；P24.b 首批还将中央保存的 DeepSeek `deepseek-flash`（不带 `[1m]`）容量映射到 `env.CLAUDE_CODE_MAX_CONTEXT_TOKENS`。缺少容量则继承；其它 ID 不改写窗口并给出双语提示，已有值仍可能影响它们。该窗口批次不增加局部参数编辑或关闭压缩。第二批为同一绑定增加显式 effort 草稿与写入，边界见下文。

| 信息                           | 原生目标与语义                                                                                                                | 状态、限制与实施决定                                                                                                                                                                                                                                 |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 上下文容量                     | `env.CLAUDE_CODE_MAX_CONTEXT_TOKENS`：校正 Agent 假定的窗口                                                                   | **supported，条件式**。仅对不能识别、不以 `claude-` 开头且不含 `[1m]` 的自定义 ID 直接生效；其它 ID 见下文，不能无条件映射中央数字                                                                                                                   |
| 最大输出能力                   | 未找到独立的任意模型最大输出能力声明字段                                                                                      | **unsupported，本轮最小范围**。不把中央能力值直接写成请求限制                                                                                                                                                                                        |
| 请求输出限制                   | `env.CLAUDE_CODE_MAX_OUTPUT_TOKENS`                                                                                           | **supported，原生请求通道**。官方 env-vars 条目：未知模型默认 32000、上限 128000，已识别模型可能降低；不作为独立输出能力声明，本轮不提供编辑器。                                                                                                     |
| 推理支持、可选档位、模型默认档 | 由 Claude Code 识别的模型决定；没有核实三家 Provider 任意模型能力表的通用声明方式                                             | **unverified**。不能把 Provider effort 列表写成 Claude 的模型能力，也不能用 `alwaysThinkingEnabled` 声称模型支持推理                                                                                                                                 |
| 选用档位                       | `modelSettings.<canonical-model>.effortLevel`：`low/medium/high/xhigh`；`env.CLAUDE_CODE_EFFORT_LEVEL` 另接受 `max` 与 `auto` | **supported，通道语义**；任意第三方 ID 的 canonical 匹配及 Provider 档位传递仍 **unverified**。`max` 不能写入 `effortLevel`。第二批仅在 DeepSeek `deepseek-flash`、已声明推理支持和明确档位列表的交集内提供 env 写入；不承诺原生 Provider/model 隔离 |
| 推理 token 数                  | `env.MAX_THINKING_TOKENS`                                                                                                     | **supported，仅固定 thinking 模型**；adaptive 模型忽略正数的数量。三家 Provider 的映射 **unverified**，首批不提供。不得由 effort 换算                                                                                                                |
| temperature、top_p             | 本轮没有核实通用持久通道                                                                                                      | **unverified**，不写未知 JSON key                                                                                                                                                                                                                    |
| 自动压缩阈值                   | `modelSettings.<canonical-model>.autoCompactWindow` 或顶层 `autoCompactWindow`，另有 env/CLI 覆盖                             | **supported**，是 Agent 策略，不是模型容量；不按中央窗口自动生成 75% 的用户策略                                                                                                                                                                      |

### 窗口、优先级与隔离

官方 model-config 页将窗口校正分成三种情况：

1. 不能识别的自定义拼写，无 `[1m]`：窗口变量可直接应用，主动压缩继续有效。
2. 同类 ID 含 `[1m]`：默认假定 1M；窗口变量单独无效，还需 `CLAUDE_CODE_DISABLE_1M_CONTEXT=1`。
3. 可识别模型或裸 `claude-` ID：窗口变量只有同时禁用全部压缩的 `DISABLE_COMPACT` 才生效。

不为声明容量而静默追加 `[1m]`、关闭 1M 或关闭压缩。第一批只考虑第一种可证明的 ID，其余展示拒绝原因。
“不能识别”必须由可复核的匹配规则证明，不能只测试是否包含字符串 `claude`。

设置文件仍按 managed → CLI settings → project local → project → user 优先。
同文件的模型级 effort 覆盖顶层 effort，跨文件以设置该模型值或适用顶层值的最高层为准。
显式 env/CLI/会话选用值先于保存值；组织与 `maxEffortLevel` 还会限制可选档。
`modelSettings` 以 canonical 模型名匹配 alias、日期后缀和可识别 Provider ID；它没有 vibemate Provider 实例命名空间。
`env` 通道同样影响使用该用户配置的会话，不能当作隔离的每模型设置。

版本条件：模型级 effort 需 2.1.251+；模型级 effort cap 需 2.1.267+；模型级压缩窗口需 2.1.288+。
压缩窗口的设置范围为 100000–1000000 tokens，并受模型窗口限制；这不是上下文容量字段的值域。
用户顶层 effort 对新模型的适用性有例外，不能用它替代模型级保存。

**分批实施边界：** 窗口切片只对已限定的 ID 写容量。第二批复用 DeepSeek 专用配方的 env effort 通道；仅对 `deepseek-flash` 提供模型明确列表与 `low/medium/high/xhigh/max` 的交集，要求推理支持声明，不展开未知、`any`、空列表或不兼容值，不把模型默认值自动当作用户选择。用户声明仍是声明，不升级为真实请求证明。

选定值写入 env，“不覆盖”保留当前原生值，且不转换为 token 数、不改模型能力元数据或 `modelSettings`。预览、详情和应用审批携带同一选择；摘要变化使旧审批失效。切换 Provider/model 会丢弃未记住的草稿。第三批增加 vibemate 内按 Claude Code 与 Provider/model 记忆的独立起始偏好，切换后读取该绑定的保存值；这不是原生 env 的每模型隔离。UI 明示 env 可能影响其它模型、Claude Code 可能限档或降档，计划值不证明会话实际生效。

第三批的独立偏好由显式“记住此选择”保存，不随预览、应用或恢复自动修改。读取返回同一 SQLite 快照内的模型元数据与偏好，保存同时比较模型摘要和随机偏好版本；清空保留新版本以识别中间变更。失效的保存档位明确显示为未映射，不降档或回退；读取失败不当作继承。模型/Provider 删除级联清理偏好，但不改原生文件或历史，也不代表 P35 引用约束已完成。偏好不是审批时的实时查表值，已经预览的显式选择继续由原有配置令牌保护。

请求输出限制、其它模型/Provider 档位和 canonical 匹配仍受逐字段缺口限制。DeepSeek 官方配方的 `max` 不是任意 Provider 或模型的通用规则。

针对剩余声明通道复核现行 model-config 页：`_SUPPORTED_CAPABILITIES` 能声明
`effort/xhigh_effort/max_effort/thinking/adaptive_thinking/interleaved_thinking`，但官方只明确
`_NAME/_DESCRIPTION` 也适用于 `ANTHROPIC_BASE_URL` 网关，未证明能力后缀在本阶段两家
网关生效。它还是完整启用/禁用集合，不能把中央单个 reasoning 布尔转换为整表而关闭未声明能力。
因此网关能力声明保持 **unverified**，不写这些后缀；图像、工具与独立输出能力仍无已核实通道。
容量识别继续仅对已有精确 Flash 绑定写入，不能由“不含 claude”推断任意 ID 一定不被识别。

证据：[Model configuration](https://code.claude.com/docs/en/model-config)、
[All settings](https://code.claude.com/docs/en/settings-reference)、
[Environment variables](https://code.claude.com/docs/en/env-vars)、
[Errors](https://code.claude.com/docs/en/errors)。前三者中的 model-config 与 settings-reference 直接浏览；
env-vars 与 errors 的 thinking/output 结论来自 Context7 官方来源摘录，未取得完整 output 字段表。
旧接入证据见 [Claude Code](claude-code.md)，不将旧版观察升级为本轮实测。

## Pi 1.4.0

模型定义在 `models.json.providers.<owned-provider>.models[]`，按 `id` 定位；保存的选用档位在 `settings.json`。
1.4.0 的模型/settings 字段与 1.3.0 相同；schema 新增字段说明与默认值，逐字段参考见安装包 `docs/models-configuration.md`。

模型条目的默认值（省略字段时由 Pi 使用）：`contextWindow` 128000、`maxTokens` 16384、`reasoning` false、`input` `["text"]`。这些默认值在 1.3.0 已生效，1.4.0 将其写入 schema。

| 信息               | 原生目标与语义                                                     | 状态、限制与实施决定                                                                                                                                                                                                                                |
| ------------------ | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 上下文容量         | 模型 `contextWindow`                                               | **supported，已写入**；schema 要求大于 0，省略时 Pi 使用 128000；vibemate 使用正整数 token 数                                                                                                                                                       |
| 最大输出能力       | 模型 `maxTokens`                                                   | **supported，已写入**；schema 要求大于 0，省略时 Pi 使用 16384。运行时以它作为输出上限，1.4.0 起 compaction 与分支摘要也按它取上限（按上下文收缩）；它不是用户另选的每次请求限制                                                                    |
| 推理支持           | 模型 `reasoning` 布尔                                              | **supported，已写入已声明值**；用户声明不升级为 Provider 验证                                                                                                                                                                                       |
| 档位映射           | 模型 `thinkingLevelMap`，Pi level → 厂商字符串或 `null`            | **supported**。键为 `off/minimal/low/medium/high/xhigh/max`；map 是转换，不是自由字符串列表。`null` 排除该档；未映射的 `off` 到 `high` 仍可用，`xhigh`/`max` 仅在显式字符串映射时可用。不能用部分 map 宣称其余档位都不可选，也不能展开 `any`        |
| 模型默认/选用档位  | `settings.modelThinkingLevels["<provider>/<modelId>"]`             | **supported**，每模型保存的启动选择；不写全局 `defaultThinkingLevel` 来表达某一个模型的默认档。Provider 的默认档只有能无歧义映射时才能作为建议值，不能自动当作用户选择。P24.c 首批已写入中央声明且 Pi 无需映射即可接受的档位（见 [Pi 契约](pi.md)） |
| temperature、top_p | 模型 `samplingParams`；档位覆盖用 `samplingParamsByThinkingLevel`  | **supported，仅 OpenAI 系协议**；先模型默认、再有效 Pi 档位覆盖、最后请求级覆盖，后者优先。其它 API 忽略这两个对象，必须拒绝而非静默丢失                                                                                                            |
| 请求输出限制       | Chat/Responses 的 sampling 输出键                                  | **unsupported，安全持久配置**。源码确认这些键在按当前会话上下文限制输出之后合并，可绕过动态限制。vibemate 明确拒绝，不把请求值写成模型 `maxTokens`；保留已有用户键。Messages 同样不提供。                                                           |
| 推理 token 数      | SDK request `thinkingBudgets` 与兼容字段不等于已核实的持久 UI 配置 | **unverified，暂不写**。不复用 Pi 内部默认 token 数作为 Provider effort 换算                                                                                                                                                                        |

Pi 启动选择源码中：scoped model 显式档位 → 该 provider/model 保存值 → `defaultThinkingLevel` → Pi 默认；
普通保存默认模型路径也优先 per-model。CLI、恢复会话和运行中 `/thinking` 能选择另一值，磁盘写入不证明运行中生效。
Pi 1.4.0 隔离 macOS 原生会话已确认扩展档位菜单、`max` 选择、每模型启动默认档、
同 ID 跨 Provider 隔离与 null 排除。切回模型时重新采用其保存档位，不保留离开前的临时选择。
11 项断言与复验步骤见 [Pi 契约](pi.md#p24c-隔离原生菜单与切换验证)；未发送 Provider 请求。
项目设置可覆盖用户设置；既有全局、项目与会话设置必须在预览中区分。

**最小可交付切片：** 保持现有容量/能力映射，增加字段来源与状态展示（P24.c 首批已在计划卡片中交付，见 [Pi 契约](pi.md)）；随后按已验证模型映射交付
`thinkingLevelMap` 与 provider/model 局部的保存档位。采样/请求输出不属于本轮注入，不另建编辑器。
不接管别的 Provider 的同名 ID，不更改全局默认档位；已有 map、采样对象和未知字段按所有权保留。

证据：官方 [Models](https://pi.dev/docs/latest/models) 及安装包 1.4.0 的 `docs/models.md`、
`docs/models-configuration.md`；`schemas/models.schema.json`、`schemas/settings.schema.json`、
`dist/core/model-config.js`、`dist/core/model-resolver.js`；
同安装的 `@earendil-works/pi-ai/dist/api/simple-options.js`、`openai-completions.js`、`openai-responses.js`。
现有映射已在仓库 `src-tauri/src/agents/pi.rs::map_metadata` 中核对；1.4.0 与 1.3.0 的 `getSupportedThinkingLevels` 规则一致。

## Grok Build 1.0.67

只拥有 `config.toml` 中有根限定归属记录的 `[model.<alias>]`。alias 是配置身份，`model` 是 API ID。
1.0.62 与 1.0.67 的参考页都列出 `reasoning_efforts`；1.0.67 内嵌的 `default_models.json` 给出
数组项 `value/label/description/default` 的形状。适配器只写 `value/label/default`，不写 `description`。

| 信息               | 原生目标与语义                                                                                | 状态、限制与实施决定                                                                                                                                                                                                                               |
| ------------------ | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 上下文容量         | `model.<alias>.context_window`                                                                | **supported，已写入**；驱动自动压缩，未知新模型未设置时默认 200000。不是请求输出长度                                                                                                                                                               |
| 图像输入           | `model.<alias>.vision`                                                                        | **supported，已写入**；仅映射中央明确声明的 supported/unsupported；unknown 保留已有值。配置声明不证明 Provider 实际接受图片。                                                                                                                      |
| 最大输出能力       | 手册未给独立容量字段                                                                          | **unsupported，本轮最小范围**；不能改写为 `max_completion_tokens`。`max_completion_tokens` 是请求字段，不构成独立能力声明                                                                                                                          |
| 请求输出限制       | `model.<alias>.max_completion_tokens`                                                         | **supported，原生请求字段**。不作为已核实的独立输出能力映射；本轮不写入，保留用户值。各 backend 出站键和 Provider 实际接受仍 **unverified**。                                                                                                      |
| 推理支持           | `supports_reasoning_effort` 旧布尔；新方向为 `reasoning_efforts`                              | **supported，但旧字段弃用**。适配器不新增或删除该布尔；显式 false 会阻止新列表注入，保留原值。不能把字段存在当作 Provider 验证。                                                                                                                   |
| 可选档位           | `[[model.<alias>.reasoning_efforts]]`                                                         | **supported，已写入**。仅在原生字段缺失、中央 `Listed` 非空、推理支持为 supported、安装参考含该字段且未被显式 false 禁用时写入；每项 `value/label`，中央默认档在列表内时该项 `default = true`。已有数组保留不覆盖；`any`、空列表、未知支持不展开。 |
| 模型默认档         | 上述列表中某项 `default = true`                                                               | **supported，已写入中央声明的默认档**；只在该档同时出现在列表中时标记。隔离 macOS Chat/Responses 会话已确认启动选用列表默认档及 `/effort` 切换；请求字段与 Provider 接受仍 **unverified**。                                                        |
| 实际选用档位       | 旧 `model.<alias>.reasoning_effort`；全局 `[models].default_reasoning_effort`；会话 `/effort` | 前者 **deprecated**，后者是默认模型的全局值，不满足每个受管 alias 的隔离；适配器不写这两处，也不改 `[models].default`。                                                                                                                            |
| 工具调用能力       | 手册未给模型级工具能力字段                                                                    | **unsupported，本轮最小范围**；不写工具声明，字段缺失不代表模型不能调用工具。                                                                                                                                                                      |
| temperature、top_p | `model.<alias>.temperature/top_p`                                                             | **supported，原生字段语义**。模型值优先于全局值；本轮不新增编辑器或写入，保留用户值。Provider/推理模式组合仍需单独验证。                                                                                                                           |
| 推理 token 数      | 本轮未找到已验证的持久字段                                                                    | **unverified，暂不写**                                                                                                                                                                                                                             |

七层优先级仍见 [Grok 契约](grok-build.md)：CLI、环境、requirements/MDM、允许的覆盖层、用户、managed、内置默认。
不要为选定某模型的 effort 而写全局 `[models]`；也不改变用户的 `[models].default`。
随版本 `/effort` 手册只列 `low/medium/high/xhigh`，不能由旧用户样本中出现 `max` 推断全版本/协议都支持。

**最小可交付切片：** 保持窗口与图像写入并展示来源；已声明的非空档位列表和默认值写入
`reasoning_efforts`。已有列表、旧布尔/单值字段、请求参数和其它 alias 保持原样。
隔离 macOS Chat/Responses 会话已确认原生菜单、默认档和会话内切换；6 个回环 stub 样本确认两个 backend 的努力字段及列表默认值，不声称真实 Provider 接受已验证。

证据：本机 `.metadata_version`（1.0.67）、`docs/user-guide/11-custom-models.md` 的 Context Window/Global Default Values、
`26-config-reference.md` 的 model 与 models 表（尤其 471–479 行）、`04-slash-commands.md` 的 `/effort`，
以及 `grok` 二进制内嵌的默认模型目录（数组项形状）。
在线 [配置参考](https://docs.x.ai/build/settings/reference) 仍展示旧布尔/单值字段，未完整列新数组规则；
本轮以随版本手册和内嵌目录记录差异，不把在线简表覆盖掉已安装版本的弃用标记。
现有窗口与 effort 写入在 `src-tauri/src/agents/grok_build.rs` 中核对。

## Provider × 协议约束

以下是配置与请求构造证据，不是实际兼容矩阵；原有组合限制继续有效。

| Provider / 协议                | Claude Code                                           | Pi 1.4.0                                                                                                                                                                           | Grok 1.0.67                                                                                                 |
| ------------------------------ | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| DeepSeek / OpenAI Chat         | 不适用此入口                                          | `openai-completions`；`compat.thinkingFormat=deepseek`，输出键按 `compat.maxTokensField`。实现可发送 `thinking.type` 与 `reasoning_effort`，合法 effort 仍需该模型的 Provider 证据 | `chat_completions` 可配置；本地样本输出顶层 `reasoning_effort`；DeepSeek 真实接受与其它 thinking 字段仍未验 |
| DeepSeek / Messages            | 官方专用网关配方；上下文与 `max` 不泛化为其它模型     | `anthropic-messages` 不接受上述 OpenAI sampling 配置；effort/budget 的厂商映射未核实                                                                                               | 现有适配器拒绝 Messages，认证请求头未核实；本轮不解除限制                                                   |
| OpenRouter / OpenAI Chat       | 不适用此入口                                          | `thinkingFormat=openrouter` 构造嵌套 `reasoning.effort`，不冒充直接 `reasoning_effort`；mandatory、effort 范围仍按 Provider/model 检查                                             | Chat 输出顶层 `reasoning_effort`（官方网关接受的 shorthand）；真实模型/路由接受未验                         |
| OpenRouter / Messages          | 现有专用网关配方，仅保证 Anthropic 模型；保留原有限制 | sampling 配置同上不适用；推理档位按具体模型/协议核实                                                                                                                               | 继续拒绝 Messages                                                                                           |
| Command Code / Chat、Responses | 现有网关身份仍未核实，继续拒绝                        | 分别 `openai-completions`、`openai-responses`；输出/采样键必须分开；档位不凭 endpoint 名决定                                                                                       | 本地样本确认 Chat `reasoning_effort`、Responses `reasoning.effort`；模型值域/Provider 接受未验              |
| Command Code / Messages        | 继续拒绝；原有证据仅允许 Claude 模型走此路由          | 可表达模型容量；不使用 OpenAI sampling 字段；推理参数需单独验证                                                                                                                    | 继续拒绝 Messages                                                                                           |

Provider 的当前范围与旧证据见 [providers](providers.md) 和 [compatibility](compatibility.md)。
本轮没有重新读取三家 Provider 的完整 schema，不能把旧目录中的某模型档位当作本轮已核实。
Responses 与 Messages 即使有同名模型 ID，也不能照搬 Chat 的参数键。

## 模型注入范围

服务商模型定义已经覆盖目标属性，本表服务于原生字段适配，不要求新增 Provider 模型字段。
有关 temperature/top_p、请求输出和 reasoning token 的行仅记录原生证据，不属于待实现编辑器。
Claude Code 当前只提供已核实绑定的默认 effort；Pi 把映射与默认分开，所有已声明且已核实的
扩展档位独立注入；Grok 已注入缺失数组和列表内默认档，并在隔离 macOS 会话中确认菜单与选择更新。独立容量、图像、工具标记须逐 Agent 判断，
不能因没有声明字段就判定模型无法调用工具或处理图片。当前实施见 [兼容矩阵](compatibility.md)。

## P24.a 尚需补齐的证据

1. Claude Code：output 条目仅作原生请求通道证据，本轮不提供编辑器；第三方 ID 的 canonical 匹配和原生 per-model effort 隔离仍未核实。
2. Pi：已核实并实现官方 DeepSeek Chat Flash/Pro 的 `xhigh→high/max→max` 与 OpenRouter Chat 的 `xhigh→xhigh/max→max`；四个安装 SDK 发送前载荷样本通过，独立于默认选择注入；其它绑定仍需厂商证据。请求输出覆盖会绕过动态限制，且不是本轮模型注入控件。
3. Grok：1.0.67 已核实数组项 `value/label/default`、内嵌默认模型目录和显式 false 的阻止规则；已有数组保留。隔离 macOS Chat/Responses 已验证 `/effort` 菜单、列表默认档及 `high ↔ max` 会话切换；本地 stub 的 6 个样本确认两个 backend 的努力值字段和列表默认档；随后原生 TUI 确认切档后的下一请求和重启选择，Grok 自身持久化到全局默认字段而非每 alias；真实 Provider 合法范围仍未核实。没有证据前不改为 deprecated 通道。
4. Provider：重新读取三家现行 schema，逐模型确认合法档位、默认值、mandatory 与采样组合；至少使用 DeepSeek `deepseek-flash` 和一个明确 Command Code API ID 作隔离样本。不猜测不同 Provider 的 ID 或 effort 等价。

这些缺口不应被标为已核实；按维护者决定，可使用已有证据分批实现，实际遇到对应限制时再补证，不要求先完成新一轮全面调研。
后续 P24.b–d 仍须分别交付 Rust、IPC、双语 UI、过期保护、字段保留与历史恢复测试。

## 本轮验证边界

执行三个固定 `--version` 命令，读取发行手册/schema/代码，浏览上述官方页面并查询 Context7。
codegraph 未找到本 checkout 的索引，已按工具指引改用文件工具，没有初始化索引。
初次调查只改文档。随后 P24.b 首批通过 Rust/前端检查、桌面构建和隔离 macOS Tauri 容量写入、过期保护及恢复验证，结果见 [todo](../plans/todo.md)。原生验证只使用合成凭据；发现并修复了重复预览的旧凭据值未脱敏问题。没有发送 Provider 请求，Windows/Linux 与真实模型行为仍未验证。

2026-10-22 按 Pi 1.4.0 复核了安装包版本、CHANGELOG、两版 schema 差异、`getSupportedThinkingLevels` 与 compaction 文档；未启动 Pi 会话、未发送请求，也未在真实 Tauri 窗口重新验收。

2026-10-23 的 Grok 字段复核未启动会话或发送请求；随后已完成隔离原生菜单/选择、6 个回环 stub 载荷样本及 4 个 Pi SDK 发送前样本，详见各 Agent 契约。仍未发送真实 Provider 请求。
