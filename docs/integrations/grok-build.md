# Grok Build 配置接入约定

核实日期：2026-10-09。对应 P03；本机验证平台为 macOS Apple Silicon。

## 产品与安装身份

[官方介绍](https://docs.x.ai/build/overview)与 [xai-org/grok-build](https://github.com/xai-org/grok-build)
确认 Grok Build 是 xAI 的编码 Agent，支持 TUI、headless 与 ACP。本机命令是
`grok`，`grok --help` 标题为 Grok Build TUI；`grok --version` 返回
`1.0.50 (c58f321264ba)`。本机入口链接到 `~/.grok/downloads/grok-1.0.50-macos-aarch64`。
官方源码许可证为 Apache-2.0；本次仅记录接入规则，没有将源码加入 MIT 应用。

官方安装页面提供 macOS/Linux/WSL 和 Windows 安装入口，本机只验证 macOS。
公开 GitHub 树查询未找到本机短提交 `c58f321264ba`；main 源码只能作为当前机制的
补充证据，不能声称与该二进制完全一致。下面注明哪些行为已由本机隔离检查确认。

## 文件与配置优先级

[Settings](https://docs.x.ai/build/settings)使用 `~/.grok/config.toml`，Windows
默认 `%USERPROFILE%\.grok\config.toml`。`GROK_HOME` 覆盖配置、认证、资源、会话
和日志的根目录。探测时使用选定目标的实际目录与环境，不固化用户路径。

[企业部署说明](https://docs.x.ai/build/enterprise)列出由低到高的五层：
系统 managed → 用户 managed → 用户 config → 用户 requirements → 系统 requirements。
requirements 可以固定设置并限制低层、环境变量及远端设置覆盖。公开 main 还存在
平台管理策略处理；具体机器的生效来源必须检查，不能只修改用户文件就认定成功。

项目 `.grok/config.toml` 只贡献 MCP、plugins 和 permission，不承载完整用户设置
或自定义模型。[MCP 文档](https://docs.x.ai/build/features/mcp-servers)说明项目目录
向仓库根目录遍历，同名项目 Server 完整替换用户条目。不能照搬 Pi 的只覆盖
`enabled` 合并语义。项目 trust 与管理策略可能进一步限制资源加载。

Grok 还发现 Claude/Cursor 兼容资源；原生 config.toml 优先于兼容 MCP 定义。
通过来源信息区分自己管理的条目、兼容来源、项目覆盖与策略固定项，不替用户关闭
整个兼容层。`inspect --json` 是发现报告，不是连接成功证明。

## 模型与认证

[配置参考](https://docs.x.ai/build/settings/reference)声明自定义 `[model.<alias>]`：

| 作用      | 字段                                                                   |
| --------- | ---------------------------------------------------------------------- |
| 标识      | alias 是 Grok 内部配置身份；`model` 是发送给 Provider 的模型 ID        |
| 协议/地址 | `base_url`、`api_backend`：`chat_completions`、`responses`、`messages` |
| 认证      | `env_key` 指向环境变量；`api_key` 是目标私有文件中的明文值             |
| 请求参数  | `temperature/top_p/max_completion_tokens`、推理支持与努力字段          |
| 元数据    | `context_window`；不能当作请求参数透传                                 |
| 路由扩展  | `extra_headers`、超时、重试、工具调用流式设置；逐项核实支持范围        |

`[models].default` 选择新会话默认模型，环境选择与 CLI `-m` 可改变实际选择，
恢复的会话和运行中的 TUI 也不能仅凭默认值判断。官方支持 `/model` 切换；
外部配置修改后的重读/重启与当前会话有效状态要单独验证。

认证顺序按官方 enterprise 文档为模型 `api_key` → 模型 `env_key` → 活动会话
Token → `XAI_API_KEY`。支持浏览器/设备认证与外部认证命令，但 vibemate 不新增
任意脚本执行 API，不执行导入的认证命令。现有登录不等于某个 BYOK 端点已获授权。

环境引用依赖独立启动 Grok 的终端环境；桌面设置不能自动传播到已有终端。
中央 OS 凭据引用也不能直接充当 `env_key` 值。若目标需要私有明文配置，应明确
预览、权限、脱敏与私有备份策略。认证文件（如 `auth.json`、`mcp_credentials.json`）
未在本次读取；未验证本机权限或 Windows ACL，不能承诺固定权限。

## Skill 与 MCP 启停

[Skill 文档](https://docs.x.ai/build/features/skills-plugins-marketplaces)支持含
`SKILL.md` 的目录：用户 `.grok/skills/`、沿项目目录发现的 `.grok/skills/`、
启用插件与 `[skills].paths`，还有 `.agents`/Claude/Cursor 兼容来源。
目录链接在本机隔离检查中可发现，其他平台链接与复制同步保持待验。

`[skills].disabled` 按名称发现但不激活资源；本机 `inspect` 确认停用的 Skill
仍被报告并带 `disabled: true`。管理名称冲突与来源，不能误停同名用户资源。
不改共享 Skill 的 frontmatter 来关闭单个 Agent，不在扫描时运行其脚本。

MCP 原生使用 `[mcp_servers.<name>]`，stdio 为 `command/args/env/cwd`，远端为
`url/headers/bearer_token_env_var`，支持 `enabled`、启动和工具超时。
`${VAR}` 与默认值引用在加载时扩展；不存在的环境值和敏感输出应安全处理。
官方 MCP 页面重点说明 HTTP，字段参考还提到 SSE；第一阶段先按已确认 stdio/HTTP
实现，不凭这一表述承诺旧 SSE 的运行兼容性。

本机有 `grok mcp enable/disable`。隔离操作已确认启停持久化且保留 command args
和无关 UI 设置。TUI `/mcps` 提供开关及 `r` 刷新；当前会话连接与工具撤销效果
没有验证。HTTP OAuth 由 Agent 处理，不能将其 Token 放进中央普通导出。

**本机版本差异：** `1.0.50` 的 `inspect` 列出启用的原生 MCP，却省略关闭的原生
MCP；兼容来源还可能以 disabled 状态列出。因此适配器必须结合配置中的 enabled
与发现报告，不能把缺席当作删除、解析失败或不存在，也不能认为报告格式与 main
源码永远相同。

## 适配方向与未实现范围

| 关系                  | 可用原生方向                                                     | 实现前还需验证                                                                     |
| --------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Provider/Model → Grok | 用户级 model alias、协议与默认模型；disabled_models 移除目录模型 | 模型身份/默认恢复、实际请求字段与生效会话；hidden_models 仅隐藏，仍可通过 CLI 使用 |
| Skill → Grok          | skills.disabled 或撤销自己的部署                                 | 名称/来源冲突、项目兼容覆盖、运行时刷新                                            |
| MCP → Grok            | per-server enabled，原生 CLI/TUI 开关                            | 原生/项目/兼容来源合并、策略限制、连接及关闭效果                                   |

没有发现第一阶段必须加入本地代理或启动器的证据；三个原生协议入口能作为适配
起点。每个 Provider/模型的工具、推理、图像与输出参数仍需 P24 请求级验证。
不据此声称六个 Provider/Agent 组合已经接通。

## 脱敏示例

下面是格式与发现检查用的用户级 TOML，不是已生效业务配置：

```toml
[model.vibemate-deepseek]
model = "deepseek-flash"
base_url = "https://api.deepseek.com"
name = "DeepSeek through vibemate"
env_key = "VIBEMATE_DEEPSEEK_API_KEY"
api_backend = "chat_completions"

[models]
disabled_models = ["vibemate-deepseek"]

[skills]
disabled = ["vibemate-smoke"]

[mcp_servers.vibemate_smoke]
command = "vibemate-must-not-execute"
args = ["--preserve"]
enabled = false
```

不存在的 MCP 命令仅用于证明配置发现不要求执行服务器；不是安装建议。
密钥值不在示例中。输出字段和推理参数后续需要显式映射到 Provider 协议。

## 本次验证

- 官网、发行仓库、`grok --version`、主帮助与 inspect/MCP 子命令帮助核对完成。
- 临时 `GROK_HOME` 与临时 cwd 的 inspect 成功；关闭自动更新与 Claude/Cursor 兼容功能。兼容定义仍可能被发现并标记 disabled，不能将关闭扫描视为完全不读取元数据。
- 自定义模型格式未出现解析警告，但 inspect 不验证模型请求或保证列出所有模型字段。
- symlink Skill 被发现并标记 disabled；原生 MCP enabled=true 被列出，false 被省略。
- 临时原生 disable/enable 保留 args 与无关 UI 设置；临时文件全部清理。
- TOML 示例解析、`pnpm run format:check`；仅文档改动不重跑原生构建。
- 未运行登录、推理、MCP doctor 或 Agent 会话，未输出真实凭据，未修改用户原有配置。
- 运行中刷新、协议实际调用、Windows/Linux、文件权限与政策覆盖留待后续验证。
