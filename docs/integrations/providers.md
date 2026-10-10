# Provider 接入证据

核实日期：2026-10-09。对应执行任务 P01。

本文件记录公开官方资料能够确认的协议与字段，不表示 vibemate 已完成接入。
本次没有读取用户凭据、发送模型推理请求或修改 Agent 配置。目录、权限、字段和
模型别名可能更新；实现适配器时应重新核对，并记录实际安装版本和测试结果。

## 产品身份与端点

| 配置身份       | 产品与官方来源                                                                                                                        | 本阶段主要协议                                               | 基础地址                                 | 模型发现                                  |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------- | ----------------------------------------- |
| `command-code` | [Command Code Provider API](https://commandcode.ai/docs/provider)；P01 证据依据其 [GOAT 套餐](https://commandcode.ai/docs/plans/goat) | 按模型选择 Chat Completions、Responses 或 Anthropic Messages | `https://api.commandcode.ai/provider/v1` | `GET /models`，读取 `supported_endpoints` |
| `deepseek`     | [DeepSeek 官方 API](https://api-docs.deepseek.com/en/)                                                                                | 首条路径使用 Chat Completions                                | `https://api.deepseek.com`               | `GET /models`                             |
| `openrouter`   | [OpenRouter 官方 Chat API](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion)                                | 首条路径使用 Chat Completions                                | `https://openrouter.ai/api/v1`           | `GET /models`，支持有界分页               |

这里的配置身份是 vibemate 的设计命名，不是外部 API 的模型 ID。
DeepSeek 直连与经 Command Code/OpenRouter 使用的 DeepSeek 模型有不同的 Provider
身份、账户、ID 与参数约定，不能仅按显示名称合并。

## Command Code

vibemate 以一个服务商身份 `command-code` 表示 Command Code；订阅套餐不是独立的
服务商身份。[官方 Provider API 文档](https://commandcode.ai/docs/provider)明确将
Command Code 的 GOAT 套餐列为有 API 访问权限的编码套餐。通过 Studio 创建密钥；
同一账户密钥用于 CLI 和 API，API 调用受所属套餐的模型权限和额度限制。vibemate
不因模型出现在公共目录里就认定当前账户有权限。

**证据范围：** P01 只核实了 Command Code 的 GOAT 套餐具有 API 访问权限；本节的
路由、按模型选择协议和错误说明来自同一份 Provider API 文档，核实时以 GOAT 套餐为
对象。维护者告知 Pro 与 Provider 套餐也支持 API 连接，但本文件尚未记录这两个套餐
的官方证据；它们的 API 访问、模型权限、额度和可用路由是否与 GOAT 套餐相同仍待
P15 核实，不能据此推断。

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

P10 的 Command Code 账户实例暂只允许 `chat_completions`：路由按模型声明选择，实例级默认值只覆盖与
Pi 首条路径一致的协议；P12 保存每个模型的 `supported_endpoints` 并据实例协议判断能否选用；按模型的 Responses/Anthropic Messages 覆盖留给 P15。

## P10 保存配置时的校验边界

以下规则由 `src-tauri/src/providers.rs` 执行，适用于所有服务商，只校验格式，不连接地址：

- 基础 URL 先去掉首尾空白；剩余部分只要含空白或控制字符（含 Tab、换行），就以
  `base_url_invalid` 拒绝。WHATWG URL 规则会静默删除 URL 内的 Tab 和换行，
  不先拒绝的话，保存的地址会与用户看到的不同。
- 规范化只去掉一个结尾 `/`：`https://a.com/v1//` 保存为 `https://a.com/v1/`；
  scheme 和 host 转小写，国际化域名转 punycode，不补任何路径。
- 回环地址和私有网络地址（如 `https://127.0.0.1`、`https://192.168.1.10`）
  允许；P12（原 P13 已并入）决定继续允许，理由见「模型列表获取（P12）」。
- 显示名称去掉首尾空白后为 1–64 个字符，拒绝控制字符，也拒绝会隐藏文字或改变
  显示方向的 Unicode 格式字符：零宽空格、BOM、词连接符、软连字符，以及双向控制符
  U+061C、U+200E/U+200F、U+202A–U+202E、U+2066–U+2069。ZWJ/ZWNJ（U+200D/U+200C）
  因 emoji 序列和波斯语、印度系文字需要而允许，但名称必须含可见字符。

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

## 模型列表获取（P12）

核实日期：2026-10-10（P12.0）。协议事实引用三家官方文档，Command Code 的响应结构另有
一次经维护者批准的真实请求（D16）；仍未写明的内容保持未知，由 P12.d 的真实获取确认。
实现设计与上限值见 `docs/plans/todo.md` 的 P12。获取只由用户点击「获取模型」触发。
**获取成功不代表密钥有效**：Command Code 的列表不带密钥也返回 200，因此列表请求成功
不能证明密钥有效、套餐可用或模型请求能成功，界面文案不能这样暗示。

**Command Code**（[Provider API](https://commandcode.ai/docs/provider)）：

- 端点为 `GET https://api.commandcode.ai/provider/v1/models`（"Models list"）。官方说明
  "Each model in the models list … carries a `supported_endpoints` field with the routes
  that serve it, so check there before you pick an endpoint"；OpenAI 与开源模型走
  `/v1/chat/completions`，多数也支持 `/v1/responses`，Claude 模型只走 `/v1/messages`。
- 列表示例 `curl https://api.commandcode.ai/provider/v1/models` 没有认证头；错误表中 `401`
  `authentication_error` 为 "Missing or invalid auth - pass `Authorization: Bearer` (any
  route)"，`403` `upgrade_required` 表示 Go 套餐没有 API 访问权限。
- 同页写明 "Every plan except the Go plan has API access - GOAT, Pro, Max, Team, and
  Provider"；本文件开头的套餐证据范围由 P15 据此正式更新。
- 官方文档没有写明响应结构。经维护者批准（D16），2026-10-10 20:10（UTC+8）不带认证头
  请求 `GET https://api.commandcode.ai/provider/v1/models`，得到 HTTP 200、17079 字节：
    - 结构为 `{ "object": "list", "data": [...] }`，没有分页字段，共 87 个模型。
    - 每个模型恰有 7 个字段：`id`、`object`（`"model"`）、`created`（秒级时间戳）、
      `owned_by`（`"command-code"`）、`name`、`context_length`（integer）、`supported_endpoints`。
    - `supported_endpoints` 的值**不带** `/v1` 前缀，与文档写法不同，只出现三种组合：
      `["/chat/completions", "/responses"]` 68 个，`["/messages"]` 11 个（均为 Claude），
      `["/chat/completions"]` 8 个。vibemate 比较时同时接受带与不带 `/v1` 前缀的值。
    - 65 个模型 ID 含 `/`（如 `mistral/mistral-large-4`），模型 ID 校验、存储与游标都必须
      支持 `/`。
    - 响应原样保存为 `src-tauri/tests/fixtures/command-code-models-2026-10-10.json`，
      不含任何密钥，供解析测试使用。
- 这次请求说明 `/models` 不要求密钥。单次观察不能说明服务端是否会按密钥过滤列表。

**DeepSeek**（[Lists Models](https://api-docs.deepseek.com/api/list-models/)、
[API 认证](https://api-docs.deepseek.com/api/deepseek-api)）：

- `GET /models` 返回 `object`（恒为 `list`）与必填数组 `data`。模型必填 `id`、`object`
  （`model`）、`owned_by`；可选 `name`、`context_window`（integer）、`max_output_tokens`
  （integer）、`input_modalities`（`text`/`image`）、`output_modalities`（`text`）、
  `effort.supported_levels/default_level`、`api_capabilities.anthropic_messages.system_prompt_update`。
- API 总览的认证方式为 "HTTP: Bearer Auth"，列表页没有声明例外，因此按文档 `/models`
  需要密钥。**未知**：不带密钥时的实际行为（不为此另发请求）。
- P12 只保存显示字段 `id`、`name`、`context_window`、`max_output_tokens` 与两个模态数组；
  `effort` 与 `api_capabilities` 留给 P16。

**OpenRouter**（[List all models](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)、
[OpenAPI](https://openrouter.ai/openapi.json) 的 `/models` 与 `ModelsListResponse`）：

- 查询参数 `offset`（默认 0，≥ 0）与 `limit`（默认 500，1–1000）；"When both offset and
  limit are omitted, the full list is returned"。`output_modalities` 默认为 `text`
  （"Defaults to text"），不传时只返回文本输出模型，符合编码 Agent 用途。
- 响应必填 `data`、`total_count`（integer）与 `links`；`links.next` 为 "URL for the next
  page of results, or null if this is the last page"。模型必填字段包括 `id`、`name`、
  `context_length`（integer 或 null）、`architecture`、`top_provider`、`supported_parameters`
  等；文档列出的错误状态码为 `400`、`403`、`500`。
- 认证：OpenAPI 的全局 `security` 为 Bearer 形式的 `apiKey`。`/models` 没有自己的
  `security` 覆盖（不需要认证的接口会写 `security: []`，例如 `/credits/coinbase`），
  因此按文档需要密钥；示例请求也带 `Authorization: Bearer <token>`。不带密钥时的实际
  行为未验证。
- vibemate 每页请求 `limit=500`，自己累加 `offset`，不跟随服务器返回的 `links.next`
  URL，只用它是否为 `null` 判断结束；最多 10 页、5000 个模型，超过即标为不完整，
  不标记下架。

**共用请求边界**：

- 只用 HTTPS，不跟随重定向；请求头为 `Authorization: Bearer <key>` 与
  `Accept: application/json`，密钥不进入 URL。
- **三家的 `/models` 请求都带密钥**（P12 设计决定，2026-10-10）。DeepSeek 与 OpenRouter 的
  文档要求认证；Command Code 不要求，但发给的是签发该密钥的同一服务商、同一主机，和
  之后的推理请求一样，没有额外暴露，还能在服务端按账户返回列表时直接受益。统一带上也
  让脱敏规则和测试只有一条路径。代价是列表请求成功不能说明密钥有效（见本节开头）。
- 只按状态码归类错误，不读取、保存或展示错误响应体。
- 使用系统代理：先读 `ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY`/`NO_PROXY`（含小写），
  未设置时读 macOS/Windows 系统设置；Linux 只读环境变量。
- 回环与私有网络地址继续允许：地址由用户配置、请求由用户触发，且只用 HTTPS。

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
| Command Code → Pi          | 按模型路由选协议；不能将 Claude 映射为 Chat；完整映射待 P02/P24 |
| 三家 Provider → Grok Build | Agent 的准确产品和配置接口由 P03 确认，目前不声称兼容           |

后续连接检查需要区分网络可达、密钥有效、套餐可用和模型请求成功。
模型列表返回成功不能替代上述全部状态（P12 的「获取模型」只说明列表请求成功，
由用户触发的最小推理检查在 P24/P38）。参数校验同时考虑 Provider、模型与 Agent，
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

Command Code 选择目录中声明支持 Chat Completions 的模型（P01 依据 GOAT 套餐资料）：

```json
{
    "model": "<COMMAND_CODE_CHAT_MODEL_ID_FROM_CATALOG>",
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
