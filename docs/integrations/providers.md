# Provider 接入证据

核实日期：2026-10-09。对应执行任务 P01。

本文件记录公开官方资料能够确认的协议与字段，不表示 vibemate 已完成接入。
本次没有读取用户凭据、发送模型推理请求或修改 Agent 配置。目录、权限、字段和
模型别名可能更新；实现适配器时应重新核对，并记录实际安装版本和测试结果。

## 产品身份与端点

| 配置身份            | 产品与官方来源                                                                                         | 本阶段主要协议                                               | 基础地址                                 | 模型发现                                  |
| ------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ | ---------------------------------------- | ----------------------------------------- |
| `command-code-goat` | [Command Code GOAT](https://commandcode.ai/docs/plans/goat)，Command Code 的订阅档位                   | 按模型选择 Chat Completions、Responses 或 Anthropic Messages | `https://api.commandcode.ai/provider/v1` | `GET /models`，读取 `supported_endpoints` |
| `deepseek`          | [DeepSeek 官方 API](https://api-docs.deepseek.com/en/)                                                 | 首条路径使用 Chat Completions                                | `https://api.deepseek.com`               | `GET /models`                             |
| `openrouter`        | [OpenRouter 官方 Chat API](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion) | 首条路径使用 Chat Completions                                | `https://openrouter.ai/api/v1`           | `GET /models`，支持有界分页               |

这里的配置身份是 vibemate 的设计命名，不是外部 API 的模型 ID。
DeepSeek 直连与经 Command Code/OpenRouter 使用的 DeepSeek 模型有不同的 Provider
身份、账户、ID 与参数约定，不能仅按显示名称合并。

## Command Code GOAT

[官方 Provider API 文档](https://commandcode.ai/docs/provider)明确将 GOAT 列为有
API 访问权限的编码套餐。通过 Studio 创建密钥；同一账户密钥用于 CLI 和 API，
API 调用受所属套餐的模型权限和额度限制。vibemate 不把 GOAT 当作另一个服务商，
也不因模型出现在公共目录里就认定当前账户有权限。

推理认证使用 `Authorization: Bearer <API_KEY>`，JSON 请求使用
`Content-Type: application/json`。官方模型列表示例没有认证头；读取公共目录
不能证明密钥有效。实际鉴权与套餐权限留给显式连接检查和真实验收。

| 路由               | 方法与相对路径           | 适配约束                                                        |
| ------------------ | ------------------------ | --------------------------------------------------------------- |
| Chat Completions   | `POST /chat/completions` | OpenAI/open 模型；按模型声明确认                                |
| Responses          | `POST /responses`        | OpenAI 与多数 open 模型；客户端执行的工具与服务端工具要分开     |
| Anthropic Messages | `POST /messages`         | 官方说明 Claude 模型只走这条路由；不能强制改为 Chat Completions |
| Models             | `GET /models`            | 用 `supported_endpoints` 校验所选路由                           |

模型的工具、图像与推理参数按其路由的 schema 和实际能力映射。`supported_endpoints`
只说明路由，不能代替工具或图像能力证据。官方声明按对应 OpenAI/Anthropic schema
传递请求，但每个模型的完整参数支持仍需验证；未知能力保持未知。
Responses 的远程 `type: "mcp"` 工具被拒绝，不能把 Provider API 的工具传输当成
Agent 原生 MCP 支持。决定模型的 System One 路由不属于第一阶段编码聊天路径。

错误按协议区分：OpenAI 格式包含 `error.message/type/code/param`；Anthropic
格式应独立处理。官方文档还列出 ZDR 无可用上游时的 `422` 与
`cmd_zdr_no_providers`，不能静默取消用户要求的 ZDR。此项尚未实现。

## DeepSeek

[首次请求文档](https://api-docs.deepseek.com/en/)确认 Bearer API key 认证与上述
基础地址。[模型目录](https://api-docs.deepseek.com/api/list-models/)现在不仅返回
ID，还声明 `name`、`context_window`、`max_output_tokens`、`input_modalities`、
`output_modalities`、`effort.supported_levels/default_level` 和按协议区分的
`api_capabilities`。这些是能力信息，不能全部当作请求参数直接提交。

[模型说明](https://api-docs.deepseek.com/quick_start/pricing/)当前列出
`deepseek-flash` 与 `deepseek-v4-pro`；Flash 支持图像输入，Pro 不支持。
旧 Flash 别名对应的原模型已退役，不能把仍被接受的别名展示成独立的新能力。
两者支持工具、推理、Responses 与 Anthropic 格式；第一条实现路径先使用 Chat
Completions，其余协议没有据此自动加入支持范围。

[Chat API schema](https://api-docs.deepseek.com/api/create-chat-completion/)的关键映射：

| 含义          | 字段与约束                                                                                                        |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| 模型与消息    | `model`、`messages`；模型 ID 来源于目录                                                                           |
| 推理开关/努力 | `thinking.type`、`reasoning_effort`；当前努力值为 `none/low/high/max`，保存两种设置时需明确冲突规则               |
| 输出上限      | `max_tokens`，结合所选模型目录中的输出与上下文上限校验                                                            |
| 采样          | `temperature` 在 thinking 模式不生效；`top_p` 有模式相关行为，不能按通用 OpenAI 假设校验                          |
| 工具          | `tools[].type = function`、`function.name/parameters`；`tool_choice` 的 `required`/指定函数在 thinking 模式被拒绝 |
| 输出          | `choices[].message.content`、`reasoning_content`、`tool_calls`；流式结束与 usage 按官方 schema 处理               |
| 过时字段      | `frequency_penalty`、`presence_penalty` 已标弃用且不生效，不新增为可用设置                                        |

严格工具 schema 和 prefix/FIM 属于 Beta 能力，不作为第一阶段默认路径。
图像输入仅在模型能力声明允许时开放，并按协议的内容部件格式转换。

[官方错误说明](https://api-docs.deepseek.com/quick_start/error_codes/)区分
`400` 格式、`401` 认证、`402` 余额、`422` 参数、`429` 限流、`500` 服务错误和
`503` 过载。本页不是完整错误体 schema；具体错误体字段保留待协议样本验证，
不能仅凭 OpenAI 兼容字样声称每个错误体完全相同。

## OpenRouter

[认证原始文档](https://github.com/OpenRouterTeam/docs/blob/main/api_reference/authentication.mdx)
确认 `Authorization: Bearer <API_KEY>`；归属标识请求头是可选项，不是密钥认证的替代。
使用 Rust 发请求，不能把官方 JavaScript 示例直接复制进 React。

[模型目录](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)
返回 `id`、`architecture.input_modalities/output_modalities`、`context_length`、
`top_provider.max_completion_tokens` 和 `supported_parameters`。模型 ID 保留组织前缀。
使用 `offset/limit` 和返回的分页信息获取有界结果，不假定目录固定或全部模型可访问。

[Chat API](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion)声明
`tools`、`tool_choice`、`parallel_tool_calls`、采样和输出限制等字段。
当前文档将 `max_tokens` 标为弃用并建议 `max_completion_tokens`；这与 DeepSeek
的字段不同，必须显式映射，不能统一透传。
[官方请求 schema](https://github.com/OpenRouterTeam/docs/blob/main/api_reference/overview.mdx)
说明图像内容使用 `image_url` 部件，URL 或 base64 数据均可；只对有图像能力的模型允许。

[推理说明](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)使用
`reasoning` 对象。努力、预算、启用与隐藏输出是不同概念；依据模型目录的
`supported_efforts`、`supports_max_tokens`、`mandatory` 等声明显示可用选项。
官方总览可能保留旧字段示例，映射以当前端点 schema 为准。对官方页面内部存在的
努力/预算组合说明差异，第一阶段采用显式二选一，不猜测同时传入的效果。

[官方错误文档](https://github.com/OpenRouterTeam/docs/blob/main/api_reference/errors-and-debugging.mdx)
规定 Chat 错误体为 `error.code/message/metadata`。HTTP `200` 后的响应体或 SSE
仍可能包含生成错误，不能仅检查 HTTP 状态就显示连接成功。错误体可能携带原始
输入与上游信息，不能直接写日志、导出或展示。

## 第一条兼容路径与待核实事项

首条路径仍选择 **DeepSeek → Pi 的 Chat Completions 配置**。
[Pi 官方模型文档](https://pi.dev/docs/latest/models)支持在 `models.json` 配置
`baseUrl`、`api: "openai-completions"` 与模型列表，和 DeepSeek 的协议入口匹配。
这确认的是文档级协议路径：本机 Pi 的准确发行版本、配置位置、认证优先级和
实际参数传递必须由 P02 验证；尚未完成真实调用。

| 组合                       | 当前证据状态                                                    |
| -------------------------- | --------------------------------------------------------------- |
| DeepSeek → Pi              | 文档级 Chat 协议兼容；P02 核对安装版本和参数支持                |
| OpenRouter → Pi            | 文档级 Chat 协议匹配；完整账户/模型/参数映射待 P02/P24          |
| Command Code GOAT → Pi     | 按模型路由选协议；不能将 Claude 映射为 Chat；完整映射待 P02/P24 |
| 三家 Provider → Grok Build | Agent 的准确产品和配置接口由 P03 确认，目前不声称兼容           |

后续连接检查需要区分网络可达、密钥有效、套餐可用和模型请求成功。
模型列表返回成功不能替代上述全部状态。参数校验同时考虑 Provider、模型与 Agent，
没有已确认映射的选项应拒绝注入并解释原因，不允许静默丢弃。

## 脱敏请求体示例

以下仅说明字段，没有执行。密钥从 Rust 凭据引用解析成请求头，不出现在 JSON、URL
或日志中。目录占位符需在实际调用前替换为所选模型 ID。

DeepSeek 首条文本路径：

```json
{
    "model": "deepseek-flash",
    "messages": [{ "role": "user", "content": "Hello" }],
    "thinking": { "type": "disabled" },
    "max_tokens": 1024,
    "stream": false
}
```

Command Code 选择声明支持 Chat Completions 的 GOAT 模型：

```json
{
    "model": "<GOAT_CHAT_MODEL_ID_FROM_CATALOG>",
    "messages": [{ "role": "user", "content": "Hello" }],
    "stream": false
}
```

OpenRouter 选择声明支持所需输出参数的文本模型：

```json
{
    "model": "<OPENROUTER_MODEL_ID_WITH_ORGANIZATION_PREFIX>",
    "messages": [{ "role": "user", "content": "Hello" }],
    "max_completion_tokens": 1024,
    "stream": false
}
```

## 本次核实边界

通过 ego-browser 阅读上述官方页面与官方文档源码，核对身份、端点与字段。
未依赖第三方插件推断套餐端点。示例经过 JSON 解析；文档链接读取成功。
没有验证真实密钥、余额、全部模型参数或安装后的 Pi/Grok Build 行为，
没有请求或执行任何 MCP Server，也没有引入 Provider 实现。
