# Claude Code 配置接入约定

核实日期：2026-10-13。对应 P18 首个配置适配目标；本机验证平台为 macOS Apple Silicon。

## 发行身份与证据

本机 `claude --version` 返回 `2.1.287 (Claude Code)`。命令由
`/opt/homebrew/bin/claude` 链接到 Homebrew Cask 安装目录
`/opt/homebrew/Caskroom/claude-code/2.1.287/claude`，不是 npm 包布局。
不能按 npm 包名假定安装路径；探测时以 `which claude` 与 `--version` 实际结果为准。

证据来自官方文档（核实日期同上）：

- [Setup](https://code.claude.com/docs/en/setup)
- [Settings files and precedence](https://code.claude.com/docs/en/settings)
- [Model configuration](https://code.claude.com/docs/en/model-config)
- [MCP](https://code.claude.com/docs/en/mcp)
- [Skills](https://code.claude.com/docs/en/skills)

官方 latest 页面可能变化；适配器要记录实际发行版本，不能把本机验证扩展为全部版本。
文档明确要求部分模型与特性绑定最低版本（如 Sonnet 5.5 需 v2.1.284+、Opus 5.5 需
v2.1.280+、Haiku 5.5 需 v2.1.293+），映射能力前先核对安装版本。

## 设置文件与优先级

Claude Code 从 JSON 设置文件读取配置。官方文档定义五个层级，数字小者优先级高：

| 顺序 | 范围     | 文件                                           | 生效人群     |
| ---- | -------- | ---------------------------------------------- | ------------ |
| 1    | 托管     | `managed-settings.json`、MDM、claude.ai 控制台 | 组织内所有人 |
| 2    | 命令行   | `claude --settings <file-or-json>`             | 本次会话     |
| 3    | 项目本地 | `.claude/settings.local.json`                  | 本项目本人   |
| 4    | 项目共享 | `.claude/settings.json`                        | 本项目所有人 |
| 5    | 用户     | `~/.claude/settings.json`                      | 本机所有项目 |

同名 key 取优先级最高层的值。列表类 key（如 `permissions.allow`）跨层合并而不是
覆盖，`fallbackModel`、`modelPicker`、`availableModels`、`modelSettings` 有独立合并规则。

环境变量不是层级之一，按 key 配对逐个判断：`ANTHROPIC_MODEL` 覆盖任何文件的
`model` key；`ANTHROPIC_DEFAULT_MODEL` 只在没有任何文件设置 `model` 时生效。
设置文件内的 `env` 块是普通 key，按层级覆盖。

vibemate 写入用户层 `~/.claude/settings.json` 时必须展示项目/托管层的潜在覆盖来源；
用户层是最低优先级，写入成功不等于最终生效值。

## 模型配置

模型由 `model` 设置项控制，值为模型别名或完整模型名：

| 别名                        | 行为                                 |
| --------------------------- | ------------------------------------ |
| `default`                   | 清除覆盖，回到账号运行时默认         |
| `opus` / `sonnet` / `haiku` | 各系列最新版本（按 provider 解析）   |
| `fable` / `best`            | 最强模型（best 依可用性回退到 opus） |
| `sonnet[1m]` / `opus[1m]`   | 1M 上下文窗口变体                    |
| `opusplan`                  | 计划模式用 opus，执行切 sonnet       |

`/model` 交互选择会把 `model` 写入 `~/.claude/settings.json`。会话级覆盖按
`--model` 标志 > `ANTHROPIC_MODEL` > 设置文件 `model` > `ANTHROPIC_DEFAULT_MODEL` >
组织默认 > 运行时默认的顺序生效；组织默认可通过管理员设置覆盖用户选择。

第三方/网关部署相关环境变量：

| 变量                                  | 作用                                   |
| ------------------------------------- | -------------------------------------- |
| `ANTHROPIC_BASE_URL`                  | 改变请求发送地址，不改变模型           |
| `ANTHROPIC_MODEL`                     | 会话启动模型（覆盖所有文件的 `model`） |
| `ANTHROPIC_DEFAULT_MODEL`             | 无文件设置 `model` 时的默认模型        |
| `ANTHROPIC_DEFAULT_OPUS_MODEL` 等系列 | 别名映射到的完整模型名                 |
| `ANTHROPIC_CUSTOM_MODEL_OPTION` 等    | 向 `/model` 选择器添加自定义条目       |
| `ANTHROPIC_API_KEY`                   | API key（经用户确认后可跳过登录）      |

其他已核实的设置 key：`availableModels`/`deniedModels`/`enforceAvailableModels`、
`modelOverrides`、`fallbackModel`、`modelPicker`、`modelSettings`、
`autoCompactWindow`、`alwaysThinkingEnabled`、`env`（环境变量块）。

本机 `~/.claude/settings.json` 观测到的 key（仅结构，未读取敏感值）：
`editorMode`、`enabledPlugins.*`、`hooks.*`、`permissions.defaultMode`、
`statusLine.*`、`theme`。该实例未设置 `model`，说明默认模型由账号或 CLI 选择提供。

Provider 侧：Anthropic API、Claude Platform on AWS、Amazon Bedrock、Google Cloud
Agent Platform、Microsoft Foundry、Mantle 有各自的模型 ID 解析规则。vibemate 的三家 Provider
（Command Code、DeepSeek、OpenRouter）经 `ANTHROPIC_BASE_URL` 接入的方式见下文
「第三方 Provider 接入」：DeepSeek 与 OpenRouter 有官方接入步骤，Command Code 未核实。
厂商给出的步骤不代替 vibemate 的真实请求验证，也不能凭设置项存在就断言协议兼容。

## MCP 配置

MCP 服务器按三个作用域存储：

| 作用域        | 加载范围 | 共享             | 位置                                             |
| ------------- | -------- | ---------------- | ------------------------------------------------ |
| Local（默认） | 当前项目 | 否               | `~/.claude.json` 的 `projects.<路径>.mcpServers` |
| Project       | 当前项目 | 是（进版本控制） | 项目根 `.mcp.json`                               |
| User          | 所有项目 | 否               | `~/.claude.json` 顶层 `mcpServers`               |

另有托管 `managed-mcp.json`。`.mcp.json` 支持环境变量展开。项目级服务器在交互
会话中需要信任确认。

本机观测：`~/.claude.json` 顶层 `mcpServers` 存有用户级服务器，`projects` 为按
项目路径索引的配置。`~/.claude.json` 不属于 settings 层级，自动发现不读取它
（见 [agent-catalog.md](../plans/agent-catalog.md)）；配置适配写入 MCP 时按
作用域选择正确位置，并保留无关条目。

## Skill 配置

| 位置                                      | 生效范围               |
| ----------------------------------------- | ---------------------- |
| `~/.claude/skills/<name>/SKILL.md`        | 本机所有项目（个人级） |
| `.claude/skills/<name>/SKILL.md`          | 本仓库会话（项目级）   |
| `<subdir>/.claude/skills/<name>/SKILL.md` | 子目录会话（嵌套）     |
| 托管设置目录                              | 组织内所有用户         |
| `<plugin>/skills/<name>/SKILL.md`         | 插件启用处             |

同名 skill 优先级：企业级 > 个人级 > 项目级。`<name>` 条目可以是符号链接，
Claude Code 从目标读取 `SKILL.md` 并去重加载。`~/.claude/skills/synced/` 是
claude.ai 同步技能的保留目录，不得用于用户技能。

本机观测：`~/.claude/skills/` 存有多个个人技能目录。技能部署（P27）可优先使用
符号链接，与官方文档的 symlink 支持一致。

## 认证与凭据

首次运行 `claude` 通过浏览器 OAuth 登录；设置 `ANTHROPIC_API_KEY` 并经用户确认
后可跳过登录。另有 `apiKeyHelper` 脚本方式。macOS Keychain 存在
"Claude Safe Storage" / "Claude Key" 条目，用于加密保存凭据。

vibemate 不读取、不展示 `~/.claude.json` 中的凭据内容；配置写入使用凭据引用，
真实密钥走 OS 凭据存储（见 [architecture.md](../architecture.md)）。

## 第三方 Provider 接入（2026-10-13 复核）

Claude Code 官方把第三方地址称为 LLM gateway，接入约定见
[Connect to an LLM gateway](https://code.claude.com/docs/en/llm-gateway-connect) 与
[Other LLM gateways](https://code.claude.com/docs/en/llm-gateway)。

| 变量                   | 发送方式                | 何时使用                  |
| ---------------------- | ----------------------- | ------------------------- |
| `ANTHROPIC_BASE_URL`   | 请求基地址              | 指向网关时总是需要        |
| `ANTHROPIC_AUTH_TOKEN` | `Authorization: Bearer` | 网关方说明是 bearer token |
| `ANTHROPIC_API_KEY`    | `x-api-key`             | 网关方说明是 API key      |
| `apiKeyHelper`         | 两者                    | 凭据轮换或来自 vault      |

官方建议在未说明凭据类型时先用 `ANTHROPIC_AUTH_TOKEN`，返回 `401` 表示变量选错，改用另一个。
变量可以放进 `~/.claude/settings.json` 的 `env` 块持久生效；官方明确禁止把凭据放进会被提交和
共享的项目 `.claude/settings.json`。同一个变量同时由 shell 导出和设置文件 `env` 提供时，
**设置文件的值为准**。

网关凭据变量优先于已保存的 claude.ai 登录：`ANTHROPIC_AUTH_TOKEN` 立即生效，
`ANTHROPIC_API_KEY` 在交互模式下先征求一次确认。`/status` 显示生效的 base URL 与凭据来源。
Anthropic 官方声明不支持通过任何网关把 Claude Code 路由到非 Claude 模型。

设置文件被监听且多数编辑会热重载，但部分键只在会话开始时读取；`model` 属于后者，需要新会话或
用 `/model` 切换。`CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1` 可让 `/model` 列出网关返回的
模型名，本机未验证该响应格式。

### 三家 Provider 的现状

| Provider     | 依据                                                                                                        | Claude Code 网关地址                                               | 凭据变量                                                  | 状态                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------- | -------------------------------------------- |
| DeepSeek     | 官方 [Integrate with Claude Code](https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code) | `https://api.deepseek.com/anthropic`                               | `ANTHROPIC_AUTH_TOKEN`                                    | 有官方逐步配方                               |
| OpenRouter   | 官方 [Claude Code Integration](https://openrouter.ai/docs/cookbook/coding-agents/claude-code-integration)   | `https://openrouter.ai/api`（与其 API 目录使用的 `/api/v1` 不同）  | `ANTHROPIC_AUTH_TOKEN`，并把 `ANTHROPIC_API_KEY` 显式置空 | 有官方配方，并附兼容限制                     |
| Command Code | 无 Claude Code 专用说明；FAQ 声称任何 Anthropic 兼容客户端可用                                              | 文档给客户端的是 `.../provider/v1`，Claude Code 需要的网关根未核实 | `Authorization: Bearer` 适用所有路由                      | DeepSeek 模型实测不可用（维护者 2026-10-13） |

DeepSeek 的配方除基地址与 `ANTHROPIC_AUTH_TOKEN` 外还设置 `ANTHROPIC_MODEL=deepseek-flash[1m]`、
`ANTHROPIC_DEFAULT_OPUS/SONNET/HAIKU_MODEL`、`CLAUDE_CODE_SUBAGENT_MODEL`、
`CLAUDE_CODE_EFFORT_LEVEL=max` 与 `CLAUDE_CODE_AUTO_COMPACT_WINDOW`。`[1m]` 变体和这些伴随变量
是否适用于其他模型未核实；首版只映射基地址、凭据与模型名。

### DeepSeek 官方推荐配置与维护者实测（2026-10-13）

DeepSeek 的 Integrate with Claude Code 页推荐的变量（原文见该页 "Migrate from Existing
Installation to DeepSeek"）共九项：`ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN`、
`ANTHROPIC_MODEL=deepseek-flash[1m]`、`ANTHROPIC_DEFAULT_OPUS_MODEL=deepseek-flash[1m]`、
`ANTHROPIC_DEFAULT_SONNET_MODEL=deepseek-flash[1m]`、`ANTHROPIC_DEFAULT_HAIKU_MODEL=deepseek-flash`、
`CLAUDE_CODE_SUBAGENT_MODEL=deepseek-flash`、`CLAUDE_CODE_EFFORT_LEVEL=max`、
`CLAUDE_CODE_AUTO_COMPACT_WINDOW=786432`。注意厂商对类别模型是分开写的：Opus/Sonnet 类用 1M 变体，
Haiku 与子代理用普通 `deepseek-flash`。

维护者本机 `~/.claude/settings.json` 的 `env` 块在官方九项之外还设置了四项（凭据已隐去），整套可用：

```json
{
    "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
    "ANTHROPIC_AUTH_TOKEN": "<DeepSeek API Key>",
    "ANTHROPIC_MODEL": "deepseek-flash[1m]",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "deepseek-flash[1m]",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-flash[1m]",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "deepseek-flash",
    "CLAUDE_CODE_SUBAGENT_MODEL": "deepseek-flash",
    "CLAUDE_CODE_EFFORT_LEVEL": "max",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW": "786432",
    "CLAUDE_CODE_MAX_CONTEXT_TOKENS": "1048576",
    "API_TIMEOUT_MS": "600000",
    "BASH_DEFAULT_TIMEOUT_MS": "300000",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"
}
```

官方页没有列出这四项，但它们在 Claude Code 文档里存在：`CLAUDE_CODE_MAX_CONTEXT_TOKENS` 用来
按网关 ID 校正假定窗口，`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` 在设置参考的遥测一节。
`API_TIMEOUT_MS` 与 `BASH_DEFAULT_TIMEOUT_MS` 我在设置与设置参考两页都没找到，来源未核实。

这是目前唯一的真实可用样本，也是唯一一条真实链路记录。它能证明这条链路可用，但不能说明哪些键必需、
哪些只是调优：官方九项是厂商推荐，多出的四项是维护者补充。vibemate 自身仍未发出过网关请求。

各键的类别与归属见 [p18-claude-code-link.md](../plans/p18-claude-code-link.md) 的「使用设置的归属」一节。

OpenRouter 的页面补充两点：Claude Code 只保证与 Anthropic 一等厂商的模型配合使用；曾用 Anthropic
账号登录过时必须先 `/logout`，否则缓存的登录与网关凭据冲突，表现为 `openrouter/auto` 之类的
模型找不到错误（macOS 上该缓存存放在钥匙串的 `Claude Code-credentials` 条目）。

Command Code 的 Provider API 文档确认 `POST /provider/v1/messages` 使用 Anthropic schema、
`Authorization: Bearer` 适用于任何路由、`x-api-key` 适用于 `/messages`。它没有 Claude Code 专用
步骤，但 API support 一节写着「Point any OpenAI or Anthropic compatible client at
`https://api.commandcode.ai/provider/v1`」，FAQ「Will it work with my agent?」补充三种端点
「any client or agent can use them. No lock-in.」。这两句是产品级声明，不等于 Claude Code 被验证支持。

同一份错误表写明 `400` 覆盖「非 Anthropic 模型走 `/messages`」这类端点错配，也就是 `/messages`
只服务 Claude 模型。维护者 2026-10-13 的实测与此一致：在 Claude Code 中选择 Command Code 上的
**DeepSeek 模型**无法接入。另一条尚未验证的候选原因：Claude Code 把请求发到
`<ANTHROPIC_BASE_URL>/v1/messages`，而 DeepSeek 与 OpenRouter 的官方地址都是网关根形式
（`.../anthropic`、`.../api`）；若同样成立，Command Code 的网关根应为
`https://api.commandcode.ai/provider`，而文档给客户端的基础地址是 `.../provider/v1`。两条原因都没有
真实请求记录，因此这个组合继续按未核实处理，vibemate 拒绝为它生成配置计划。

## 未验证边界

- vibemate 尚未发出一次真实网关请求：DeepSeek 与 OpenRouter 的模型名映射、参数透传与错误行为都还
  没有实测记录。
- Command Code：Claude 模型走 `/messages` 是否可用，以及 Claude Code 需要的网关根是
  `https://api.commandcode.ai/provider` 还是文档给客户端的 `.../provider/v1`。
- 非 Anthropic 模型经网关配合 Claude Code 的行为：Command Code 在文档层面禁止（`/messages` 只服务
  Claude 模型），OpenRouter 只保证 Anthropic 一等厂商的模型。vibemate 目前在计划上只给一条提示，
  没有按模型的厂商或路由做区分。
- Windows/Linux 平台的安装布局、设置路径与凭据存储。
- 托管设置（managed-settings.json）的完整 key 集合与 MDM 交付方式。
- 设置文件全部 key 的默认值与校验规则（官方 [All settings](https://code.claude.com/docs/en/settings/all-settings)
  页未在本次核实中完整提取）。
- 项目级 `.mcp.json` 信任确认流程与 `disabledMcpjsonServers` 的实际行为。

这些缺口在对应实施任务中补验，不按产品名或单个设置项推断整体能力。
