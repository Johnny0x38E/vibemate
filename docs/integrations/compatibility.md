# 第一阶段接入能力矩阵

核实日期：2026-10-09。对应检查点 C03，来源见
[Provider](providers.md)、[Pi](pi.md)、[Grok Build](grok-build.md) 接入文档。
这张表记录协议与配置入口；所有业务适配器和真实组合请求尚未实现。

## Provider 到 Agent

| Provider          | Pi 1.1.0                                                    | Grok Build 1.0.50                    | 当前结果                                                      |
| ----------------- | ----------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------- |
| DeepSeek          | openai-completions，可配置 DeepSeek thinking/maxTokensField | chat_completions，自定义模型/env_key | 有文档级路径与配置解析证据；首条选 Pi，请求参数和真实调用待验 |
| OpenRouter        | openai-completions，有专有 thinking/routing 兼容字段        | chat_completions，自定义 base_url    | 文档级协议匹配；模型/路由/参数逐项映射待验                    |
| Command Code GOAT | 按目录支持的 Chat/Responses/Messages 选择                   | 按模型 api_backend 选择              | Claude 使用 Messages；不能统一当 Chat；套餐认证与模型权限待验 |

模型能力来源与请求字段分开；Provider 目录返回不代表账户授权，也不代表运行中的
Agent 使用了该配置。绿色表达已启用的配置关系，不代表模型推理连接已验证。

## Skills、MCP 与开关

| 项目             | Pi                                      | Grok Build                                     |
| ---------------- | --------------------------------------- | ---------------------------------------------- |
| Skill 用户级目录 | agent-dir/skills 与 .agents/skills      | GROK_HOME/skills 与 .agents/兼容来源           |
| Skill 停用       | 资源精确排除或撤销受管部署              | skills.disabled 或撤销受管部署                 |
| symlink          | macOS 临时发现通过                      | macOS 临时发现通过                             |
| 原生 MCP         | stdio、streamable HTTP                  | stdio、HTTP；旧 SSE 暂不承诺                   |
| MCP 停用         | enabled=false，关闭条目保留在解析结果中 | enabled=false；本机 inspect 省略关闭的原生条目 |
| 项目 MCP 覆盖    | 可只覆盖 enabled/exposure，保留用户定义 | 同名项目条目完整替换用户定义                   |
| 其他来源         | 扩展可替换内置 MCP                      | Claude/Cursor/插件/管理策略可能贡献或覆盖配置  |

## 实施边界

两家 Agent 均有原生模型、Skill、MCP 配置入口，目前无需先决的桥接方案。
P04 可以开始建立现有界面的行为测试；P07–P09 中英文基础完成前不实施业务页面。
真实 Agent 注入仍须预览、并发检查、备份、原子替换、保留未管理字段与回滚。
所有会话刷新、凭据注入、Windows/Linux 系统行为和六种组合的真实调用留给后续任务。
