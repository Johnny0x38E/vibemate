# 接入能力矩阵

协议与配置入口的文档证据核实于 2026-10-09（Claude Code 部分于 2026-10-13 补齐），
来源见 [Provider](providers.md)、[Claude Code](claude-code.md)、[Pi](pi.md)、
[Grok Build](grok-build.md) 接入文档。三个写入适配器已实现预览、应用与恢复；
这张表记录的是文档证据，真实组合请求仍待 P38 验收。

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

## P24 模型注入边界

服务商模块维护唯一模型定义；以下只记录各 Agent 已消费的字段，不再提供请求调参编辑器。
本地配置映射不等于真实推理或有效会话。Provider 端点/协议限制继续按上表生效。

| Agent       | 已注入的模型属性                                                                             | 思考档位与默认值                                                                                                                   | 尚未实现或核实                                                                            |
| ----------- | -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Claude Code | 单一模型注入；会话模型与 class alias 指向同一模型；仅 DeepSeek `deepseek-flash` 的上下文校正 | 已核实绑定的默认 env effort，从中央声明列表选取；独立记忆只是已有起始偏好                                                          | 其他 ID 的容量识别、独立输出/图像/工具声明；任意自定义模型内部切换                        |
| Pi          | 声明的上下文、最大输出、推理与图片输入                                                       | 每模型默认档位单独保存；官方 DeepSeek Chat `deepseek-flash` 的所有已声明 `xhigh/max` 映射独立注入，允许原生切换；保留用户 map/null | 其他厂商/模型映射待核实；当前默认由声明列表手选；无模型定义工具标记不代表模型不能调用工具 |
| Grok Build  | 受管 alias 的模型 ID、协议、上下文与已声明图像能力 (`vision`)                                | 现有用户 effort 字段保留，不新增弃用字段                                                                                           | 原生 `reasoning_efforts` 可切换数组与默认值尚未实现；独立最大输出与工具能力映射待核实     |

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
