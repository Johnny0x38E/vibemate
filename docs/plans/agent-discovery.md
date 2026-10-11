# Agent 只读发现（P17）

范围是 Pi 与 Grok Build 的本机入口发现和手动路径检查，不安装、不启动、不连接、不写 Agent 配置。用户输入仅保存在当前窗口，不写 SQLite。

2026-10-11 重新核对官方 [Pi configuration](https://pi.dev/docs/latest/configuration) 与 [Grok settings](https://docs.x.ai/build/settings)：Pi 默认 `~/.pi/agent`，桌面进程的 `PI_CODING_AGENT_DIR` 可覆盖；Grok 默认 `~/.grok`，`GROK_HOME` 可覆盖。桌面环境不一定等同终端，SDK/CLI 运行时覆盖和项目 trust 无法通过静态发现确认。

自动发现只查进程 PATH 的绝对目录及已核实用户入口（`~/.local/bin/pi`、`<pi-agent-dir>/bin/pi`、`~/.local/bin/grok`）；不遍历全盘或当前项目。手动输入绝对入口路径，检查 regular file/symlink 目标，不执行它，路径存在不等同于安装真实性或可启动。

Pi 只从入口的解析路径附近读取有界 `package.json`，匹配 `@earendil-works/pi-coding-agent` 和 `bin.pi` 目标后才展示 metadata 版本。托管 launcher 不解析或执行其内容，也不从 releases 目录任意挑选一个版本冒充当前安装。Grok 仅从已核实的版本化下载文件名提取版本线索，始终标为未核实；不通过 `--version` 或 `inspect` 启动二进制。只有 Pi metadata 版本 `1.1.0` 可关联 P02 的已记录文档能力，仍不等于真实安装或运行连接验证。

配置仅检查路径类型/存在状态，不读取 settings/models/auth/MCP/TOML 内容，不返回凭据、配置片段或原始 I/O 错误。显示来源与文档优先级，明确项目/策略/CLI/现有会话可能覆盖用户设置，未判定有效配置。能力区分文档记载/版本未核实，不将未来部署功能表示为可用。

不新增依赖、任意脚本 API 或 Tauri 权限；Rust 在 blocking pool 内探测，React 只处理输入与展示。响应在 TS 边界严格验证；browser preview 不扫描。未知版本和不受支持版本不混为一谈：当前没有足够证据宣称其他版本不支持，因此一律未核实。
