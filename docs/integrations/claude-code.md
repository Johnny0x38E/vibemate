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
Agent Platform、Microsoft Foundry、Mantle 有各自的模型 ID 解析规则。vibemate 的
三家 Provider（Command Code、DeepSeek、OpenRouter）经 `ANTHROPIC_BASE_URL` +
`model` 映射的具体请求格式尚未验证，不能凭设置项存在就断言协议兼容。

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

## 未验证边界

- 三家 vibemate Provider 经 `ANTHROPIC_BASE_URL` 发送的实际请求协议与字段映射。
- Windows/Linux 平台的安装布局、设置路径与凭据存储。
- 托管设置（managed-settings.json）的完整 key 集合与 MDM 交付方式。
- 设置文件全部 key 的默认值与校验规则（官方 [All settings](https://code.claude.com/docs/en/settings/all-settings)
  页未在本次核实中完整提取）。
- 项目级 `.mcp.json` 信任确认流程与 `disabledMcpjsonServers` 的实际行为。

这些缺口在对应实施任务中补验，不按产品名或单个设置项推断整体能力。
