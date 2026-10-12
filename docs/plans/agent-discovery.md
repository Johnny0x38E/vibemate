# Agent 只读发现（P17）

当前已实现 Pi 与 Grok Build 的本机入口发现和手动路径检查，不安装、不启动、不连接、不写 Agent 配置。用户输入仅保存在当前窗口，不写 SQLite。

维护者已确认下一批改为六种固定目录并实现全部检测：Claude Code、Codex、OpenCode、Pi、Grok Build、Hermes。范围与分批规则见 [agent-catalog.md](agent-catalog.md)，任务状态见 [todo.md](todo.md)。本说明在代码完成前不将新增四家记为已支持。

2026-10-11 重新核对官方 [Pi configuration](https://pi.dev/docs/latest/configuration) 与 [Grok settings](https://docs.x.ai/build/settings)：Pi 默认 `~/.pi/agent`，桌面进程的 `PI_CODING_AGENT_DIR` 可覆盖；Grok 默认 `~/.grok`，`GROK_HOME` 可覆盖。桌面环境不一定等同终端，SDK/CLI 运行时覆盖和项目 trust 无法通过静态发现确认。

自动发现只查进程 PATH 前 64 项中的绝对目录及已核实用户入口（`~/.local/bin/pi`、`<pi-agent-dir>/bin/pi`、`~/.local/bin/grok`）；不遍历全盘或当前项目。手动输入绝对入口路径，检查 regular file/symlink 目标，不执行它，路径存在不等同于安装真实性或可启动。

Pi 官方当前 [package metadata](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/package.json) 的包名、版本和 `bin.pi` 也已核对。Pi 只从入口的解析路径附近读取有界 `package.json`，匹配 `@earendil-works/pi-coding-agent` 和 `bin.pi` 目标后才展示 metadata 版本。托管 launcher 不解析或执行其内容，也不从 releases 目录任意挑选一个版本冒充当前安装。Grok 仅从已核实的版本化下载文件名提取版本线索，始终标为未核实；不通过 `--version` 或 `inspect` 启动二进制。只有 Pi metadata 版本 `1.1.0` 可关联 P02 的已记录文档能力，仍不等于真实安装或运行连接验证。

配置仅检查路径类型/存在状态，不读取 settings/models/auth/MCP/TOML 内容，不返回凭据、配置片段或原始 I/O 错误。显示来源与文档优先级，明确项目/策略/CLI/现有会话可能覆盖用户设置，未判定有效配置。能力区分文档记载/版本未核实，不将未来部署功能表示为可用。

不新增依赖、任意脚本 API 或 Tauri 权限；Rust 在 blocking pool 内探测，React 只处理输入与展示。响应在 TS 边界严格验证；browser preview 不扫描。未知版本和不受支持版本不混为一谈：当前没有足够证据宣称其他版本不支持，因此一律未核实。

## 验证边界

已有 Rust/IPC/UI 测试覆盖有界 metadata、入口关联、未知版本、坏 symlink、手动路径、环境覆盖、脱敏和过期请求；隔离 Linux 原生检查确认未执行入口或读取/改写配置。历史测试计数和构建过程从 Git/CI 与 CHANGELOG 查阅。

真实 Pi/Grok 安装、macOS/Windows 和用户视觉验收仍待执行；已有样本不替代这些结果。托管 launcher/Windows shim 无法安全关联 metadata 时显示版本未知，不能据此推断未安装或不支持。
