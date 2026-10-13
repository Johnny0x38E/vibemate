# Agent 只读发现（P17）

固定目录按 Claude Code、Codex、OpenCode（Anomaly）、Pi、Grok Build、Hermes（Nous Research）排序，全部支持本机自动入口发现与手动路径检查。不安装、不启动、不连接、不写 Agent 配置；输入仅保存在当前窗口，不写 SQLite。六种发现不等于六种配置部署支持，后者仍在 Pi/Grok 后续任务中。

## 入口与用户配置观测

自动发现只查桌面进程 PATH 前 64 项中的绝对目录及已核实的固定用户入口，不遍历全盘或当前项目，也不执行 shell、WSL 或容器。手动输入优先，必须是安全的绝对路径；检查普通文件或 symlink 目标，不执行它。PATH 坏入口保留遮蔽结果，不静默选择后续候选。Windows 检查相应 exe/cmd/bat/无扩展入口；不解析或执行 shim。

| Agent       | CLI        | 已知 PATH 外入口                                               | 用户路径/覆盖                                                                      | 只检查存在的文件                                          |
| ----------- | ---------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Claude Code | `claude`   | POSIX `~/.local/bin/claude`；Windows 仅 PATH/手动              | `~/.claude` / `CLAUDE_CONFIG_DIR`                                                  | `settings.json`                                           |
| Codex       | `codex`    | 仅 PATH/手动，不猜 npm/Homebrew 前缀                           | `~/.codex` / `CODEX_HOME`                                                          | `config.toml`、`auth.json`                                |
| OpenCode    | `opencode` | POSIX `~/.opencode/bin/opencode`；Windows 仅 PATH/手动         | `OPENCODE_CONFIG_DIR`，否则绝对 `XDG_CONFIG_HOME/opencode` 或 `~/.config/opencode` | `opencode.json`、`opencode.jsonc`                         |
| Pi          | `pi`       | `~/.local/bin/pi`、`<agent-dir>/bin/pi`                        | `~/.pi/agent` / `PI_CODING_AGENT_DIR`                                              | `settings.json`、`models.json`、`mcp.json`                |
| Grok Build  | `grok`     | `~/.local/bin/grok`                                            | `~/.grok` / `GROK_HOME`                                                            | `config.toml`、`managed_config.toml`、`requirements.toml` |
| Hermes      | `hermes`   | POSIX `~/.local/bin/hermes`；Windows `LOCALAPPDATA/hermes/bin` | POSIX `~/.hermes`、Windows `LOCALAPPDATA/hermes` / `HERMES_HOME`                   | `config.yaml`、`.env`、`auth.json`                        |

显式无效目录覆盖不会静默回退；Codex 的空 `CODEX_HOME` 按默认路径处理，非空覆盖要求已有目录。OpenCode 是候选目录观测：官方版本间目录覆盖含义不同，不将 `OPENCODE_CONFIG_DIR` 声称为唯一生效路径，也不检查文件/内联/远程/项目配置覆盖。Claude 的 `~/.claude.json`、各家的项目/profile/策略/CLI/会话覆盖，以及 Hermes 桌面包的独立路径均不在本次探测内。桌面环境可能不同于终端。

官方资料与产品边界见 [agent-catalog.md](agent-catalog.md)、[Claude Code](../integrations/claude-code.md)、[Pi](../integrations/pi.md) 和 [Grok Build](../integrations/grok-build.md)。资料证明发现规则，不证明本机安装真实性、可执行性或有效配置。

## 版本与安全边界

自动发现中新四家版本未知、能力未核实，不执行 `--version` 或诊断命令补齐。Pi 仅从解析入口附近读取有界 `package.json`，匹配 `@earendil-works/pi-coding-agent` 与 `bin.pi` 目标后展示 metadata 版本；只有 `1.1.0` 关联既有 P02 文档能力，仍不等于真实运行验证。Grok 仅从已核实的版本化下载文件名提取未核实线索。托管 launcher 不执行/解析内容，不挑 releases 中的版本冒充当前入口；无法关联时保持未知。

配置仅观测固定路径的类型/存在状态，不读取配置、认证或密钥内容，不返回配置片段、凭据或原始 I/O 错误。残留配置不代表已安装；文档能力不代表连接成功或部署可用。没有证据时不将未知版本标为不受支持。

Rust 在 blocking pool 内执行只读发现，不提供通用脚本 API、持久化或新增 Tauri 权限。TS 严格验证六条身份/顺序、按 Agent 区分文件与证据，拒绝缺失、重复、矛盾与额外字段。

详情页另有已实现的主动「读取版本」：展示固定命令与风险，二次确认后才执行，浏览器预览不可用。CLI 输出只返回安全版本标记，与发现元数据/能力证据分开；重新发现时清除窗口内 CLI 结果。限制与核验边界见 [agent-version-probe.md](agent-version-probe.md)。主动执行不是沙箱，所选程序可能读取配置/凭据、访问网络、写文件或启动其他进程；这些副作用不属于自动发现。

## 页面与验证边界

列表状态与版本同一行两端对齐；状态前保留装饰圆点及可访问文字。当前已发现为绿色，未发现为灰色，检查失败/入口不可用为红色；刷新失败的旧结果不会维持成功绿色。六项在检查中、预览、失败和未发现时都显示；未发现可打开详情并指定入口。浏览器预览不扫描。首次失败不冒充未安装；刷新失败保留并标注上次结果。双语详情展示路径、存在状态与优先级限制；独立路径在导航/语言切换后保留，返回恢复来源条目焦点，异步 guard 忽略过期/卸载回复。

Rust/IPC/UI 测试覆盖六家自动与手动路径、配置残留、环境覆盖、元数据边界、无执行/无配置读取、旧结果、扫描锁与生命周期。完整前端检查、Rust fmt/Clippy/测试与锁定 macOS no-bundle 构建通过；单次运行的平台与工具版本见 Git/CI。既有隔离 Linux 样本仅验证当时 Pi/Grok 原生 IPC，不替代新增四家运行检查。

六家真实 CLI 安装、原生运行、人工双语/最小窗口/长路径/键盘验收已通过（2026-10-13）；Windows/Linux 与专门读屏延后到发布前平台验证。唯一任务状态见 [todo.md](todo.md)。
