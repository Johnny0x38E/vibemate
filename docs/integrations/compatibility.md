# 接入能力矩阵

协议与配置入口的文档证据核实于 2026-10-09（Claude Code 部分于 2026-10-13 补齐），
来源见 [Provider](providers.md)、[Claude Code](claude-code.md)、[Pi](pi.md)、
[Grok Build](grok-build.md) 接入文档。三个写入适配器已实现预览、应用与恢复；
映射基线证据为 Claude Code 2.1.304、Pi 1.4.0、Grok Build 1.0.67；本轮固定
`claude --version` 已返回 2.1.308，未因此重复全部调研，也未对 2.1.308 宣称新增真实组合验收。
下表 Claude 行版本指原有契约证据；Pi/Grok 原生样本平台为 macOS。
下面区分配置适配、原生会话/载荷样本与真实 Provider 结果；真实组合请求仍待 P24/P38 验收。

## Provider 到 Agent（配置写入）

| Provider     | Claude Code                                     | Pi                                                          | Grok Build                           | 当前结果                                                                            |
| ------------ | ----------------------------------------------- | ----------------------------------------------------------- | ------------------------------------ | ----------------------------------------------------------------------------------- |
| DeepSeek     | 官方网关配方，`/anthropic`                      | openai-completions，可配置 DeepSeek thinking/maxTokensField | chat_completions，自定义模型/env_key | Claude Code 与 Pi 可生成计划；真实消息请求待验                                      |
| OpenRouter   | 官方配方，`/api`，仅保证 Anthropic 一等厂商     | openai-completions，有专有 thinking/routing 兼容字段        | chat_completions，自定义 base_url    | 可生成计划并附兼容提示；模型/路由/参数逐项映射待验                                  |
| Command Code | 未核实：网关根与 `/messages` 仅服务 Claude 模型 | 按目录支持的 Chat/Responses/Messages 选择                   | 按模型 `api_backend` 选择            | Claude Code 组合拒绝生成计划；证据只覆盖 GOAT 套餐，Pro/Provider 套餐与模型权限待验 |

Grok Build 的 `messages` 后端保持 `protocol_unsupported`，`chat_completions` 与
`responses` 可以进入预览。模型能力来源与请求字段分开；Provider 目录返回不代表账户
授权，也不代表运行中的 Agent 使用了该配置。配置写入只表示文件已写入，不代表会话
已生效或推理成功。

## P24 九种组合的状态

`supported` 仅表示表内指定的配置通道有证据及实现，不表示该账户可调用模型。
`unsupported` 表示当前适配器拒绝该通道，`unverified` 表示不能证明兼容；后两者均不是验收通过。
每行的真实 Provider 结果目前均为 `unverified`，不得用目录获取或本地 stub 替代。

| Provider     | Agent               | 配置通道状态                                               | 模型档位与默认规则                                                            | 原生/请求级证据与剩余限制                                                                             |
| ------------ | ------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| DeepSeek     | Claude Code 2.1.304 | supported：官方 Messages 网关                              | 精确 `deepseek-flash` 支持声明列表内的显式 env 默认档；不覆盖则保留原值       | 官方配方及配置测试；无每模型原生隔离，任意 ID 的容量识别/能力表和内部切换仍 unverified                |
| OpenRouter   | Claude Code 2.1.304 | supported with limits：官方网关，仅保证 Anthropic 模型     | 保留原生 effort；不向其它模型套用 DeepSeek 默认档规则                         | 无通用容量/能力声明或第三方 canonical 匹配证明；真实模型/路由 unverified                              |
| Command Code | Claude Code 2.1.304 | unverified：缺少官方 Claude Code 网关契约，拒绝计划        | 不生成默认档位或能力写入                                                      | Messages 路由存在不足以证明该 Agent 组合可用                                                          |
| DeepSeek     | Pi 1.4.0            | supported：按保存协议注入；Chat 使用官方兼容格式           | 官方 Chat 的 Flash/Pro 注入已声明 `xhigh→high/max→max`；显式默认单独保存      | 原生菜单/同 ID 隔离/null 排除已验；Chat SDK 载荷发送前捕获通过；其它 API 的厂商档位 unverified        |
| OpenRouter   | Pi 1.4.0            | supported：按保存协议注入；Chat 使用网关兼容格式           | 官方 Chat 注入已声明 `xhigh→xhigh/max→max`；默认不自动选用；保留用户 map/null | SDK 发送前确认嵌套 `reasoning.effort`；Responses/Messages 不复用该映射，真实 Provider 接受 unverified |
| Command Code | Pi 1.4.0            | supported：按模型已保存的 Chat/Responses/Messages 路由     | 基础声明档位可显式保存，未核实扩展档位不生成厂商 map；用户 map 仍由 Rust 检查 | 目录不提供努力能力表，用户声明不升级为套餐授权或请求兼容证明                                          |
| DeepSeek     | Grok Build 1.0.67   | supported：Chat/Responses；Messages unsupported            | 缺失的原生列表注入中央声明，列表内默认标 true；用户数组与全局默认不改         | 菜单/会话选择和两 backend 载荷样本已验；DeepSeek 协议/模型接受仍 unverified                           |
| OpenRouter   | Grok Build 1.0.67   | supported：Chat/Responses；Messages unsupported            | 同上，不猜上游努力值或 token 数                                               | Chat 输出顶层 `reasoning_effort`，Responses 输出嵌套 `reasoning.effort`；真实路由/模型接受 unverified |
| Command Code | Grok Build 1.0.67   | supported：目录允许的 Chat/Responses；Messages unsupported | 同上，仅中央明确声明列表                                                      | 本地 backend 字段已验，不证明各套餐、上游模型、图像或工具兼容                                         |

Grok 两种 backend 另已确认运行中 `high→max` 后下一回环请求发送 max，并将选择保存到
全局 `[models].default_reasoning_effort`，新会话优先采用该值。它不是每 alias 持久默认；
vibemate 不写这个全局值。Pi 的每模型保存与 Grok 的全局保存不能互相类推。

证据与复验：[字段矩阵](model-parameter-mapping.md)、[Pi](pi.md#p24c-openrouter-与-deepseek-pro-映射证据)、
[Grok](grok-build.md#p24d-本地-backend-载荷验证)、[Provider schema](providers.md)。
DeepSeek Flash 的上下文与 Claude env 默认档证据不能泛化到 DeepSeek Pro、
OpenRouter 同名模型或 Command Code 同名模型。模型 ID 及 Provider 实例共同构成绑定身份。

## P24 模型注入边界

服务商模块维护唯一模型定义；以下只记录各 Agent 已消费的字段，不再提供请求调参编辑器。
本地配置映射不等于真实推理或有效会话。Provider 端点/协议限制继续按上表生效。

| Agent       | 已注入的模型属性                                                                             | 思考档位与默认值                                                                                                | 尚未实现或核实                                                                                        |
| ----------- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Claude Code | 单一模型注入；会话模型与 class alias 指向同一模型；仅 DeepSeek `deepseek-flash` 的上下文校正 | 已核实绑定的默认 env effort，从中央声明列表选取；独立记忆只是已有起始偏好                                       | 其他 ID 的容量识别、独立输出/图像/工具声明；任意自定义模型内部切换                                    |
| Pi          | 声明的上下文、最大输出、推理与图片输入                                                       | 每模型默认单独保存；官方 DeepSeek Chat Flash/Pro 与 OpenRouter Chat 的已声明扩展档位独立注入；保留用户 map/null | 其他厂商/模型映射待核实；当前默认由声明列表手选；无模型定义工具标记不代表模型不能调用工具             |
| Grok Build  | 受管 alias 的模型 ID、协议、上下文、已声明图像能力 (`vision`) 与已声明 effort 列表           | 非空声明列表写入 `reasoning_efforts`，列表内中央默认档标 `default = true`；已有数组保留                         | 原生菜单、会话选择及 Chat/Responses 本地载荷样本已验；真实 Provider 接受未验；无独立输出/工具能力字段 |

Temperature/top_p、请求输出限制和推理 token 数不是本轮注入控件，不增加新模型字段来填这些原生 key。
已有用户请求设置保留。未声明或 `any` 不扩展成所有档位；未核实不等于不支持，也不能显示为已注入。
真实组合验收仍在 P38，由维护者在 Agent 内使用注入模型检查。

## Skills、MCP 与开关

Skill 与 MCP 的中央库已实现，逐 Agent 部署尚未实现。

| 项目             | Pi                                      | Grok Build                                     |
| ---------------- | --------------------------------------- | ---------------------------------------------- |
| Skill 用户级目录 | agent-dir/skills 与 .agents/skills      | GROK_HOME/skills 与 .agents/兼容来源           |
| Skill 停用       | 资源精确排除或撤销受管部署              | skills.disabled 或撤销受管部署                 |
| symlink          | macOS 临时发现通过                      | macOS 临时发现通过                             |
| 原生 MCP         | stdio、streamable HTTP                  | stdio、HTTP；旧 SSE 暂不承诺                   |
| MCP 停用         | enabled=false，关闭条目保留在解析结果中 | enabled=false；本机 inspect 省略关闭的原生条目 |
| 项目 MCP 覆盖    | 可只覆盖 enabled/exposure，保留用户定义 | 同名项目条目完整替换用户定义                   |
| 其他来源         | 扩展可替换内置 MCP                      | Claude/Cursor/插件/管理策略可能贡献或覆盖配置  |

Claude Code 的 MCP 作用域与 Skill 目录见 [Claude Code 接入文档](claude-code.md)。

## 实施边界

三个 Agent 均有原生模型、Skill、MCP 配置入口，目前无需先决的桥接方案。真实 Agent
部署仍须预览、并发检查、备份、原子替换、保留未管理字段与回滚，规则见
[configuration.md](../configuration.md)。无原生支持时明确标记为不支持、不实现额外
桥接（维护者 2026-10-13 决定）。

会话刷新、凭据注入、Windows/Linux 系统行为和九种组合的真实调用留给 P37/P38。
