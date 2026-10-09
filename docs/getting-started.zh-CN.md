# 从这里开始

当前仓库是可以运行的桌面骨架。接入 API、写入 Agent 配置、管理 Skill 和 MCP
会按 `docs/plans/phase-1.md` 逐项实现。

## 启动

需要 Node.js 24 LTS、Rust，以及 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)。
macOS 桌面开发通常从 Xcode Command Line Tools 开始；如构建报错，按官方文档
检查 Xcode 环境。安装命令是 `xcode-select --install`。

```sh
cd /Users/johnny/code/personal/vibemate
npm ci
npm run tauri dev
```

第一次运行会下载并编译 Rust 依赖，耗时通常比后续启动长。
`npm run dev` 仅预览网页；`npm run tauri dev` 同时启动网页开发服务器与 Rust 桌面程序。

## 建议阅读顺序

1. `src/App.tsx`：React 组件用 JSX 描述界面。`useState` 保存界面状态，
   `useEffect` 在组件挂载后读取桌面信息。
2. `src/lib/desktop.ts`：`invoke` 通过 Tauri 的 IPC 调用 Rust。
   IPC 是前端与本地进程之间的通信；TypeScript 类型用于约束调用结果。
3. `src-tauri/src/commands.rs`：`#[tauri::command]` 把 Rust 函数标记为可被前端调用。
   `Serialize` 允许返回值转成 JSON；这里使用 camelCase 与前端字段命名保持一致。
4. `src-tauri/src/lib.rs`：注册命令并启动窗口。
5. `src-tauri/tauri.conf.json`：窗口、开发服务器和打包配置。
   `src-tauri/capabilities/default.json` 控制窗口可使用的 Tauri 权限。

Rust 管系统操作、网络、密钥和配置写入；React 管界面。先理解这一次只读调用，
再逐步增加功能，不必一次掌握全部技术。代码注释使用英文，学习说明可以使用中文。

## 检查与 CI

`npm run typecheck` 检查 TypeScript；`npm run build` 构建前端。
`cargo fmt` 统一 Rust 格式，`cargo clippy` 检查常见问题，`cargo test` 运行测试。
完整命令见 `CONTRIBUTING.md`。

GitHub CI 的定义位于 `.github/workflows/ci.yml`。推送到 GitHub 后，工作流会在
macOS、Windows、Linux 上检查并构建。初期不自动发布或签名安装包。

下一步先确认 Command Code GOAT、Pi、Grok Build 的官方资料与本机安装情况，
再实现第一条 Provider → Model → Agent 配置流程。
