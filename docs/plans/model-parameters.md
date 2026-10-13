# P16 / P18：模型信息与 Agent 使用设置

## 已确认的职责

维护者确认：请求参数在 Agent 配置/注入流程设置，不放在服务商模型页。
任务状态只记录在 [todo.md](todo.md)，本文件记录设计与依据。

| 位置       | 职责                                                         |
| ---------- | ------------------------------------------------------------ |
| 服务商 API | Base URL、凭据引用、协议                                     |
| 服务商模型 | 获取/添加模型、别名、查看上游声明的能力与来源                |
| Agent 配置 | 选择 Provider/Model，设置该 Agent 可表达的使用选项，预览差异 |
| Rust       | 按 Provider、模型、协议和 Agent 版本检查组合，再生成配置计划 |

同一个模型供不同 Agent 使用时可以有不同设置。修改某个 Agent 的使用设置不改变
中央模型能力，不自动改动其他 Agent。能力未知或手动补充不等于已验证支持。

## 现状与边界

当前模型记录保存名称、上下文/输出上限、模态、路由和逐字段能力证据。别名已有带
内容摘要保护的独立编辑操作：Rust 读取模型并返回 SHA-256 摘要，只有摘要未变才写入，
过期编辑返回 `model_changed` 并要求重新加载（[provider-completion.md](provider-completion.md)）。
模型行仍没有请求参数，也没有供配置预览绑定的模型 revision，预览过期检查留在 P18。
SQLite 最新迁移为 v13（v11 为技能上限重建，v12 为并行工作区的 Agent 自动探测偏好保留、本分支留空占位，v13 为能力列）；新增持久化时追加迁移，
不改写旧 SQL。

能力上限和请求设置不是同一字段。例如 Pi 模型定义中的 `maxTokens` 描述模型上限，
不能直接当作每次请求想生成的 token 数量。Agent 配置只能提供已核实版本可表达的
设置；若不能表达某个请求参数，应明确拒绝该设置，不假设存在透传接口。

本计划不增加推理请求、API 转发或 Agent 配置写入行为。P18 只预览；P19–P21 再
实现可靠写入与恢复。真实推理和平台验收仍独立记录。

## 官方证据与缺口

上一轮只读取公开文档，未读取凭据或请求模型接口。下列证据保留，不因职责调整
将未知字段改成已支持。

- [Command Code Provider API](https://commandcode.ai/docs/provider)：浏览器读取成功。
  三条推理路由分别遵循 OpenAI Chat、OpenAI Responses、Anthropic Messages schema。
  `supported_endpoints` 只说明路由声明，不证明图像、工具或推理参数支持。
- [OpenRouter 模型目录](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)：
  浏览器读取成功，示例含 `supported_parameters` 和 `top_provider.max_completion_tokens`。
  目录中的参数名不能单独决定每个协议或 Agent 的配置字段映射。
- [OpenRouter 推理设置](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)：
  浏览器读取成功。`reasoning.supported_efforts` 缺失表示未公开努力选项；`null` 表示
  接受 gateway 的全部努力值；数组表示列出的选项，三者不能混同。
  `supports_max_tokens` 为 true 时公开推理 token 数量设置；`mandatory` 为 true 时禁止关闭。
  同页对努力与 token 数量并用的说明不一致，未来提供该设置时采用二选一，并核实
  目标 Agent 是否能够表达，不在服务商页开放请求参数。
- [DeepSeek 模型目录](https://api-docs.deepseek.com/api/list-models/)：2026-10-13 重新核实
  可访问，页面记录 `name`、`context_window`、`max_output_tokens`、
  `input_modalities`（`text`/`image`）、`output_modalities`、
  `effort.supported_levels`/`default_level` 与
  `api_capabilities.anthropic_messages.system_prompt_update`。旧记录中“无法确认扩展字段”
  已由本次核实替代；工具调用不在该目录里，由官方定价页的逐模型能力表提供。
- [DeepSeek Chat schema](https://api-docs.deepseek.com/api/create-chat-completion/)：
  上一轮仅取得 Context7 官方来源摘录，包含 `reasoning_effort` 的
  `none/low/high/max`、兼容别名和 thinking 模式下 temperature 不生效的说明。
  摘录不能代替其他协议或 Agent 版本的映射证据。

## P16：中央模型元数据

P16 保留目录能力解析、来源记录、持久化和只读显示，以及独立的别名编辑任务。
不保存全局请求输出长度、思考强度、思考 token 数量或任意请求 JSON。

来源按字段记录；不能用模型行最初的 `source = manual/fetched` 代替当前能力来源，
因为手动模型也可能在之后被上游列出。重新获取只更新上游元数据，不覆盖别名或
各 Agent 的设置。缺失、null、空列表、畸形值按对应协议解释；不能转换成肯定支持。

需要在 Agent 配置中补充未知能力或覆盖中央描述时，该覆盖属于目标 Agent，明确
标为用户声明，不将中央模型改成“已支持”。

涉及编辑或配置预览的元数据必须有并发/过期保护。实现时选择模型 revision 或
稳定内容摘要，并补齐所有影响预览的更新路径；现有时间戳不能直接充当 revision。
新增中央模型 revision 的任务留在 P16，P18 最小路径可采用经验证的内容摘要，
但不能跳过模型变化检查。

实现状态（2026-10-13）：逐字段能力解析、来源记录、v13 持久化与列表图标已完成，
只对“明确支持”显示图标；别名编辑使用 SHA-256 内容摘要做并发保护，过期编辑返回
`model_changed`；已保存模型的目录刷新（只更新已保存行、保留别名、完整目录才标记缺失）
与服务商安全删除（确认、并发保护、凭据补偿与未知结果反馈）均已完成。字段证据与映射
规则见 [providers.md](../integrations/providers.md)。供 Agent 预览绑定的模型 revision
仍未完成，拆分与验收要求见 [provider-completion.md](provider-completion.md)；P35 的
Agent 引用检查不在此列。

## P18：以 Pi 配置预览为下一主线

P18 不再等待完整 P16。先核实已安装 Pi 版本和官方配置规则，仅按最小路径补充所需
模型信息。别名编辑和整个模型目录扩展不是 Pi 预览的前置条件。

1. 核实 Pi 支持的模型定义、使用选项、配置位置、优先级和凭据表达方式。
2. 在 Agent 配置流程选择服务商和模型，说明缺失信息；必要时允许明确的 Agent 局部
   用户声明，不猜测能力。最小路径先不提供未经核实的请求参数。
3. Rust 生成保留无关字段的脱敏计划，绑定文件摘要、Provider revision 和模型
   revision/内容摘要；展示不兼容原因、覆盖来源和生效要求，不写文件。
4. 根据 Pi 的实际表达能力逐项增加使用设置，每项一起完成校验、双语反馈、预览
   映射与测试。不先建立无人使用的通用参数面板。

参数未设置表示不覆盖目标原生行为，不保存猜测的服务端默认值。若引入继承，必须
明确来源及其变化检查。每次校验绑定目标 Agent、版本、Provider 实例、模型和解析协议。
改变任何一项都需要重新校验。

努力值、推理 token 数量、启用和隐藏输出是不同概念，不互相猜测转换。
结构化校验原因由 UI 翻译；未知/不支持明确反馈，不静默丢弃设置。

请求头和扩展参数仅在实际 Agent 路径需要且支持时增加 allowlist。禁止覆盖认证或
传输控制字段，禁止任意 JSON 透传。敏感头值必须使用 OS 凭据引用，不能存入普通
参数 JSON、日志或导出。Grok 的映射在 P22 单独核实，不照搬 Pi 设置。

## 实施与验证

下一批先做 P18 的版本/配置规则核对与最小预览拆分，实施计划直接放在 `docs/plans/`。
每批列出最多五个实际文件、输入输出与验收标准。原先“先实现独立 Rust 请求参数
校验模块”的安排已撤回，相关规则随 Agent 使用选项逐项实现。

P16 批次测试目录解析、未知状态、迁移保留和上游更新保留别名。
P18 批次测试配置格式保留、优先级、局部设置隔离、参数兼容、脱敏与过期计划，
并验证预览没有写入副作用。IPC/UI 变更运行完整 `check:frontend`，Rust 变更运行
fmt、Clippy 与测试，检查点做锁定本机桌面构建。真实推理、Windows/Linux 原生
行为与后续写入恢复的验证不以文档检查或预览测试代替。
