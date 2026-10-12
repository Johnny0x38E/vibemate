# 用户主动读取 Agent CLI 版本

## 已确认范围

维护者确认现在实施独立的版本探测。自动发现继续只检查路径、存在状态与有界元数据，不启动程序。详情页先展示入口、解析目标、固定命令与风险，用户再次确认后才由 Rust 启动选定 CLI；浏览器预览不能执行。

固定六家均使用已核实的 `--version`：

| Agent       | 证据                                                                                                                       | 解析范围                                        |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| Claude Code | [CLI reference](https://code.claude.com/docs/en/cli-reference)                                                             | 版本号及可选 `(Claude Code)` 后缀               |
| Codex       | [official CLI parser](https://github.com/openai/codex/blob/main/codex-rs/cli/src/main.rs)                                  | `codex-cli` / `codex` 的版本行                  |
| OpenCode    | [CLI](https://opencode.ai/docs/cli/)                                                                                       | 单独版本号                                      |
| Pi          | [CLI](https://pi.dev/docs/latest/cli)                                                                                      | 单独版本号                                      |
| Grok Build  | [official README](https://github.com/xai-org/grok-build)、[既有本机记录](../integrations/grok-build.md)                    | 版本号，可带括号内提交号                        |
| Hermes      | [official fast-version implementation](https://github.com/NousResearch/hermes-agent/blob/main/hermes_cli/_startup_fast.py) | `Hermes Agent v...` 首行，忽略附加安装/更新信息 |

资料核对日期为 2026-10-12。Codex 官网 CLI 链接重定向，Grok 的旧 CLI 页面返回 404，以上对应使用官方源码与 README，而不猜参数。Hermes 的版本命令会解析安装/配置、检查更新，可能启动其他进程或容器；不能承诺无网络或文件副作用。

## 执行边界

- IPC 仅接受固定 Agent 身份与确认过的入口/解析目标，不接受任意参数、命令字符串或环境变量。重新检查路径，解析目标变化时拒绝并要求重新发现；此检查不能消除检查与执行之间的文件替换竞态，也不证明程序真实性。
- 直接启动重验后的绝对解析目标，固定 `--version`，不拼接 shell。Windows 的批处理文件会隐式经 `cmd.exe`，首版只执行 `.exe`，明确拒绝 `.cmd/.bat` 和其他包装入口；不偷偷回退到 shell。
- 关闭 stdin，限制 stdout/stderr 各 8 KiB；运行超时 5 秒，失败/超量/取消时请求终止并回收直接子进程。全应用只允许一个探测，不排队重复请求。
- 清除继承环境，只恢复启动所需的平台/语言/配置路径变量；不传递 API key、token 或解释器注入变量。独立临时工作路径避免把应用仓库作为项目运行；只清理空的自建目录，不递归删除程序写入的文件。
- 只返回安全的版本标记和固定证据类别，不展示或记录原始输出、内部错误及其他安装信息。不升级能力证据、不持久化版本、不改变配置。
- 这些是执行保护，不是沙箱：程序仍可访问用户文件/凭据、网络、写文件、启动后代进程；只保证对直接子进程请求终止，不承诺杀死全部后代或撤销副作用。风险说明在确认前可见。

## 已实现与核验边界

`agents/version_probe.rs` 负责受限进程与版本解析，薄命令仅转发；`agentVersions.ts` 严格核验身份、路径、版本与 `cliOutput` 证据。`AgentVersionPanel` 提供双语二次确认、取消/Escape、重试及生命周期保护；扫描与当前页导航在执行期间锁定。CLI 结果仅留在窗口中，重新发现时清除，不替代配置能力证据。Tokio 增加 `process`/`io-util` feature，Cargo 锁文件相应增加既有 `signal-hook-registry` 依赖边；没有升级依赖版本或扩大 Tauri capability。

完整前端检查通过（602 个 UI/边界测试、8 个发布工具测试、14 个翻译检查器测试）；Rust fmt/Clippy 通过，237 个测试通过，1 个既有真实凭据库测试忽略；锁定 macOS arm64 no-bundle 桌面构建通过。浏览器模拟 IPC 样本检查了中文浅色/英文深色、900px 宽度、长路径、失败旧结果及确认/取消焦点；模拟环境已移除。这不是原生 IPC 或真实安装验收。

六家真实 CLI 版本格式/副作用、Windows/Linux 原生执行、专门读屏与维护者外观验收仍待执行；品牌发布条款见 [assets README](../../assets/agents/README.md)。唯一状态清单仍为 [todo.md](todo.md)。未经另行确认具体目标，不执行维护者真实安装的 Agent 来测试；原生进程测试只启动隔离的合成入口。
