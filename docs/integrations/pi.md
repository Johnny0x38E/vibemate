# Pi 配置接入约定

核实日期：2026-10-09。对应 P02；本机验证平台为 macOS Apple Silicon。

## 发行身份与证据

本机 `pi --version` 返回 `1.1.0`。命令由 `~/.local/bin/pi` 链接到托管安装的
`~/.pi/agent/bin/pi`，再转发到 `install/releases/1.1.0/node_modules/.bin/pi`。
安装包 metadata 确认包名为 `@earendil-works/pi-coding-agent`、版本 `1.1.0`，
官方仓库为 [earendil-works/pi](https://github.com/earendil-works/pi)，许可证 MIT。
不能继续按旧的 `@mariozechner` 包名假定安装布局。

证据包含官网与该安装包的 `docs/`、`dist/core/model-config.js`、
`dist/core/auth-storage.js`、`dist/core/skills.js`、`dist/extensions/mcp/config.js`。
官网 latest 页面可能变化；适配器要记录实际发行版本，不能把本机验证扩展为全部版本。

## 配置位置与优先级

[官方配置说明](https://pi.dev/docs/latest/configuration)使用默认 agent 目录
`~/.pi/agent`，支持 `PI_CODING_AGENT_DIR` 和 SDK `agentDir` 覆盖。
探测时以运行环境与选定目录为准，不把默认路径当作唯一位置。

| 文件或资源     | 用户级                                     | 项目级                               | 优先级/生效                                                    |
| -------------- | ------------------------------------------ | ------------------------------------ | -------------------------------------------------------------- |
| 设置           | `<agent-dir>/settings.json`                | `<cwd>/.pi/settings.json`            | 受信任项目覆盖普通用户设置，资源数组组合；部分设置只允许用户级 |
| Provider/Model | `<agent-dir>/models.json`                  | 当前官方目录表未声明项目 models.json | 不自行创建项目模型文件；`/model` 重新加载目录                  |
| 凭据           | `<agent-dir>/auth.json`                    | 无对应项目凭据文件                   | 使用原生认证顺序；不能向项目文件写真实密钥                     |
| Skill          | `<agent-dir>/skills/`、`~/.agents/skills/` | `.pi/skills/`、`.agents/skills/`     | 项目资源受 trust 约束；同名 Skill 首个发现者保留并报告冲突     |
| MCP            | `<agent-dir>/mcp.json`                     | `<cwd>/.pi/mcp.json`                 | 受信任项目同名条目覆盖用户级；也可只覆盖 enabled/exposure      |

项目 `.agents/skills/` 会沿父目录发现，遇仓库根目录停止；自定义资源路径另有
settings 规则。[Settings](https://pi.dev/docs/latest/settings)说明用户资源相对
agent 目录解析，项目资源相对 `.pi` 目录解析，支持绝对路径和 `~`。
CLI 选择与恢复的会话可能改变当前模型，不能仅凭 `defaultProvider/defaultModel`
认定正在运行的会话已经使用新模型。运行时确认留给后续 Agent 验收。

## Provider、模型与独立启动认证

[模型文档](https://pi.dev/docs/latest/models)支持兼容端点的 `baseUrl`、`api`、
`apiKey`、`headers`、`models` 与 `modelOverrides`。模型字段包括 `id/name`、
`reasoning/input`、`contextWindow/maxTokens`、成本、采样参数与兼容设置。
`maxTokens` 描述模型上限，不能直接等同于每次请求想要的输出长度。

首条 DeepSeek 路径使用 `openai-completions`。安装 schema 明确允许
`compat.maxTokensField = max_tokens`、`compat.thinkingFormat = deepseek`；
OpenRouter 可使用自己的 thinking 格式和 routing 设置。Claude 使用
`anthropic-messages` 路由的映射需按具体模型核对。schema 接受字段不代表已经
验证请求效果；P01 的 Provider 字段还必须与 Pi 实际发送的请求对应。

推理使用 `thinkingLevelMap` 与 Pi 的 `off/minimal/low/medium/high/xhigh/max`
级别；不把努力级别强行转换为 token 数量。采样顺序为模型默认、所选 Pi 推理级别
覆盖、请求级参数，后者优先。兼容开关只针对已确认的差异使用。

认证优先级由官方 models 文档与 [Providers](https://pi.dev/docs/latest/providers)
说明：运行时 `--api-key` → `auth.json` → `models.json` 的 `apiKey` → Provider
环境变量/云端凭据。扩展可定义不同认证。`/logout` 仅移除存储凭据，不能解除其他
来源，因此不能当作通用停用操作。

`apiKey` 可写环境变量引用或文字值，也支持 `!command`。vibemate 仅管理经过验证的
结构化配置，不生成任意凭据执行命令，也不在导入/预览时执行既有命令。
桌面进程设置的环境变量不会自动传播给用户已有的终端；选择环境变量方式时必须
确认用户独立启动 Pi 的实际来源，缺少变量不能显示可用。

Pi 的 `auth.json` 是明文凭据存储，而非 vibemate 的 OS 凭据引用。安装源码创建
凭据文件时使用 `0600`、新建父目录时使用 `0700`；已有文件权限与 ACL 不自动更改。
需要明确区分中央 OS 凭据、目标 Agent 私有凭据及其备份，不声称外部 Agent 能直接
读取 vibemate 引用。真实凭据注入尚未实施。

## Skill 部署与关闭

[Skills](https://pi.dev/docs/latest/skills)采用含 `SKILL.md` 的目录，frontmatter
提供名称和描述，按任务需要加载内容。安装版目录扫描会跟随目录 symlink；本机
临时 Skill 的 symlink 发现已验证，Windows 链接权限与 Linux 行为仍待原生验证。

Settings 的资源数组支持 `!pattern`、`+path`、`-path`。Skill 停用可通过精确排除
或撤销 vibemate 管理的链接/复制实现；必须保留用户资源和中央 Skill 内容。
`enableSkillCommands: false` 只控制命令发现，手动输入仍可使用；
`disable-model-invocation` 只排除自动选择，也不能代表完全关闭。

文件改变后按官方要求 `/reload`。运行中的会话是否重载要单独确认，重新加载前
不能把磁盘上的停用操作当作当前会话已撤销全部既有上下文。

## MCP 原生能力与关闭

[Pi MCP 文档](https://pi.dev/docs/latest/mcp)与安装 `1.1.0` 的实现一致：
原生支持 stdio 和 streamable HTTP，拒绝旧 SSE。stdio 配置是单个 executable 的
`command` 加 `args/env/cwd`，不是可任意执行的 shell 字符串；HTTP 使用
`url/headers`，可选 OAuth。OAuth 凭据保存到私有 `mcp-auth.json`，本次未读取。

每个 Server 支持 `enabled: false`，定义保留但不连接。项目同名完整定义替换全局
定义；仅含 `enabled/exposure/toolExposure` 的项目条目合并这些状态并保留命令、
环境与认证配置。未获项目 trust 时忽略项目配置。

外部修改后需要 `/reload`；Pi `/mcp` 可在会话里切换并保存状态。启动 Pi 或运行
`pi mcp list` 都可能连接 enabled Server，因此本次没有执行它们。
预览只用纯配置解析，不能用会连接服务器的命令当成无副作用验证。

安装了接管 `/mcp` 的第三方扩展时，会话可能不再使用内置 `mcp.json`。此外，
`-builtin:mcp`、`--no-mcp`、`--no-extensions` 等会影响内置能力。
需要检测有效配置/扩展来源；仅看到 `mcp.json` 不能保证 Agent 已加载其中的 Server。

## 开关映射约定

| 首页关系            | Pi 适配方向                                                            | 当前可确认边界                                                                     |
| ------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Provider/Model → Pi | 管理自己的 Provider/模型声明和默认选择；停用时撤销受管注入并恢复原配置 | schema 没有通用 enabled 字段；不能借 enabledModels 的启动/循环筛选声称彻底禁止使用 |
| Skill → Pi          | 精确排除或撤销自己的链接/复制，之后重载                                | 不修改共享 Skill 文档来停用某个 Agent，不清理用户自有资源                          |
| MCP → Pi            | 原生 enabled；识别全局/项目覆盖与内置实现是否被替换                    | 停用定义可保留；当前会话的连接状态仍需确认                                         |

这些是后续实现约定，当前尚未写入任何用户配置。中央停用需要给出受影响目标的
差异、应用结果和逐目标失败状态；不把多个文件操作描述为一个原子事务。

## 脱敏格式示例

以下只用于 schema 核实，不是已生效配置。环境变量引用保持原文，不解析凭据。

```json
{
  "providers": {
    "vibemate-deepseek": {
      "baseUrl": "https://api.deepseek.com",
      "api": "openai-completions",
      "apiKey": "${VIBEMATE_DEEPSEEK_API_KEY}",
      "models": [
        {
          "id": "deepseek-flash",
          "input": ["text", "image"],
          "reasoning": true,
          "compat": {
            "maxTokensField": "max_tokens",
            "thinkingFormat": "deepseek"
          }
        }
      ]
    }
  }
}
```

项目 MCP 停用覆盖示例，要求用户级已经存在同名 Server：

```json
{
  "mcpServers": {
    "vibemate-smoke": { "enabled": false }
  }
}
```

## 实际验证与限制

- `pi --version`、托管 launcher、安装 metadata 与本机附带文档/源码核对完成。
- 临时目录调用 `ModelConfig.load`：接受上述模型配置，未解析环境变量引用。
- 临时 MCP 调用 `loadMcpConfig`：受信任项目 enabled=false 保留全局 command/env；未受信任项目覆盖被忽略。
- 临时 `loadSkillsFromDir`：symlink 指向的 Skill 成功发现。所有临时文件随后清理。
- `pnpm run format:check` 与 JSON 示例解析检查通过。
- 未启动 Agent 会话、MCP Server、Skill 脚本或推理请求；未读写真实凭据/配置。
- Windows/Linux、有效会话重载、实际协议请求、复制同步与 OS 凭据注入留待后续任务验证。
