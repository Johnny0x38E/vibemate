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
