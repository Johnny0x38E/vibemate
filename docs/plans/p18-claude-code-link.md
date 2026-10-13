# P18：Claude Code 首个 Agent 与模型链接

任务状态只在 [todo.md](todo.md) 记录，本文件记录 P18 的最小预览实施计划与依据。
职责划分与参数规则见 [model-parameters.md](model-parameters.md)，接入证据见
[claude-code.md](../integrations/claude-code.md)。

## 目标与边界

让用户在 Claude Code 的 Agent 详情页选择已保存的 Provider 与模型，由 Rust 生成一份脱敏的
配置计划：写哪个文件、改哪些键、可能被谁覆盖、需要重启与否、凭据以什么形式落盘。
预览不写文件、不读凭据明文、不发请求。可恢复写入与回滚是 P19–P21。

## 已核实依据（2026-10-13）

- Claude Code 网关变量为 `ANTHROPIC_BASE_URL` + 凭据变量（`ANTHROPIC_AUTH_TOKEN` 走
  `Authorization: Bearer`，`ANTHROPIC_API_KEY` 走 `x-api-key`），可写入
  `~/.claude/settings.json` 的 `env` 块；设置文件 `env` 的值优先于 shell 导出；官方禁止把凭据
  写进项目级 `.claude/settings.json`。
- `model` 键只在会话开始时读取；`env` 变更同样需要新会话。预览必须给出重启提示。
- DeepSeek 有官方 Claude Code 配方；OpenRouter 有官方配方并说明只保证 Anthropic 一等厂商的
  模型；Command Code 没有 Claude Code 接入说明。
- 本机安装为 `2.1.287`（Homebrew Cask 布局）。

## 映射决定（首批）

| 目标键                     | 值                                             | 说明                                        |
| -------------------------- | ---------------------------------------------- | ------------------------------------------- |
| `env.ANTHROPIC_BASE_URL`   | 该 Provider 已核实的 Claude Code 网关地址      | DeepSeek `.../anthropic`；OpenRouter `/api` |
| `env.ANTHROPIC_AUTH_TOKEN` | 占位符；应用阶段由 Rust 从 OS 凭据存储取值写入 | 预览只标记"此处是密钥"                      |
| `env.ANTHROPIC_MODEL`      | 所选模型的模型 ID                              | 与两家厂商配方一致                          |
| `env.ANTHROPIC_API_KEY`    | `""`                                           | 仅 OpenRouter 官方要求显式置空              |

未映射项（保持未知，不做猜测）：`[1m]` 上下文变体、`ANTHROPIC_DEFAULT_*_MODEL` 系列与
`CLAUDE_CODE_SUBAGENT_MODEL`、`CLAUDE_CODE_EFFORT_LEVEL`、
`CLAUDE_CODE_AUTO_COMPACT_WINDOW`、网关模型发现。

## 凭据通道（2026-10-13 决定）

采用 Claude Code 文档的 `env` 通道：应用阶段由 Rust 从 OS 凭据存储读取密钥，写入用户级
`~/.claude/settings.json` 的 `env.ANTHROPIC_AUTH_TOKEN`。这意味着**密钥以明文落在该文件里**，
预览与界面必须明示，`credentialFromStore` 提示就是这条说明。

不使用 `apiKeyHelper`：它能让配置文件只保存一条命令引用、密钥不落盘，但 Claude Code 默认每
5 分钟执行一次该命令（`CLAUDE_CODE_API_KEY_HELPER_TTL_MS` 可调），命令的签名与 macOS 钥匙串
授权行为、以及空输出与命令失败的后果都尚未核实；另外 `ANTHROPIC_AUTH_TOKEN` 一旦存在会覆盖
helper。将来若改成引用式，先验证这些点再改计划映射。

不写 `model` 键：它是用户级设置，而 `env.ANTHROPIC_MODEL` 覆盖所有设置文件；两家厂商的官方配方
都用后者。重复键会在预览里作为覆盖提示出现。

组合状态（选择器与预览共用）：

| Provider     | 状态         | 处理                                        |
| ------------ | ------------ | ------------------------------------------- |
| DeepSeek     | 已核实       | 生成计划                                    |
| OpenRouter   | 已核实有限制 | 生成计划并附"仅保证 Anthropic 模型"提示     |
| Command Code | 未核实       | 拒绝生成并说明缺少官方 Claude Code 接入说明 |

实例地址被用户改成厂商默认之外的地址时同样拒绝，理由为无法核实该地址的网关行为。

## 使用设置的归属（2026-10-13 分类）

Claude Code 的 `model-config` 与 `settings-reference` 页已核实下列键的语义。规律是：**能力事实放模型层，
写哪个键与写什么值放 Agent 层**，同一件事在不同 Agent 里的键名、单位和语义都不同（Codex 用
`model_context_window`/`model_max_output_tokens`，Pi 把窗口写在模型定义里，Grok Build 的
`context_window` 只是元数据）。

| 键                                                                                      | 类别                                              | 归属与默认值                                                        |
| --------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------- |
| `ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_MODEL`                         | 链接身份                                          | Agent 绑定；厂商已核实地址、OS 凭据、所选模型 ID                    |
| `ANTHROPIC_DEFAULT_OPUS/SONNET/HAIKU_MODEL`、`CLAUDE_CODE_SUBAGENT_MODEL`               | 类别 → 模型策略（Claude Code 按用途与表面取模型） | Agent 绑定；默认可全部指向所选模型，允许逐类指定                    |
| `CLAUDE_CODE_MAX_CONTEXT_TOKENS`                                                        | 模型事实在 Agent 侧的声明                         | Agent 绑定；**派生**自模型目录 `context_window`，缺失则不写         |
| `CLAUDE_CODE_AUTO_COMPACT_WINDOW`                                                       | Agent 策略值（压缩阈值）                          | Agent 绑定；派生自窗口的比例（官方样本 786432 = 0.75 × 1M），可覆盖 |
| `CLAUDE_CODE_EFFORT_LEVEL`                                                              | 使用设置（档位）                                  | Agent 绑定；**可选档位来自模型声明**，选定值属于用户                |
| `API_TIMEOUT_MS`、`BASH_DEFAULT_TIMEOUT_MS`、`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` | Agent 自身行为（超时、遥测）                      | 不映射；链接流程不静默改用户的超时与隐私选择                        |

四条已核实的细节，映射时必须处理：

1. `max` 档位只在环境变量通道有效，`effortLevel` 与 `modelSettings` 两个设置键不接受它（另有独立的
   `ultracode` 键）。通道选择限制可选值。
2. 不支持的档位不会报错：Claude Code 降级到不高于该值的最高支持档。预览必须提示实际生效档位。
3. 类别键会盖过 `ANTHROPIC_MODEL`：只改 `ANTHROPIC_MODEL` 会让子代理、技能等表面继续用旧模型。
4. `[1m]` 是 Claude Code 自己的窗口约定（网关或自定义 ID 带 `[1m]` 时按 1M 假定），
   `CLAUDE_CODE_MAX_CONTEXT_TOKENS` 的生效方式随 ID 形状变化，不能只写数字。

据此，首版预览（只映射三个链接键）在维护者那份可用配置上会产生「一半新、一半旧」的状态。下一批要
么把类别键一起映射，要么在预览里把它们明确列为覆盖来源；顺序建议先做类别映射，再做窗口声明与自动
压缩，档位最后（受通道约束）。

三个 Agent 都把**选定的档位按模型**保存，只是形状不同：Claude Code 用 `modelSettings`，Pi 用
`settings.json` 的 `modelThinkingLevels`（键是 `provider/model`），Grok Build 用
`[[model.<alias>.reasoning_efforts]]` 数组表。这印证了使用值按 (Agent × 模型) 存，而不是按 Agent
存一个全局值；模型侧的「有哪些档位可选」则由模型行声明。

计划绑定三个变化令牌，任一变化都必须重新预览：目标设置文件的 SHA-256（文件不存在时明确标注）、
Provider `revision`、模型的内容摘要。模型行没有 `revision`，复用 `models/alias.rs` 已有的
SHA-256 内容摘要机制，不新增猜测字段。预览只读取目标文件本身；shell 环境、项目级与托管设置
不在本次读取范围内，以"未检查"提示出现。

## 批次

### B1：Rust 计划域与 Claude Code 映射（已交付 2026-10-13）

文件：`src-tauri/src/config_plan.rs`、`src-tauri/src/config_plan/tests.rs`、
`src-tauri/src/agents/claude_code.rs`、`src-tauri/src/agents/claude_code/tests.rs`，
以及 `lib.rs`、`agents.rs` 各一行模块注册。

- `config_plan.rs`：目标文件描述与摘要（存在性、SHA-256、符号链接标记）、计划来源、
  逐键变更、提示与拒绝原因；读取上限 1 MiB，空文件按空文档处理，非对象 JSON 与目录被拒绝。
- `agents/claude_code.rs`：Provider/模型事实到 Claude Code 键的映射、保留无关键、组合状态判定；
  合并后的文档只在本模块内部供写入阶段复用，不进入返回给界面的计划。
- 验证：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、
  `cargo test --locked` 通过（308 通过、1 忽略；本批新增 12 + 14 项），覆盖键映射、无关键保留、
  损坏/超大/非对象文件、未核实组合与自定义地址的拒绝、摘要绑定、密钥占位与符号链接标记。

这一批没有 IPC 与界面，因此没有用户可见变化，`CHANGELOG.md` 未新增条目。

### B2：IPC 命令（已交付 2026-10-13）

`commands/agents.rs` 的 `plan_claude_code_settings`、`commands/state.rs` 的存储解析、
`lib.rs` 注册，以及 `config_plan.rs` 的命令级错误码（`storage_unavailable`、
`provider_not_found`、`model_not_found`、`location_unavailable`、`operation_failed`）。
Claude Code 的设置根目录改由 discovery 的 `claude_config_root` 统一解析，检测与预览共用同一条
`CLAUDE_CONFIG_DIR` 规则，不再各写一份。

命令只读取数据库与目标文件：先读 Provider 记录，再读模型快照（含内容摘要）；两次读取各自的
令牌都进入计划，因此应用阶段必须重新核对。凭据值不参与读取。

验证：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、
`cargo test --locked` 通过（308 通过、1 忽略）。命令层按本仓库惯例不加单测：域逻辑由 B1 覆盖，
IPC 契约由 B3 的边界测试覆盖。

### B3：前端边界（已交付 2026-10-13）

`src/lib/desktop/agents.ts` 与其测试：`planClaudeCodeSettings(providerId, modelId)` 只在桌面上调用
IPC，浏览器预览返回 `{ kind: "preview" }` 且不下发 IPC。载荷按精确键集合校验（`agent`、`target`、
`source`、`changes`、`notices`、`restartRequired`），并检查跨字段一致性：文件存在性与内容摘要必须
同时存在或同时缺失、`secret` 与占位符必须一致、`changes` 非空、摘要为 64 位小写十六进制。拒绝码覆盖
Rust 的 12 个 `PlanError` 码，加上边界自身的 `invalid_response` 与 `desktop_required`；
畸形载荷抛 `PlanRequestError`，不与发现流程的错误类型混用。`providers.ts` 的 provider ID 校验改为
导出复用，避免两处实现同一规则。

验证：`pnpm run check:frontend` 通过（39 个测试文件、690 项通过）。边界文件自身 80 项，其中新增
合法计划、缺文件计划、12 个拒绝码、3 类非契约失败与 23 项畸形/矛盾载荷用例。本批不含界面，
因此没有需要浏览器验收的可见行为。

### B4：预览界面与模型槽位（已交付 2026-10-13）

`ConfigPreview.tsx` 与其测试、`AgentList.tsx` 的入口、`en.json` 与 `zh-CN.json`。Claude Code
详情页新增「模型配置」卡片：选择已保存的服务商与其中已添加的模型，生成计划后显示目标文件与状态、
逐键改动、提示与拒绝原因；改选任一侧都会丢弃已显示的计划。模型从不代为选择，浏览器预览明确说明
无法读写配置文件。

验证：`pnpm run check:frontend` 通过（40 个测试文件、699 项；`ConfigPreview.test.tsx` 9 项覆盖
选择、生成、拒绝原因、切换服务商丢弃旧计划、读取失败重试、空列表与意外错误降级）。浏览器预览核对：
卡片只出现在 Claude Code 详情页（Pi 详情页没有）、中文文案完整、无未解析的键。浏览器预览没有 IPC，
因此选择器与计划卡片本身只能由组件测试覆盖，真实窗口仍待桌面核对。

槽位状态的首期范围：预览只读取目标文件本身，因此界面能显示「未配置」（文件缺失或无相关键）与
「已有外部值」（文件里的当前值），不能显示「待重启」——vibemate 还没有写入过任何内容，运行中
会话的状态也不可观测。独立的只读状态行仍未实现；记录「待重启」需要 P20 的写入结果，不显示无法
证明的状态。

## 检查点 C18

预览能列出变更文件、覆盖来源、重启要求与密钥落盘方式；未核实组合被拒绝并说明；预览没有写入。
写入、回滚与真实推理仍在 P19–P21 与 P38 单独验收。
