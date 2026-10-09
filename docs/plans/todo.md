# vibemate 第一阶段执行清单

状态：P00 包管理器迁移、P01–P03 接入证据、P04 UI 测试入口、P05 私有配置存储、P06 凭据接口与 P07 翻译基础、P08 语言偏好与 I01 配置精简/四空格迁移已完成；P09 暂停，先执行 I02 固定草图对应的桌面壳与窗口设计。业务功能尚未实现，首页草图与绿色启用语义已确认，修订桌面壳设计已获确认；I02.b 生产壳与 I02.c.1–c.2 macOS 覆盖式标题栏已实现并提交，文档记录（b.5/c.4）已完成。待办：b.2–b.4 的 ego-browser 双语/720×560 验收、c.1–c.2 的人工拖动/缩放/全屏确认、c.3 启动门禁拖动区、c.5 Windows/Linux 窗控决策与 I02.d「关于」。说明与设计见 [development-plan.md](development-plan.md)。
这里是唯一任务状态来源，不能在其他文件维护第二份勾选清单。

## 执行约定

- 按依赖顺序实施，一次一个可检查的行为；默认不使用子代理。
- 每个任务通常 1–5 个实际文件；建议路径不是强制架构。测试、注册、迁移和锁文件也计数。
  实际范围超过约五个文件或一个专注会话，先拆 Pxx.a/Pxx.b 子任务并补依赖。
- 所有 UI 任务必须同时补齐中英文、可访问名称和错误文案；资源更新也计入文件范围，
  超过约五个文件先拆本任务子项，而不是等功能全部完成后再统一翻译。
- 每项 Verification 加上本阶段适用的 `development-plan.md` 完成标准。
  Rust 过滤测试必须确认执行条数非零；UI 命令在 P04 后才存在。
- 完成任务时在该任务末尾追加实际命令、版本/平台、结果与提交号，再勾选验收条件。
- 检查点用于报告结果与记录维护者意见；已授权范围可继续，必需的范围决策要先解决。

## 开始实施前

- [x] 维护者已明确授权开始实施，并决定先迁移至 pnpm（2026-10-09）。
- [x] 已读取最新 AGENTS 文件、代码和本清单；Zed 配置已独立提交并推送为 `a2d0eab`，开始迁移前工作区干净。

## 前置任务 P00：迁移至 pnpm

维护者在开始功能实施前选择 pnpm。迁移范围包含安装、发布校验、CI、Tauri、Zed
和文档，因此先拆成以下子任务；完成后继续 P01，不扩展业务功能。

### P00.a：固定工具版本并迁移锁文件

- [x] 在 `package.json` 固定 pnpm 当前稳定版本，替换内部 npm 脚本。
- [x] 使用 `pnpm import` 迁移已有解析结果，提交 `pnpm-lock.yaml` 并移除 npm 锁文件。
- [x] 仅批准实际需要的依赖构建脚本，冻结锁文件安装成功。

**Files:** `package.json`、`package-lock.json`、`pnpm-lock.yaml`；仅在确有依赖构建脚本时增加 `pnpm-workspace.yaml`。
**Verification:** `pnpm --version`、`pnpm install --frozen-lockfile`。
**Dependencies:** None。

### P00.b：保留发布前校验

- [x] 保留 manifest/tag 版本一致性和 changelog 校验；按 pnpm importer 验证依赖声明。
- [x] 沿用现有公开 `readReleaseMetadata` 测试边界，覆盖锁文件过期、缺失与损坏。

**Files:** `scripts/release-notes.mjs`、`scripts/release-notes.test.mjs`。
**Verification:** `pnpm run test:release`；先确认失败，再实现对应行为。
**Dependencies:** P00.a。

### P00.c：切换运行入口和远端工作流

- [x] CI/Release 安装固定 pnpm 并使用冻结锁文件；保留所有已有检查和草稿发布条件。
- [x] Tauri 前置命令和 Zed 任务使用 pnpm，正确传递参数。

**Files:** `.github/workflows/ci.yml`、`.github/workflows/release.yml`、
`src-tauri/tauri.conf.json`、`.zed/tasks.json`。
**Verification:** 完整前端、Rust 检查和本机桌面构建；远端结果单独记录。
**Dependencies:** P00.b。

### P00.d：更新开发规范与入门命令

- [x] 根目录与前端 AGENTS、README、CONTRIBUTING、前端文档使用 pnpm。
- [x] 安装说明固定版本，说明 npm 仅用于首次安装 pnpm；共享配置与 CI 命令一致。

**Files:** `AGENTS.md`、`src/AGENTS.md`、`README.md`、`CONTRIBUTING.md`、`docs/frontend.md`。
**Verification:** 命令核对与格式检查。
**Dependencies:** P00.c。

### P00.e：同步发布、学习文档和计划

- [x] 同步中文入门文档、发布文档、锁文件格式忽略、changelog 和开发计划。
- [x] 记录实际版本、命令、平台、结果与限制，迁移不标记任何业务任务完成。

**Files:** `docs/getting-started.zh-CN.md`、`docs/releases.md`、`.prettierignore`、
`CHANGELOG.md`、`docs/plans/development-plan.md`；本清单随各子任务更新。
**Verification:** `pnpm run check:frontend`、差异检查和 npm 命令残留检查。
**Dependencies:** P00.d。

**执行记录（2026-10-09）：**

- macOS Apple Silicon；Node.js `26.3.0`、pnpm `12.10.1`、Rust/Cargo `1.99.0`；CI 保持 Node.js 24。
- `pnpm import` 后核对并恢复两项补丁版本，最初 18 个已有直接依赖版本保持一致；远端 CI 后续暴露发布年龄限制，修正结果见下方；仅新增发布工具需要的 `yaml@2.9.1`（ISC，MIT 兼容）。
- `pnpm install --frozen-lockfile` 通过；不需要额外依赖构建脚本，因此未新增 workspace 或放宽构建审批策略。
- `pnpm run check:frontend` 通过，发布测试实际执行 8 条；测试先因旧 npm 锁文件读取逻辑失败，迁移实现后通过。
- Rust fmt、Clippy 通过；`cargo test --manifest-path src-tauri/Cargo.toml --locked` 成功，但当前骨架实际为 0 条测试，不作为业务测试覆盖。
- `pnpm run tauri build --no-bundle -- --locked` 通过，产物为 `src-tauri/target/release/vibemate`。
- 两份 workflow YAML 可解析，保留冻结安装与全部既有检查；Windows/Linux、Node.js 24 和远端草稿 Release 尚待实际工作流验证。
- Zed 配置提交 `a2d0eab` 已推送；迁移提交 `3f75043` 已推送；P01 资料核实继续，不标记其他业务任务完成。

### P00.f：修复干净 CI 安装的发布年龄限制

- [x] 使用符合 pnpm 默认 24 小时发布年龄策略的 Tauri API `2.12.1` 与 Vite `8.3.3`，不禁用策略。
- [x] 在隔离 store/cache 中验证冻结安装，重新完成前端与桌面构建。
- [x] 核对实际 GitHub CI 结果，记录补丁版本调整和平台结果。

**Files:** `pnpm-lock.yaml`、`CHANGELOG.md`；本清单记录验证结果。
**Dependencies:** P00.a–P00.e。
**执行记录：** `3f75043` 的远端前端检查在安装阶段失败：Tauri API `2.12.2` 与 Vite `8.3.4` 发布未满 24 小时，本机缓存未暴露此限制。此前本机成功不代表干净 CI 安装通过。修正后隔离 store/cache 冻结安装、完整前端检查（8 条发布测试）和 macOS 桌面构建再次通过；修复提交 `dd4c710` 已推送；[CI run 37908290812](https://github.com/Johnny0x38E/vibemate/actions/runs/37908290812) 的前端、macOS、Windows、Linux 四个 job 全部成功，三平台均完成原生检查和桌面构建。标签触发的四目标草稿 Release 未执行。

## 阶段 A：接入证据

### Task P01: 核实三家 Provider 的接入约定

**Description:** 查阅准确的官方资料，记录协议、认证、端点、模型发现与参数支持，先确认是否能直接用于目标 Agent。

**Acceptance criteria:**

- [x] 记录三家产品的准确名称、官方来源、证据日期和可确认的 API 类型；未知项明确标记。
- [x] 整理认证、模型列表、工具/图像/推理字段及错误响应，示例不含真实密钥。
- [x] 确认第一条兼容路径；缺少 Command Code GOAT 资料时列出具体待补链接，不猜测端点。

**Verification:**

- [x] 人工核对每个结论的原始来源与示例。
- [x] 运行 `pnpm run format:check`，检查文档链接。

**Dependencies:** None。

**Files likely touched:**

- `docs/integrations/providers.md`

**Estimated scope:** S：1 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录（2026-10-09）：**

- 官方页面与官方文档源码由 ego-browser 核对；身份、协议、认证、模型元数据与字段记录在 `docs/integrations/providers.md`。
- Command Code GOAT 的准确官方资料已找到；Claude 的 Messages 路由不能当作 Chat Completions。
- 首条 DeepSeek → Pi 的文档级 Chat 协议路径已确认；安装版本、配置优先级和真实调用仍由 P02/后续任务验证。
- 没有使用真实凭据或发出模型推理请求；DeepSeek 完整错误体样本和未确认参数组合明确保留待验。
- 验证：JSON 示例解析与官方来源阅读；`pnpm run format:check`。本任务仅文档，不重复原生构建。
- P00 迁移提交 `3f75043`，CI 发布年龄修复提交 `dd4c710`。本任务文档提交 `e8f2c4a`；P01 的三个 JSON 示例解析成功，格式检查通过。

### Task P02: 核实 Pi 的配置能力

**Description:** 确认实际发行来源和安装版本，以脱敏示例记录原生配置与生效方式。

**Acceptance criteria:**

- [x] 确认全局/项目/环境变量优先级、Provider 协议、模型字段、认证方式及重启要求。
- [x] 记录 Skill 目录、格式与 MCP 支持情况；区分原生能力与第三方扩展。
- [x] 记录独立启动时密钥来源、配置文件权限与平台限制；不读取或输出真实密钥。

**Verification:**

- [x] 人工用官方资料与可用安装版本核对，未安装的平台标记待验。
- [x] 运行 `pnpm run format:check`。

**Dependencies:** P01。

**Files likely touched:**

- `docs/integrations/pi.md`

**Estimated scope:** S：1 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录（2026-10-09）：**

- macOS Apple Silicon，Pi `1.1.0`，准确包名 `@earendil-works/pi-coding-agent`；官网与安装包附带文档/源码交叉核对。
- `docs/integrations/pi.md` 记录目录、认证优先级、模型字段、Skill 原生资源排除、原生 MCP enabled 与项目覆盖。
- 临时纯解析检查通过：模型 schema 接受且不解析凭据、MCP 项目停用保留全局字段/未受信任项目被忽略、symlink Skill 发现；临时文件已清理。
- JSON 示例解析、`pnpm run format:check`；仅文档改动不重复原生构建。
- 未启动真实会话、MCP、Skill 脚本或模型请求，也未读取真实 auth.json；Windows/Linux 和运行中重载保持待验证。
- 已将首页启停语义记录到 architecture/development-plan；下一项按依赖为 P03。本任务提交为 `b07afbb`。

### Task P03: 核实 Grok Build 的配置能力

**Description:** 确认 Grok Build 对应的准确产品与版本，列出可实现的注入约定。

**Acceptance criteria:**

- [x] 记录官网/发行来源、配置位置、原生协议与可配置字段，避免混淆同名产品。
- [x] 记录独立启动认证、配置优先级、Skill/MCP 能力及跨平台支持。
- [x] 对需要启动器、桥接器或没有配置接口的功能明确列出缺口，保留维护者决策。

**Verification:**

- [x] 人工对照原始来源，检查脱敏示例可解析。
- [x] 运行 `pnpm run format:check`。

**Dependencies:** P01。

**Files likely touched:**

- `docs/integrations/grok-build.md`

**Estimated scope:** S：1 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录（2026-10-09）：**

- macOS Apple Silicon，`grok 1.0.50 (c58f321264ba)`；官方产品、安装与协议文档核对，短提交在公开仓库未找到，版本差异明确保留。
- `docs/integrations/grok-build.md` 记录五层配置、项目范围、认证、model backend、Skill/MCP 开关和 inspect 省略关闭 MCP 的本机行为。
- 临时 GROK_HOME 的 inspect、symlink Skill disabled 发现、MCP enabled/disabled 发现差异与原生启停持久化/无关字段保留检查通过；未启动会话或 Server。
- Python 3.11 tomllib 示例解析、`pnpm run format:check`；真实凭据、协议请求和 Windows/Linux 行为保持未验证。
- `docs/integrations/compatibility.md` 完成 C03 能力矩阵；当前无必须新增代理/启动器的前置决策，进入 P04 测试基础。
- 本任务提交 `cc68e78`。用户已有 `docs/getting-started.zh-CN.md` 删除保持原样，不纳入本任务提交。

### Checkpoint C03: 接入范围已明确（P01–P03）

- [x] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [x] 三家 Provider、两个 Agent 的协议/认证/Skill/MCP 支持矩阵已记录，缺项已提出。
- [x] 维护者已指示继续 P04，并确认继续 pnpm；当前无必需的协议或范围决策。

**C03 执行记录：** P01–P03 官方证据与本机隔离验证已记录；矩阵区分协议入口与真实接通。未实现业务能力保持未实现，本阶段没有额外桥接范围决策。

## 阶段 B：测试、存储与中英文基础

### Task P04: 建立首个 UI 行为测试入口

**Description:** 引入当前稳定、兼容现有 Vite 的 UI 测试工具，为已有桌面信息加载增加行为测试，不在准备测试环境时提前实现 Provider 表单。

**Acceptance criteria:**

- [x] 选择并说明最小测试组合，建议 Vitest + Testing Library，提供 `pnpm run test:ui`。
- [x] 替换现有 desktop 边界，验证加载完成、失败反馈和卸载后忽略异步结果，测试不是静态标签快照。
- [x] 把 UI 测试接入 `check:frontend`，同步 ESLint/TypeScript 测试环境，不降低严格规则。

**Verification:**

- [x] 运行 `pnpm run test:ui src/App.test.tsx`，确认实际执行上述行为测试。
- [x] 运行 `pnpm run check:frontend`；如另需 setup 文件或额外配置，先补拆子任务。

**Dependencies:** P01,P02,P03。

**Files likely touched:**

- `package.json`
- `pnpm-lock.yaml`
- `vite.config.ts`
- `src/App.test.tsx`
- `eslint.config.mjs`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

### P04.a：测试入口与首个加载行为

- [x] 核对稳定 Vitest/Testing Library/DOM 环境与 Vite/Node 的兼容性。
- [x] 添加 runner 与桌面信息加载测试，保留严格 TypeScript/ESLint。

**Files:** `package.json`、`pnpm-lock.yaml`、`vite.config.ts`、`src/App.test.tsx`。
**Verification:** 目标 UI 测试、双 tsconfig 类型检查、现有 lint。
**Dependencies:** P03。

### P04.b：失败与生命周期回归

- [x] 覆盖失败反馈、浏览器预览与 StrictMode 清理后的旧成功/失败请求。
- [x] 临时移除清理保护时回归测试失败，恢复后通过；不改生产行为来迎合测试。

**Files:** `src/App.test.tsx`；仅在实际问题需要时修改 `src/App.tsx`。
**Verification:** 实际测试数量非零，记录临时变异验证结果。
**Dependencies:** P04.a。

### P04.c：完整检查与学习说明

- [x] `test:ui` 纳入 `check:frontend`，CI/Release 继续使用同一完整入口。
- [x] 更新公开测试说明与 changelog，学习材料写入被忽略的 docs/local。

**Files:** `package.json`、`docs/frontend.md`、`CHANGELOG.md`、`docs/local/p04-ui-tests.zh-CN.md`；本清单随子任务记录。
**Verification:** 完整前端检查、冻结安装与相关原生检查；不宣称 DOM 测试证明真实桌面行为。
**Dependencies:** P04.b。

### P04.d：修复 Zed 的生成锁文件诊断

- [x] 只将 pnpm-lock.yaml 设为 Plain Text，保留普通 YAML 检查和冻结锁文件校验。
- [x] 验证 Zed 中原先第 158 行的多文档错误消失，记录兼容限制。

**Files:** `.zed/settings.json`、`docs/frontend.md`；changelog/本清单与 P04.c 合并记录。
**Dependencies:** P04.a。

**执行记录（P04.a–P04.d，2026-10-09）：**

- 实现提交：`2daff2a`（依赖、`test:ui`、App 行为测试与 Vite 测试环境）；`28368cb`（`check:frontend` 文档、Zed 锁文件规则、`.prettierignore`、CHANGELOG 与 README 旧链接）。两提交已随 `18b0e4f` 推送，远端 CI 结果见 C06 记录。
- 依赖（均为 MIT，已核对 registry 的 latest 稳定版）：Vitest `5.0.3`、React Testing Library `16.3.3`、DOM Testing Library `10.4.2`、jsdom `30.1.2`。RTL 的对等依赖支持 React 19 与 `@testing-library/dom ^10`；Vitest 5 的对等依赖包含 Vite `^8.0.0`，本项目 Vite `8.3.3` 满足。Vite 最新 `8.3.4` 尚未满足 pnpm 24 小时发布年龄策略，因此保持 `8.3.3`。jsdom 30 在 Node 24 线要求 `^24.15.0`，与 `package.json` engines 一致。
- 环境：macOS（Darwin 27.0.0，arm64）；pnpm `12.10.1`；Node.js `24.16.0`（CI 使用的 24 线）与 `26.3.0`；Rust/Cargo `1.99.0`。
- 完整检查：`pnpm install --frozen-lockfile` 通过；`pnpm run check:frontend` 在 Node 24.16.0 与 26.3.0 下均通过（Prettier、ESLint 与 oxlint 零警告、发布测试 8/8、UI 测试 5/5、双 tsconfig、Vite 构建）。
- 目标测试：`pnpm run test:ui src/App.test.tsx` 执行 5 条，覆盖加载完成、失败反馈且不显示底层错误、浏览器预览，以及 StrictMode 清理后旧成功/旧失败响应均不覆盖当前结果。
- 变异验证：临时删除 `App.tsx` 中的 `active = false` 清理保护后，两条旧响应用例失败（2 failed / 3 passed）；按字节恢复后 `src/App.tsx` 与提交版本一致，5 条再次通过。生产代码未改动。
- 卸载语义：“卸载后忽略异步结果”通过 StrictMode 的 cleanup 后重新 setup 的新旧响应竞争验证。单纯卸载后检查页面为空不能证明保护有效，原因见学习说明。
- 原生检查：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`（0 条，当前骨架尚无业务测试）与 `pnpm run tauri build --no-bundle -- --locked` 均通过。
- 限制：DOM 测试不证明 Tauri WebView、Rust IPC、真实窗口或 Provider 连接；Windows/Linux 上的 UI 测试待远端 CI 验证。Zed 误报清除由维护者确认。
- 学习说明位于被 Git 忽略的 `docs/local/p04-ui-tests.zh-CN.md`，不进入提交；`getting-started` 学习文档已按授权移入 `docs/local/`，README 中的公开旧链接已移除。

### Task P05: 建立私有配置存储

**Description:** 在首次 Provider 保存之前提供 app-data 定位、SQLite 初始化与最小迁移，不预建全部业务表。

**Acceptance criteria:**

- [x] 使用平台 app-data 目录与临时测试数据库，创建 schema version 并可重复初始化。
- [x] 迁移失败保持旧数据可读且报告安全错误；不写入项目目录或数据库密钥字段。
- [x] 确定连接与并发访问方式，只有需要的业务表随后续任务增加。

**Verification:**

- [x] 运行 `cargo test --manifest-path src-tauri/Cargo.toml --locked storage`，覆盖首次启动、重复初始化与失败迁移。
- [x] 运行 Rust fmt/Clippy；在 Tauri 中检查实际数据目录。

**Dependencies:** P01,P02,P03。

**Files likely touched:**

- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/storage.rs`
- `src-tauri/src/lib.rs`

**Estimated scope:** M：4 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录（2026-10-09）：**

- 实现提交：`dab6344`（`feat: add private SQLite configuration storage`）；涉及 `Cargo.toml`、`Cargo.lock`、`src/storage.rs`、`src/lib.rs` 与 CHANGELOG。已随 `18b0e4f` 推送，远端 CI 结果见 C06 记录。
- 依赖：`rusqlite 0.40.2`（MIT，与 `cargo info` 显示的最新版本一致），关闭默认特性，只启用 `bundled`，使三平台使用同一版 SQLite（`libsqlite3-sys 0.38.2`，MIT）。新增传递依赖 `fallible-iterator`、`fallible-streaming-iterator`、`vcpkg` 为 MIT/Apache-2.0。
- 设计决定：
    - 连接：一个 `Connection` 放在 `Mutex` 中，每次短操作加锁；跨进程写入由 SQLite 串行化，`busy_timeout` 等待 5 秒；保留默认回滚日志，数据库是单个文件，便于 P19 的备份与恢复。
    - 版本与迁移：版本号存于 SQLite `user_version`；每个迁移与版本更新在同一事务提交，失败则回滚。事务使用 `BEGIN IMMEDIATE`，先取得写锁再读取版本，两个应用实例不会同时执行同一迁移。
    - 迁移器：维护者委托判断后，决定保留自写迁移器，不引入 `rusqlite_migration`。`rusqlite_migration 2.6.0` 为 Apache-2.0、要求 Rust 1.95，依赖 `rusqlite ^0.40`，与本项目兼容，但会新增依赖；当前只需要线性版本号与事务升级，自写实现很短并由测试覆盖。若将来需要降级迁移或 Rust 数据改写，再评估引入。
    - 版本基线：版本 1 只标记数据库已版本化，不建业务表。
    - 更新版本的数据库：版本高于本构建时拒绝写入，避免误读未知表。
    - 启动失败：维护者决定应用仍然打开。`StorageStatus` 将“已就绪的数据库”或“不可用原因”保存在 Tauri 托管状态中；失败原因以安全文案输出到 stderr（尽力写入，写入失败不影响启动）。不可读的文件不会被删除、重命名或改写，之后的备份导入与重建功能负责修复。界面中的存储状态提示随 P09/P10 的错误码与翻译一起实现。
- 后续修正（同日，维护者要求）：`64a2f89` 让应用在存储不可用时仍然打开；`09ee8f5` 让迁移改用立即写锁。两次改动都有测试与真实 Tauri 检查，见下方。
- 测试：`cargo test --manifest-path src-tauri/Cargo.toml --locked storage` 执行 9 条，覆盖：首次创建目录与数据库并到最新版本；重复打开保留数据且不重复执行迁移；多版本升级保留已有行；失败迁移回滚、旧数据仍可读，错误文本不含 SQL 或路径；拒绝更新版本的数据库且不修改；无法读取的文件返回不可用状态且字节不变；四个线程同时打开时每个迁移只执行一次；迁移编号连续；状态可跨线程共享。完整 `cargo test --locked` 为 17 条通过、1 条手动测试忽略。
- 变异验证：(a) 临时把迁移事务改为丢弃时提交（`DropBehavior::Commit`），失败迁移测试失败（退出码 101）；(b) 临时把立即事务改回普通事务，并发打开测试连续 10 次全部失败（断言 `opened.iter().all(...)` 不成立）。两次都按字节恢复，恢复后测试通过。
- 静态检查：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings` 通过；`pnpm run format:check` 通过（CHANGELOG）；`pnpm run tauri build --no-bundle -- --locked` 通过。
- 真实 Tauri（macOS arm64，发布构建产物）：
    - 首次启动在 `~/Library/Application Support/dev.vibemate.desktop/vibemate.sqlite3` 创建数据库；`PRAGMA user_version` 为 1，`integrity_check` 为 ok，无业务表；项目目录未生成数据库。
    - 把版本改为 9 后启动：应用保持运行，stderr 输出安全文案（数据库版本 9，本构建支持到 1），数据库仍为版本 9，文件 SHA-256 不变。
    - 把数据库替换为无法读取的文件后启动：应用保持运行，stderr 输出“vibemate could not open its private configuration database.”，文件 SHA-256 不变。
    - 三种情况都用 `perl -e 'alarm 12; exec @ARGV'` 限时运行，退出码 142 表示应用一直运行到闹钟触发；验证后删除了测试生成的数据目录。
    - 验证结束后删除了测试生成的数据目录（验证前该目录不存在）。
- 限制：Windows/Linux 上的 bundled SQLite 编译与路径行为待远端三平台 CI 验证；数据库文件权限未额外收紧（位于用户私有的 Application Support 目录），备份权限留给 P19/P37 验证；错误文案目前为英文开发者文本，P09 改为错误码加翻译。本任务未接触 OS 凭据库，数据库中没有密钥字段。

### Task P06: 建立 OS 凭据接口

**Description:** 实现可测试的凭据写入、读取和删除边界，供账户配置使用。

**Acceptance criteria:**

- [x] 生产使用 OS 凭据库，测试使用可注入假实现；状态输出不含明文密钥。
- [x] 覆盖不可用、取消访问、替换失败和删除失败，不回退为明文数据库。
- [x] 定义与数据库保存的补偿步骤和私有备份处理，避免孤立凭据。

**Verification:**

- [x] 运行 `cargo test --manifest-path src-tauri/Cargo.toml --locked credentials`。
- [x] 运行 Rust fmt/Clippy；本机凭据库 smoke check 使用临时测试条目并清理。

**Dependencies:** P05。

**Files likely touched:**

- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/credentials.rs`
- `src-tauri/src/lib.rs`

**Estimated scope:** M：4 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录（2026-10-09）：**

- 实现提交：`af5de68`（`feat: add OS credential store boundary`）；涉及 `Cargo.toml`、`Cargo.lock`、`src/credentials.rs`、`src/lib.rs`。已随 `18b0e4f` 推送，远端 CI 结果见 C06 记录。
- 维护者决定：Linux 使用 Secret Service；无密钥环服务时明确失败，不回退为明文。
- 依赖：`keyring 4.2.0`（MIT OR Apache-2.0，registry 最新稳定版，最低 Rust 1.88，本机工具链 1.99）。使用默认 `v1` 特性：macOS Keychain、Windows Credential Manager、其他 Unix 的 Secret Service；首次 `Entry::new` 自动选择平台后端。新增 69 个锁定包，许可证扫描均属 MIT、Apache-2.0、Zlib、BSD 等兼容类别。MPL-2.0 项（cssparser、selectors 等）早于本任务即由 Tauri 依赖引入。
- 设计：
    - `CredentialStore` 包含 `save`、`load`、`delete`；数据库只保存非敏感引用。生产实现为 `OsCredentialStore`，测试使用内存假实现。
    - `Secret` 的 `Debug` 输出占位符，取值必须调用 `expose`，便于审查。
    - `CredentialError` 不携带底层数据。部分 keyring 错误（如 `BadEncoding`）含原始密钥字节，映射时直接丢弃。
    - 补偿流程 `replace_then_commit`：读取旧值作为仅内存的私有备份；写入新值；执行数据库提交。提交失败则回写旧值，或在没有旧值时删除新引用。回写失败时返回 `restored: false`，调用方必须保留恢复记录（P19/P34 处理）。
    - 删除不存在的条目视为成功，保证幂等。
- 测试：`cargo test --manifest-path src-tauri/Cargo.toml --locked credentials` 执行 8 条，覆盖 Debug 脱敏、不可用时提交前停止、访问被拒绝时保留旧值、提交失败回写旧值、无旧值时删除新引用、回写失败被报告、删除失败被报告、平台错误映射。完整 `cargo test --locked` 为 14 条通过、1 条手动测试忽略。
- 变异验证：临时去掉“提交失败后回写旧值”，两条补偿测试失败（退出码 101）；按字节恢复后 8/8 通过。
- 真实钥匙串冒烟（macOS arm64）：`cargo test --manifest-path src-tauri/Cargo.toml --locked credentials -- --ignored` 使用临时合成条目完成保存、读取、删除，删除后读取为空；随后 `security` 查询确认未残留 `dev.vibemate.desktop` 条目。
- 静态与构建：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings` 通过；`pnpm run tauri build --no-bundle -- --locked` 通过。
- 限制：
    - macOS 用户取消钥匙串授权（`errSecUserCanceled`，-128）未被 keyring 单独映射，当前归为 `OperationFailed`，界面只能显示通用失败。“取消访问”的边界目前以 `AccessDenied` 类别覆盖，细分需真机确认后单独处理。
    - Linux Secret Service 与 Windows Credential Manager 未在本机运行。需远端 CI 编译，并在真实桌面上验证无服务、锁定、删除等行为（P37）。
    - `Secret` 离开作用域时不会清零内存（尚未引入 zeroize）；平台库内部的副本也不由本项目控制。
    - 错误文本目前为英文开发者文本，P09 改为错误码加翻译。
    - 补偿流程已定义，但还没有业务功能调用它（P10–P11 接入）。

### Checkpoint C06: 存储与测试基础可用（P04–P06）

- [x] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [x] 前端完整检查、Rust fmt/Clippy/测试与本机原生构建通过；基础没有扩张成完整框架。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

**C06 执行记录：** 提交 `18b0e4f`（含 P04–P06）已推送，远端 [CI run 37923290123](https://github.com/Johnny0x38E/vibemate/actions/runs/37923290123) 的 Frontend checks、Desktop（ubuntu-24.04）、Desktop（macos-latest）与 Desktop（windows-latest）全部成功。远端 CI 只证明编译与自动测试通过；被忽略的真实钥匙串冒烟测试未在 CI 中运行，Linux Secret Service 与 Windows Credential Manager 的真实凭据行为仍待真机验证（P37）。维护者审阅结论尚未记录。

### Task P07: 建立中英文翻译资源

**Description:** 引入当前稳定的 i18n 方案，建议 i18next/react-i18next，提供语言解析、类型化翻译键与中英文资源。此任务只建立基础，P09 接入实际页面。

**Acceptance criteria:**

- [x] 支持简体中文 `zh-CN` 和英文 `en`；中文系统语言映射到简体中文资源，其余不支持语言回退英文，初始化完成前不闪烁错误语言。
- [x] 用稳定、按功能分组的翻译键，支持参数插值、复数和 `Intl` 格式化；不拼接片段句子，不使用界面中文句子作为 key。
- [x] 两种资源覆盖相同 key/插值参数，约束类型；模型 ID、URL、用户内容和服务商品牌不自动翻译。

**Verification:**

- [x] 核对语言解析表与资源 key/插值参数一致性，覆盖中文变体、英文和未知语言样本；对应自动检查在 P09 接入。
- [x] 运行 `pnpm run check:frontend`；不得为 i18n 依赖降低 TypeScript 或 lint 规则。

**Dependencies:** P04。

**Files likely touched:**

- `package.json`
- `pnpm-lock.yaml`
- `src/i18n/index.ts`
- `src/locales/zh-CN.json`
- `src/locales/en.json`

**Estimated scope:** M：5 个建议主文件；复杂类型声明或新增测试文件需要提前补拆子任务。

### P07.a：稳定依赖与双语资源

- [x] 核对 i18next/react-i18next 稳定版本、兼容性与许可证，保留发布年龄策略。
- [x] 为现有首页与语言设置准备成组的双语资源，品牌与用户内容通过原值传入。

**Files:** `package.json`、`pnpm-lock.yaml`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** 冻结安装、资源 key/插值核对。
**Dependencies:** P04。

### P07.b：类型化初始化与语言行为测试

- [x] 提供无导入副作用、可等待的初始化，中文变体映射简体中文，未知语言回退英文。
- [x] 约束翻译键，验证插值、复数、Intl 格式化和英文后备；不接入页面或持久化。

**Files:** `src/i18n/index.ts`、`src/i18n/index.test.ts`。
**Verification:** 目标测试、完整前端检查；不降低严格类型/lint。
**Dependencies:** P07.a。

### P07.c：验证记录与开发说明

- [x] 说明初始化边界、JSON 的类型约束范围与 P08/P09 接入点，更新英文 changelog。
- [x] 记录实际测试数量、平台和限制，不宣称现有界面已经双语化。

**Files:** `docs/frontend.md`、`CHANGELOG.md`、`docs/plans/development-plan.md`；本清单随子任务更新。
**Verification:** `pnpm run check:frontend`、差异检查。
**Dependencies:** P07.b。

**执行记录（2026-10-09）：**

- 环境：macOS arm64；Node.js `26.3.0`、pnpm `12.10.1`。本任务未修改 Rust、IPC 或原生权限，未重复原生构建。
- 依赖：registry latest 为 `i18next 26.4.2` 与 `react-i18next 17.0.16`，均 MIT；后者 peer 要求 React >=16.8、i18next >=26.2、TypeScript 5/6/7，与本项目兼容。固定版本并保留 pnpm 发布年龄策略；`pnpm install --frozen-lockfile` 通过。
- 官方 i18next 资料核对了 createInstance、initAsync、CustomTypeOptions、英文后备与 Intl 格式化；使用内置格式化，不增加语言检测或格式化依赖。
- `src/i18n/index.ts` 没有导入时初始化副作用；调用方传入解析后的语言并等待 ready 实例。JSON 类型约束翻译键，不承诺静态检查插值参数；资源一致性测试核对 key 与参数。
- `pnpm run test:ui src/i18n/index.test.ts` 执行 16 条通过：10 个语言样本，以及初始化、资源一致性、品牌/版本插值、复数/数字、日期与缺少中文时的英文后备。P09 将增加空值/复数完整性校验。
- 测试先因缺少实现无法加载（0 条执行，不作为行为验证）；变异验证把中文识别限制为精确 zh-CN 后，4 条变体测试失败、12 条通过。恢复正确实现后完整检查为 21 条前端测试与 8 条发布测试通过。
- `pnpm run check:frontend` 通过：Prettier、ESLint/Oxlint 零警告、双 tsconfig、测试、Vite 构建；`git diff --check` 通过。
- 范围限制：工厂返回前翻译已就绪，但当前 App 尚未调用它。真实页面的启动显示、HTML lang、窗口标题和语言偏好均由 P08/P09 实现；不能据此宣称首页双语或真实 Tauri 已验证。未执行 Windows/Linux 或远端 CI，未读取凭据或用户 Agent 配置。
- 维护者本次指示按计划继续；C06 未新增具体审阅意见，不解释为跨平台凭据验收。本次改动尚未创建 Git 提交。

### Task P08: 保存语言偏好

**Description:** 提供语言选择与 Rust 侧偏好存储，使自动识别和用户选择有明确优先级。

**Acceptance criteria:**

- [x] 提供“跟随系统 / 中文 / English”，初次默认跟随系统，用户选择优先；切换无需重启且不丢失未提交表单。
- [x] 偏好通过 Rust 存入 app-data 配置，重启恢复；不使用 localStorage/sessionStorage，浏览器预览不伪造持久化成功。
- [x] 读取完成前控制启动显示；保存失败清楚提示并保持界面/持久化状态一致，同步 `<html lang>` 与必要窗口标题。

**Verification:**

- [x] 运行 Rust `locale_preference` 保存/重读/失败测试和语言选择组件行为测试。
- [x] 运行 `check:frontend` 与 Rust fmt/Clippy；实际 Tauri 切换→重启验证。

**Dependencies:** P05,P07。

**Files likely touched:**

- `src-tauri/src/settings.rs`
- `src/lib/desktop/settings.ts`
- `src/features/settings/LanguageSelector.tsx`
- `src/features/settings/LanguageSelector.test.tsx`
- `src/i18n/index.ts`

**Estimated scope:** M：5 个建议主文件；Rust 注册、样式和语言资源增加时先拆子任务，不能漏计文件。

### P08.a：偏好存储与迁移

- [x] 追加 schema v2 的单项语言偏好表，默认跟随系统，只接受 system/zh-CN/en。
- [x] 领域函数不依赖 Tauri；覆盖保存重读、v1 升级保留数据、读写失败和非法存储值。

**Files:** `src-tauri/src/settings.rs`、`src-tauri/src/storage.rs`、`src-tauri/src/lib.rs`（模块注册）。
**Verification:** Rust `locale_preference`、完整测试、fmt/Clippy。
**Dependencies:** P05,P07。

### P08.b：类型化 IPC 边界

- [x] 注册读取/保存命令，返回稳定安全错误 code，不把 SQLite 错误传到前端。
- [x] 校验 IPC 返回值；浏览器读取明确返回 preview，保存明确拒绝，不伪造持久化成功。

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`（命令注册）、`src/lib/desktop/settings.ts`、`src/lib/desktop/settings.test.ts`。
**Verification:** IPC 包装测试、完整前端/Rust 检查、本机桌面构建与启动；真实选择→重启留给 P08.d。
**Dependencies:** P08.a。

### P08.c：语言选择器与双语反馈

- [x] 实现原生 select、保存状态和失败反馈；成功保存后再切换，失败保持原选择。
- [x] 覆盖用户选择优先、保存失败、预览禁止保存与切换不丢失输入。

**Files:** `src/features/settings/LanguageSelector.tsx`、`src/features/settings/LanguageSelector.test.tsx`、`src/features/settings/LanguageSelector.module.css`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** 组件行为测试、完整前端检查。
**Dependencies:** P08.b。

#### P08.c.1：选择器、状态与组件测试

- [x] 在组件层使用真实翻译实例，仅替换 desktop 边界；保存成功后切换，确定失败保持原选择，结果未知时禁止继续写入并提供重新读取。
- [x] 覆盖双语、跟随系统、输入保留、保存中禁止重复操作、卸载后忽略结果；使用临时页面检查样式与键盘，不接入正式首页。

**Files:** P08.c 上述五个文件；临时浏览器检查页面不作为产品入口，验证后删除。
**Verification:** 目标测试、变异验证、完整前端检查与 ego-browser。
**Dependencies:** P08.b。

#### P08.c.2：验证记录与接入说明

- [x] 更新开发说明与英文 changelog，记录组件已实现但启动接入尚未完成。

**Files:** `docs/frontend.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** 格式与差异检查。
**Dependencies:** P08.c.1。

**执行记录（P08.c，2026-10-09）：**

- 环境：macOS arm64、Node.js `26.3.0`、pnpm `12.10.1`；未新增依赖或 Rust/IPC 命令，未改变当前两空格格式配置。
- 独立组件接收经过校验的 startup snapshot 与 systemLanguage；使用真实 i18next provider，保存确认后切换。选择/反馈/重读按钮双语齐全，品牌与表单输入不翻译，不重挂载相邻表单。
- 状态区分保存中、成功、确定写失败、结果未知、重新读取中、读取失败和翻译切换失败。未知结果禁止新写入；重新读取确认后才恢复。保存已提交但翻译失败时，不误报“保存失败”或回滚成功。
- `pnpm run test:ui src/features/settings/LanguageSelector.test.tsx` 实际执行 12 条通过，覆盖：等待保存后切换并保留同一个输入节点/内容、双语确定失败/重试、未知结果重读、预览禁止保存、显式选择优先与中文系统变体、相同选择无写入、读失败重试、翻译失败与持久化成功的区分、runtime 变为不可用、重读防重复与卸载后的读/写结果保护。
- 行为先失败：未知结果用例在最初实现中得到“原选择未改变”，实际 1 failed / 2 passed；增加确认状态与重读恢复后通过。测试文件首次缺少组件时无法加载（0 条执行），不计为行为证据。
- 变异验证：临时移除保存确认后应用语言前的卸载保护，目标命令 `-t "unmounting during a save"` 执行 1 条并失败（11 条跳过），因为共享译器被旧响应切到 zh-CN；恢复后 12/12 通过。
- `pnpm run check:frontend` 通过：48 条前端测试、8 条发布测试、Prettier、ESLint/Oxlint 零警告、双 tsconfig 与生产构建；`git diff --check` 通过。
- ego-browser 在临时隔离页面、720×560 下检查真实 browser preview 禁用与解释，及明确标注的 synthetic IPC fixture 中英文“结果未知→重新读取”流程。两种语言无水平溢出；Tab 到重新读取按钮后 focus-visible outline 为 solid，Enter 触发读取与语言更新。fixture 没有调用 Rust 或保存真实配置。
- 浏览器限制：原生 select 的箭头键操作未触发预期变更，随后用 selectOption 检查选择流程；不能宣称下拉框全键盘验收通过。截图 CDP 超时，未取得图片，不宣称视觉审查完成。正式页面、原生下拉键盘与真实 Tauri 选择→重启留给 P08.d。
- 临时 QA HTML/JSX 与本次 Vite server 已清理；组件未接入 `main.tsx` 或 App，当前首页仍为英文。没有新增生产 mock、浏览器持久化或假接通状态。
- 末次完整检查出现外部改动阻塞：维护者编辑了 `.zed/settings.json`（Rust format_on_save 改为 off，并增加 JSONC 尾逗号），Prettier 因尾逗号格式失败。之前的完整通过记录仍有效，但当时的末次验证未通过，未擅自覆盖维护者设置。维护者随后选择“仅规范格式”，运行 Prettier 只移除该文件尾逗号，保留 Rust format_on_save=off；差异核对确认相对原提交只有该用户设置值变化。重新运行完整前端检查（48 条前端测试、8 条发布测试）与差异检查通过，阻塞已解决，P08.c.2 完成。
- 开发说明与英文 CHANGELOG 已更新；P08.d/P08.e 及整个 P08 保持未完成。维护者要求整个 P08 完成后先执行 I01（四空格与项目/Zed 全局配置审查），再继续 P09。未创建 Git 提交。

### P08.d：启动接线与真实验证

- [x] 启动先读取偏好，再初始化翻译与显示内容；读取失败明确提示且允许重试，不静默覆盖偏好。
- [x] 接入选择器、同步 html lang；窗口标题当前仅品牌 vibemate，不随语言翻译。
- [x] 浏览器检查 720×560 与键盘，真实 Tauri 选择→重启验证；页面其余文本由 P09 迁移。

**Verification:** 目标/完整前端检查、Tauri 实际运行。
**Dependencies:** P08.c。

#### P08.d.1：启动门禁与生命周期

- [x] 通过 LocaleStartup 的可见 UI 测试读取等待、用户选择优先、失败重试、预览、StrictMode 过期响应与切换保留输入；只替换 desktop 边界，React 与译器保持真实。
- [x] 读取成功后初始化译器再挂载应用；门禁使用系统语言的双语资源，不写默认偏好。同步 html lang 并清理语言事件订阅。

**Files:** `src/features/settings/LocaleStartup.tsx`、`src/features/settings/LocaleStartup.test.tsx`、`src/features/settings/LocaleStartup.module.css`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** 目标行为测试、完整前端检查。
**Dependencies:** P08.c。

#### P08.d.2：入口接线与真实验收

- [x] main.tsx 在 StrictMode 下使用启动门禁；不改变品牌窗口标题，不迁移 P09 页面文案。
- [x] ego-browser 检查最小尺寸/键盘与双语；实际 Tauri 保存选择后重启，确认恢复。

**Files:** `src/main.tsx`；临时验证文件不作为产品入口，使用后清理。
**Verification:** 完整前端/Rust 检查、实际桌面构建与运行；记录未完成的原生验收。
**Dependencies:** P08.d.1。

### P08.e：记录验证与接入说明

- [x] 更新开发说明、英文 changelog 和实际验证记录，不把未验证项目勾选完成。

**Files:** `docs/frontend.md`、`CHANGELOG.md`、`docs/plans/development-plan.md`；本清单随各子任务更新。
**Verification:** 格式与差异检查。
**Dependencies:** P08.d。

**执行记录（P08.d/P08.e，2026-10-09）：**

- 启动门禁先读取，再等待真实译器初始化，最后挂载选择器与 App；保存选择优先于启动时捕获的系统语言。读取与译器失败分开提示，只显示双语安全资源，不写默认值。读取失败前的门禁使用系统语言，不声称已确认用户偏好。
- ready provider 不随语言切换重新挂载；layout effect 在显示前同步 html lang，再订阅 languageChanged 并清理同一监听器。产品窗口标题保持 vibemate；App 主体英文文案仍由 P09 迁移。
- 最初目标行为测试实际 1 failed：未实现门禁时提前显示输入。实现后新增启动测试共 11 条通过，覆盖等待、显式英/中文优先、系统变体、预览、读失败与重复重试保护、译器失败重试、StrictMode 过期成功/失败、确认保存前后 HTML 语言与同一输入保留、卸载后读响应保护。
- 变异验证临时取消 effect cleanup 的失效标记，目标 obsolete StrictMode 用例执行 2 条并全部失败（9 条跳过）；恢复后全部通过。ESM 导出不能直接 spy 的测试问题已通过仅包装外部 i18next 工厂解决；正常初始化保持真实，没有 mock startup 模块。异步闭包收窄导致的 lint 错误已修复，没有削弱规则。
- 最终完整前端检查通过：59 条前端测试、8 条发布测试、Prettier、ESLint/Oxlint 零警告、双 tsconfig 与生产构建；Rust fmt、全 target Clippy 零警告与测试（26 条通过、1 条真实凭据 smoke 忽略）通过。本机 macOS no-bundle 桌面构建成功，git diff --check 通过。
- ego-browser 的同一 TaskSpace 检查正式预览页，720×560 下中/英文选择器与明确禁用保存的提示无水平溢出，html lang 分别为 zh-CN/en，标题 vibemate。英文环境通过该页 CDP acceptLanguage override 模拟；未修改用户全局设置。Tab 跳过禁用控件，未借用浏览器 mock 声称持久化成功。
- 实际 release Tauri 二进制使用隔离 HOME；确认数据库位于临时 app-data，初次没有偏好行。macOS 原生下拉框通过 Space、箭头、Enter 选择 English，出现英语保存反馈；退出并重启后选择与标签仍为 English。随后切到中文，再选择跟随系统并重启，恢复跟随系统/中文标签。SQLite 只读核对补充确认已保存行与 integrity_check=ok，不代替 UI 观察。
- 原生窗口设为 720×560，Tab 进入 select；截图已实际查看，绿色 focus-visible 轮廓和中/英文保存反馈可见，没有裁切选择器。原生读失败验证仅在临时数据库显式构造非法偏好：界面只显示读取失败/重试，没有覆盖非法行；显式修复测试数据后 Tab/Enter 重试，实际 IPC 重读并进入英语界面。
- 未增加依赖、插件、权限、生产 mock 或浏览器持久化。已关闭原生/Vite 测试进程、删除临时 app-data，并结束浏览器 TaskSpace。未访问真实凭据或用户 Agent 配置，未创建 Git 提交。Windows/Linux 原生验收和远端 CI 未运行；输入保留由真实 React 子输入的 DOM 测试验证，现有 App 尚无编辑表单。
- P08 完成；I01 已完成，下一项 P09。此完成范围是语言偏好、启动接线和选择器，并非现有页面已翻译。

**执行记录（P08.a/P08.b，2026-10-09）：**

- 环境：macOS arm64；Node.js `26.3.0`、pnpm `12.10.1`、Rust/Cargo `1.99.0`。未新增依赖、插件或权限。
- 迁移：schema v2 只增加 singleton 语言偏好表，CHECK 约束允许 system/zh-CN/en；v1 升级保留无关数据。未保存时只返回 System，不写默认行。
- Rust 领域代码不依赖 Tauri。SQLite 单条 UPSERT 原子保存，锁复用 P05；startup/poisoned lock、读写失败、非法数据使用安全错误 code。命令通过 spawn_blocking 避免 SQLite 等锁阻塞窗口线程或 async executor。
- 官方 Tauri 2 Calling Rust 页面与 Context7 资料核对了命令注册、Serialize 错误、AppHandle/managed state 和异步命令。注册读写命令，CSP/capabilities 不变；没有新增任意文件读写接口。
- `cargo test --manifest-path src-tauri/Cargo.toml --locked locale_preference` 实际执行 9 条通过：默认只读、三种选择替换/重开、v1 数据保留、真实 SQLite 只读写失败、缺表读失败、非法存储值保留、SQL 约束、锁中毒、serde 输入验证。
- 变异验证：把读取错误改为返回默认值，读失败测试实际执行 1 条并失败（退出码 101）；恢复后 9 条通过。首次仅测试文件时是编译失败，不计为行为测试证据。
- `pnpm run test:ui src/lib/desktop/settings.test.ts` 实际执行 15 条通过：preview 无 IPC/不能保存、三种偏好、四种非法读取响应、保存返回不一致、五种安全 code 与未知诊断丢弃。自动 mock 保留 invoke 泛型签名，不通过强制类型转换或规则豁免解决类型问题。
- 变异验证：删除保存响应必须等于请求值的检查，1 条确认测试失败、14 条通过；恢复后 15 条通过。
- `pnpm run check:frontend` 通过：36 条前端测试、8 条发布测试，Prettier、ESLint/Oxlint 零警告、双 tsconfig 与生产构建。`cargo fmt --check`、Clippy 全 target 零警告与完整 Rust 测试（26 条通过、1 条真实凭据 smoke 忽略）通过。
- `pnpm run tauri build --no-bundle -- --locked` 通过。实际 macOS 发布产物启动时，app-data 目录原先不存在；运行至 12 秒闹钟停止（142）。生成的数据库 user_version = 2、integrity_check = ok、locale_preference 表为空；随后只删除本次生成的数据目录。
- 限制：本次实际启动只验证 Tauri 初始化与迁移，没有声称 WebView 发出偏好 IPC、语言选择→重启或双语 UI 验收。保存成功的域级重读与 IPC mock 分别验证，不能代替真实端到端行为。Windows/Linux 与远端 CI 未运行。
- 文档和英文 changelog 已记录本次部分实现；P08.c–P08.e 及整个 P08 验收仍未完成，下一项 P08.c。未访问真实凭据或用户 Agent 配置，未创建 Git 提交。

### 插队任务 I01：四空格缩进与配置精简审查

维护者已要求立即审查，并批准保留 Zed tasks、调整其他已审查配置；因此先执行 I01，再回到 P08.d，不再等待整个 P08 完成。

- [x] 前端与适用配置使用 4 空格；生成锁文件和 Rust 源码不重新格式化。
- [x] 已对照项目与 Zed 全局相关字段：全局缩进为 4，项目设置和 EditorConfig 将其覆盖为 2；独立 Prettier 文件只写默认值，嵌套 gitignore 重复。
- [x] 精简 Zed 项目偏好、将必要格式设置合并到 package.json、移除三个冗余文件；保留 Zed tasks 的全部命令与语义。
- [x] 不修改全局设置，不削弱 CI/Release 的 lint、类型、可访问性、测试与构建；格式化前后核对非业务变更。

#### I01.a：格式化来源与冗余配置

- [x] 按维护者进一步确认删除 .editorconfig；package.json 只添加必要的 prettier.tabWidth=4，不改变原 80 列换行宽度与默认 LF 换行。
- [x] Zed settings 仅保留 ESLint、Rust 项目定位/Clippy 与锁文件类型映射；保留用户的全局偏好来源。
- [x] 删除 .editorconfig、独立 .prettierrc.json 与 src-tauri/.gitignore；保留 ESLint/Oxlint、.prettierignore 与 rust-toolchain.toml。

**Files:** `.editorconfig`、`package.json`、`.zed/settings.json`、`.prettierrc.json`、`src-tauri/.gitignore`。
**Verification:** 格式配置实际解析、忽略路径核对、任务语义与锁文件/全局配置摘要核对。
**Dependencies:** 审查结果与维护者批准。

#### I01.b：四空格格式迁移

- [x] 根据格式化器实际报告，在写入源码前补齐每组不超过五个文件的子任务。
- [x] 只由 Prettier 调整格式，用迁移前后的标准化输出与 YAML/JSON 数据核对，保留业务逻辑。

格式化器实际报告 32 个文件需要调整，在写入前拆分如下：

- [x] **I01.b.1：前端工具与 Zed 配置** — `.oxlintrc.json`、`.zed/settings.json`、`.zed/tasks.json`、`package.json`、`eslint.config.mjs`。
- [x] **I01.b.2：现有页面与桌面信息边界** — `src/App.css`、`src/App.tsx`、`src/App.test.tsx`、`src/lib/desktop.ts`、`src/main.tsx`。
- [x] **I01.b.3：翻译资源与偏好边界** — `src/i18n/index.ts`、`src/i18n/index.test.ts`、`src/locales/en.json`、`src/locales/zh-CN.json`、`src/lib/desktop/settings.ts`。
- [x] **I01.b.4：语言选择器与 IPC 测试** — `src/features/settings/LanguageSelector.tsx`、`src/features/settings/LanguageSelector.test.tsx`、`src/features/settings/LanguageSelector.module.css`、`src/lib/desktop/settings.test.ts`。
- [x] **I01.b.5：构建配置与发布脚本** — `vite.config.ts`、`tsconfig.json`、`tsconfig.node.json`、`scripts/release-notes.mjs`、`scripts/release-notes.test.mjs`。
- [x] **I01.b.6：CI 与桌面 JSON/HTML** — `.github/workflows/ci.yml`、`.github/workflows/release.yml`、`index.html`、`src-tauri/capabilities/default.json`、`src-tauri/tauri.conf.json`。
- [x] **I01.b.7：文档中的嵌套列表与示例** — `docs/integrations/pi.md`、`docs/integrations/providers.md`、`docs/plans/todo.md`。

各组只有格式化修改；不增加测试文件或更改检查规则。I01.c 的说明文字另计。
**Verification:** 各组 Prettier debug-check 与内容核对；全部迁移后完整前端检查与实际桌面构建。
**Dependencies:** I01.a；各格式子组顺序实施。

#### I01.c：说明与最终检查

- [x] 更新格式来源、项目/个人设置边界与英文 changelog，记录实际验证及限制。

**Files:** `docs/frontend.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** 完整前端与 Rust 检查、本机桌面构建、差异核对。
**Dependencies:** I01.b。

**执行记录：**

- 配置精简与七组格式迁移已完成。维护者进一步确认删除 EditorConfig，保留 ESLint/Oxlint、Prettier ignore、Rust toolchain 与全部 Zed tasks；没有修改全局设置。
- 格式配置实际解析为 package.json 的 tabWidth=4；生成目录仍由根 gitignore 忽略。迁移前后 40 个文件标准化内容一致，10 个 JSON/YAML 数据一致；7 个 Zed tasks 与 package.json 原有命令/依赖不变。两个锁文件与两个全局 Zed 文件的 SHA-256 摘要不变。
- 最终 pnpm frozen-lockfile 安装、完整前端检查通过：48 条前端测试、8 条发布测试、格式检查、ESLint/Oxlint 零警告、双 tsconfig 与生产构建。Rust fmt、Clippy 全 target 零警告与测试通过（26 条通过、1 条真实凭据 smoke 忽略）。
- 本机 macOS no-bundle 桌面构建与 git diff --check 通过。未运行 Windows/Linux 原生验证、远端 CI 或本次 UI 端到端验收；未访问真实凭据或用户 Agent 配置，未创建 Git 提交。
- I01 完成，下一项回到 P08.d；P08.d/P08.e 及整个 P08 未提前完成。

### 插队任务 I02：固定桌面壳与无独立标题栏设计

维护者指出当前页面偏离私人草图，要求先固定桌面设计再继续功能。已确认 macOS
保留原生红黄绿、融入侧栏顶部，并默认跟随系统外观。随后补充确认：侧栏可收起为
88 px 图标栏，右侧无独立标题栏，Settings 外观用一个图标按钮轮换模式。设计基准见
[desktop-shell-design.md](desktop-shell-design.md)；预览不代表生产功能已实现。

#### I02.a：草图审查与可查看的设计基准

- [x] 对照草图固定左侧导航、上下 Provider/Agent、两侧 Skills/MCP、中央配置关系与下方统计预留；语言入口改为 Settings。
- [x] 制作浅/深色预览，检查 720×560、固定侧栏、主区滚动、导航/Settings 与键盘。
- [x] 根据补充要求更新预览：188/88 px 展开/收起，窗控位置不变，移除右侧标题栏；Settings 图标按钮轮换跟随系统/浅色/深色，补齐键盘与可访问提示。
- [x] 维护者确认修订后的视觉基准后，再开始生产壳替换，不以文档批准代替视觉确认。

**Files:** `docs/plans/desktop-shell-design.md`、`docs/plans/desktop-shell-preview.html`、`docs/plans/development-plan.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** ego-browser 浅/深色截图与导航、键盘、溢出检查；格式与差异检查。
**Dependencies:** 草图、维护者窗口控件/主题决策。

#### I02.b：生产桌面壳与设置入口

- [ ] 开始前按实际边界拆分每组最多五个文件的子任务与测试；移除宣传式首页，而不是仅调整旧页面颜色。
- [ ] 实现可折叠侧栏（188/88 px）与主区独立滚动，不设右侧标题栏；关系图/预留统计和未实现页明确标注，不能伪造连接或启用。
- [ ] Startup 只负责偏好/译器门禁；语言选择器移入 Settings，保留保存、重读、输入与 HTML 语言行为。外观使用图标按钮按 system/light/dark 轮换，当前/下一模式提示与键盘操作齐全。新增屏幕同时补齐两种翻译。

**Verification:** 实际 UI 行为测试、完整前端检查、720×560 双语/长内容/键盘验收。
**Dependencies:** I02.a 的视觉确认。

维护者已选择暂不引入 Tailwind，并确认本轮先做界面、后加记忆：外观与导航折叠只在
当前窗口内保留，重启恢复 system/展开；语言偏好仍使用 P08 Rust 持久化。
本轮测试沿用已批准的可见 UI/desktop 边界：真实 React/译器/组件，只替换原生 IPC；
捕获折叠/导航、外观循环、跨页面保存重读与状态保留，不把 DOM 测试当作原生验收。

##### I02.b.0：共用装饰图标

- [x] 为导航与外观两个真实使用者提供小型 SVG 图标组件，不引入图标库或 UI 框架。

**Files:** `src/components/Icon.tsx`。

##### I02.b.1：外观控制与双语资源

- [x] 图标按钮循环 system/light/dark，名称/提示包含当前与下一模式；独立于页面可见性，不使用浏览器存储。
- [x] 补齐桌面壳/关系首页/设置的双语资源；保留 P08 文案与翻译校验。

**Files:** `src/features/settings/AppearanceControl.tsx`、`src/features/settings/AppearanceControl.module.css`、`src/features/settings/AppearanceControl.test.tsx`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** 外观行为/清理/语言切换测试、完整前端检查。

##### I02.b.2：启动门禁与布局解耦

- [ ] Startup 提供已校验的初始偏好与系统语言，由调用方组合设置入口，不再在 App 顶部插入选择器。
- [ ] 保留启动等待、失败重试、StrictMode 过期响应、HTML lang 与真实输入保留测试。

**Files:** `src/features/settings/LocaleStartup.tsx`、`src/features/settings/LocaleStartup.module.css`、`src/features/settings/LocaleStartup.test.tsx`、`src/main.tsx`。
**Verification:** P08 启动回归测试；main 同步更新函数式 children 契约，最终 Settings 组合属于 b.4。

##### I02.b.3：诚实的关系首页

- [ ] 按草图排布 Provider/Agent/Skills/MCP，中性计划节点与虚线关系；下方明确统计未实现。

**Files:** `src/features/overview/RelationshipOverview.tsx`、`src/features/overview/RelationshipOverview.module.css`。
**Verification:** 在完整壳中检查双语、长名称与最小尺寸，不增加静态标签快照测试。

##### I02.b.4：实际桌面壳与入口组合

- [ ] 实现 188/88 px 导航、内容区独立滚动、无右侧标题栏；品牌返回概览，业务页明确未实现。
- [ ] Settings 保持挂载，用 hidden 控制可见性，避免导航丢失未知保存结果/输入；外观同样保留。
- [ ] main 组合真实语言选择器；桌面 metadata 按状态在渲染时翻译，不保存过时翻译字符串。

**Files:** `src/App.tsx`、`src/App.module.css`、`src/App.css`、`src/App.test.tsx`、`src/main.tsx`。
**Verification:** 壳行为与真实 startup/selector 组合回归、完整前端/Rust 检查、本机构建/运行、ego-browser 双语和尺寸检查。

##### I02.b.5：记录实现与验收范围

- [x] 更新结构与设计状态，记录真实结果；不宣称未验证的原生窗控行为已完成。

**Files:** `docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`docs/plans/development-plan.md`、`CHANGELOG.md`；本清单同步更新。

**执行记录（2026-10-09）：** b.0–b.1 已勾选。b.2–b.4 的代码已在工作区（未提交）：启动门禁以函数式 children 传出偏好与系统语言；`App.tsx` 组合 188/88 px 壳、关系首页、设置与外观；`main.tsx` 组合真实语言选择器。自动化测试覆盖折叠、导航保留、设置输入与双语切换，`pnpm run check:frontend` 通过（格式、ESLint/Oxlint 零警告、67 条 UI 测试、8 条发布测试、双 tsconfig、构建）。ego-browser 的双语与 720×560 验收尚未记录，因此 b.2–b.4 暂不勾选；b.5 待补。

#### I02.c：平台窗口壳

- [ ] macOS 使用 Tauri 2 Overlay/hiddenTitle 与原生红黄绿位置配置；Windows/Linux 单独设计边框与自绘控件。
- [ ] 拖动、关闭、最小化、缩放/最大化或全屏符合平台行为；顶部拖动区不画独立标题栏，不覆盖交互区；加载/失败门禁下保留窗控，侧栏折叠不改变原生红黄绿坐标。
- [ ] 实现前拆分配置、IPC 封装、组件/双语资源/权限与测试文件；不增加无关插件或宽泛权限。

**Verification:** Rust/前端检查、实际 Tauri 构建与窗控运行、截图；本机验收不能代表其他平台。
**Dependencies:** I02.b；当前 Tauri 2 官方文档与安装版本核对。

**执行记录（2026-10-09）：** 原生检查（macOS arm64、发布构建、隔离 HOME）确认当前问题：系统标题栏仍占整行，侧栏 44 px 留白位于其下，红黄绿不在侧栏顶部；内容页在 zh-CN 下可渲染。窗口配置（`tauri.conf.json`）尚未修改，两处 diff 仅为缩进。上方验收条件由下列子任务分别覆盖，按依赖顺序实施，每项单独验证后再勾选。

- [ ] **I02.c.1：应用壳拖动区与权限**：新增共享组件 `WindowDragRegion`；侧栏 44 px 留白与主区顶部 28 px 无绘制条作为拖动区；控件不放入拖动区；`capabilities/default.json` 仅增加 `core:window:allow-start-dragging`。
    - Files：`src/components/WindowDragRegion.tsx`、`src/App.tsx`、`src/App.module.css`、`src/App.test.tsx`、`src-tauri/capabilities/default.json`。
    - Verification：新增“拖动区不包含按钮/输入等交互控件”的行为测试；`pnpm run check:frontend`；原生运行截图；拖动是否生效如实记录。
- [ ] **I02.c.2：macOS 覆盖式标题栏**：`tauri.conf.json` 设置 `titleBarStyle: Overlay`、`hiddenTitle: true` 与 `trafficLightPosition`，保留 `decorations: true`，以截图校准坐标。
    - Files：`src-tauri/tauri.conf.json`。
    - Verification：原生截图确认整行系统标题栏消失、红黄绿位于侧栏顶部；展开/折叠前后坐标不变；Windows/Linux 仍为原生装饰，留给 I02.c.5。
- [ ] **I02.c.3：加载与失败门禁的拖动区**：`LocaleStartup` 的门禁提供顶部拖动条，重试按钮位于拖动区之外，门禁中仍可拖动。
    - Files：`src/features/settings/LocaleStartup.tsx`、`LocaleStartup.module.css`、`LocaleStartup.test.tsx`。
    - Verification：门禁行为测试（重试仍可用）、`pnpm run check:frontend`、原生门禁截图（若能稳定触发）。
- [x] **I02.c.4：记录实现与验收范围**：更新开发说明、设计实现状态、英文 CHANGELOG 与本清单，写明未验证项。
    - Files：`docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`CHANGELOG.md`、`docs/plans/todo.md`。
- [ ] **I02.c.5：Windows/Linux 边框与自绘控件**：等待维护者确认方案（无边框自绘或保留原生）后再拆分；决定前不声称跨平台窗控完成。

**I02.c.1/c.2 进度（2026-10-09，未提交）：** c.1 的代码、拖动区不变量测试（变异验证：拖动属性放到侧栏后失败，还原后通过）与完整前端检查（68 条 UI 测试）已通过。c.2 已写入 `titleBarStyle: Overlay`、`hiddenTitle: true`、`trafficLightPosition {16, 22}`。macOS 发布构建截图确认整行系统标题栏消失，红黄绿位于侧栏顶部留白内。合成鼠标事件在侧栏顶部拖动后窗口确实移动；主区顶部条的合成拖动没有移动窗口。合成事件结束得早于异步的 `start_dragging`，这个结果不能算失败证据，需要人工拖动确认。折叠后坐标、双击缩放、全屏与门禁状态尚未验证，因此两项均不勾选。

**维护者调整（2026-10-09，未提交）：** 侧栏去掉运行状态文本；品牌为应用图标加 `vibemate` 文字（折叠时只显示图标，按钮名称以品牌名开头；展开时的 `vibemate` 为自绘单线条 SVG 字标 `BrandWordmark`，不依赖字体文件；曾试用的 `@fontsource/jost` 已移除）（直接引用 `src-tauri/icons/128x128@2x.png`，目前仍是 Tauri 模板图标）；新增“概览”导航，与点击品牌一样回到关系首页；折叠按钮改为仅图标，最终放在“设置”同一行最右端，折叠后叠在设置上方居中（试过红绿灯所在行后由维护者改回）；`trafficLightPosition` 调为 y=24，与 44 px 顶部行中心对齐，主区顶部拖动条为 44 px；设置中的语言下拉框改为 44 px 自绘外观。

#### I02.d：设置中的「关于」（计划，未开始）

- [ ] 在 Settings 增加「关于」：展示品牌、版本、GitHub 仓库等信息；可复用现有 `get_app_info` 命令与 `getAppInfo` 包装（当前未被调用）。实施前拆分文件并补齐中英文。

- 新增共享设计基准与可导航的静态预览，全部未实现节点保持中性/计划中，统计与窗控明确为预留/示意。语言预览不能保存，主题切换只影响该页面，不访问网络、凭据或浏览器存储。
- ego-browser 同一 TaskSpace、720×560 下检查浅/深色：页面无水平或整体垂直溢出，侧栏宽 188 px、Settings 底部 548 px，主区独立滚动。初次检查发现 Grid 默认最小尺寸将底部推出窗口；已修复轨道与 min-height，复查通过。
- 导航与 Settings 可切换；Tab 跳过禁用语言控件并进入外观选择器，focus-visible 轮廓为 solid。预览颜色分别解析为浅色 rgb(247,248,247) 与深色 rgb(25,29,26)。系统跟随补充检查也已通过：保持 system 选择，通过仅该页的 prefers-color-scheme 覆盖切换系统浅/深色，实际背景随之改变。1080×760 下同样无整体溢出，Settings 底部 748 px。
- ego-browser 的 Page.captureScreenshot 超时，尚未取得截图，不声称已完成视觉审查。DOM/尺寸检查不代替维护者看图确认；预览待维护者打开后反馈，生产桌面壳、原生窗控与跨平台行为都尚未实施。
- Tab/Enter 从品牌入口进入 Provider 计划页，当前导航标记和主区标题随之更新。完整前端检查通过（59 条 UI 测试、8 条发布测试、格式、lint、类型与生产构建），git diff --check 通过。本次只改设计文档/预览，没有修改生产源码、窗口配置或权限。预览页为便于维护者查看而保留；本地预览服务仅监听 127.0.0.1:1422，确认后停止。
- 初稿获得暂定确认后，维护者补充导航折叠、去掉右侧标题栏与图标外观轮换要求，已更新同一预览和设计基准。720×560 实测侧栏 188/88 px，无整页溢出；三个窗控示意坐标在折叠前后始终为 (16,16)、(36,16)、(56,16)，右侧标题栏已移除。
- 收起后各导航按钮仍有明确可访问名称，Settings 可进入。外观按钮经 Enter/Space/Enter 完成 system→light→dark→system，图标、当前/下一模式名称、状态文字与背景同步变化；焦点始终停留在按钮，focus-visible 为 solid。实际原生窗控位置仍须生产 Tauri 实施后验证，不能由示意坐标检查代替。
- 维护者确认修订版“差不多可以”，本版作为当前设计基准，I02.a 完成。截图工具限制不变，不声称助手已完成截图视觉审查；维护者的确认也不代替原生运行验收。预览文件保留，临时本地服务关闭。下一项 I02.b；生产壳与原生窗控未提前勾选完成。

### Task P09: 让现有界面支持中英文

**Description:** 把现有 App 的文本与 metadata 状态接入 i18n，并将翻译一致性和切换回归检查纳入完成标准。

**Acceptance criteria:**

- [ ] 现有导航/标题/状态/提示、可访问名称均有中英文；语言偏好入口可用，语言切换保留页面状态。
- [ ] Rust 后续返回稳定错误 code + 安全参数，由前端翻译；当前已有 metadata 错误按边界处理，不向用户直接显示未知原始错误或翻译 key。
- [ ] 自动检查缺失 key、空翻译和插值参数不一致，接入 `check:frontend`；后续业务 UI 必须随功能同时补齐中英文。

**Verification:**

- [ ] 运行 `pnpm run test:ui src/App.test.tsx`，覆盖双语、切换、回退、启动偏好及错误状态。
- [ ] 运行 `check:frontend`；按需把资源校验/脚本/CI接线拆成 P09 子任务后验证。
- [ ] ego-browser 和 Tauri 检查两种语言、720×560、长英文/中文、日期/数字及可访问名称；确认不会改变模型 ID 或配置内容。

**Dependencies:** P04,P07,P08。

**Files likely touched:**

- `src/main.tsx`
- `src/App.tsx`
- `src/App.test.tsx`
- `src/locales/zh-CN.json`
- `src/locales/en.json`

**Estimated scope:** M：5 个建议主文件；额外校验脚本、包命令和注册接线先拆子任务。

**执行记录：** 尚未实施。

### Checkpoint C09: 中英文基础可用于业务页面（P07–P09）

- [ ] 双语文本、语言偏好、缺失翻译和插值检查有实际证据，`check:frontend` 已包含相关检查。
- [ ] 真实 Tauri 重启保持偏好，切换不丢失输入；浏览器预览与真实持久化区分明确。
- [ ] 本组结果已报告维护者，审阅意见已记录；后续新增界面同步补齐中英文。

## 阶段 C：Provider 与 Model

### Task P10: 保存一个 Provider 配置

**Description:** 让用户从表单保存非敏感账户设置，重启后仍可查看，并为同一服务商创建多个实例。

**Acceptance criteria:**

- [ ] 校验名称、URL、协议和扩展字段，持久化稳定 ID；错误通过类型化 IPC 返回。
- [ ] 列表使用有界分页和稳定排序，编辑时保持实例身份，不按显示名称关联。
- [ ] 提供加载、空、错误与保存反馈，浏览器模式不伪造保存成功。

**Verification:**

- [ ] 运行 Rust `providers` 过滤测试与 ProviderForm UI 测试，覆盖非法 URL 和持久化重读。
- [ ] 运行 `pnpm run check:frontend` 与 Rust fmt/Clippy。
- [ ] 实际 Tauri 保存非敏感配置，重启验证；需要额外注册/样式文件时先拆子任务。

**Dependencies:** P04,P05,P09。

**Files likely touched:**

- `src-tauri/src/providers.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P11: 维护 Provider 密钥

**Description:** 让用户保存、替换、清除账户密钥，同时只显示凭据存在状态。

**Acceptance criteria:**

- [ ] UI 临时输入密钥，提交/取消后清理；配置、错误、日志与列表不回传密钥。
- [ ] 数据库保存引用；凭据/元数据任一步失败按约定补偿并报告结果。
- [ ] 清除密钥使账户进入明确的待配置状态，不能继续使用旧密钥。

**Verification:**

- [ ] 运行 Rust `credentials`/`providers` 测试，注入两种资源的保存失败。
- [ ] 运行 UI 测试与 `check:frontend`，检查敏感字段清理。
- [ ] Tauri 使用临时凭据完成替换/删除，确认日志脱敏。

**Dependencies:** P06,P10。

**Files likely touched:**

- `src-tauri/src/providers.rs`
- `src-tauri/src/credentials.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P12: 维护手动 Model 配置

**Description:** 让用户在某个 Provider 实例下建立模型并配置基础能力信息和请求参数。

**Acceptance criteria:**

- [ ] 按 Provider ID + 模型 ID 关联，支持别名，能力与请求参数分开存储。
- [ ] 校验上下文/输出上限等基础值；未知能力和手动覆盖有来源标识。
- [ ] 支持保存、编辑、选择模型，列表分页；删除正在被引用的模型需明确处理。

**Verification:**

- [ ] 运行 Rust `models` 与 ModelForm 测试，覆盖无效参数和跨 Provider 同名模型。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 重启后验证选择与保存结果。

**Dependencies:** P10。

**Files likely touched:**

- `src-tauri/src/models.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/models.ts`
- `src/features/models/ModelForm.tsx`
- `src/features/models/ModelForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C12: 账户和模型可保存（P10–P12）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

### Task P13: 验证 DeepSeek 连接

**Description:** 实现已核实的 DeepSeek 协议适配，并从 UI 明确触发最小连接检查。

**Acceptance criteria:**

- [ ] 使用文档确认的认证和请求结构，显示连通性与能力限制，不假定所有模型相同。
- [ ] 网络请求有超时与取消/失效结果处理；认证失败、限流与协议错误脱敏。
- [ ] 默认用模拟 HTTP 服务测试；真实请求只在用户触发时发送，并提示可能计费。

**Verification:**

- [ ] 运行 Rust `deepseek` 模拟服务测试与 ProviderForm 状态测试。
- [ ] 运行 `check:frontend`、Rust fmt/Clippy；新增网络依赖若扩大范围先拆分。
- [ ] 提供测试凭据时才做真实连接验证并记录结果。

**Dependencies:** P01,P11,P12。

**Files likely touched:**

- `src-tauri/src/providers/deepseek.rs`
- `src-tauri/src/providers.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P14: 接入 OpenRouter 模型发现

**Description:** 增加 OpenRouter 模板和模型发现，把返回信息用于已有 Model 管理流程。

**Acceptance criteria:**

- [ ] 按真实协议连通并查询模型，映射支持的能力/参数字段，不复制 DeepSeek 假设。
- [ ] 能选择并保存返回模型，保留手动覆盖；无列表、失败或字段缺失时可手动配置。
- [ ] 覆盖分页/大列表、超时、限流与缓存来源时间，错误不含认证值。

**Verification:**

- [ ] 运行 Rust `openrouter`/`models` 模拟测试和 ModelForm 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 有测试凭据时检查实际模型字段来源；更新 integrations 证据记录。

**Dependencies:** P01,P13。

**Files likely touched:**

- `src-tauri/src/providers/openrouter.rs`
- `src-tauri/src/models.rs`
- `src/lib/desktop/models.ts`
- `src/features/models/ModelForm.tsx`
- `src/features/models/ModelForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P15: 接入 Command Code GOAT

**Description:** 使用 P01 的准确接入约定新增服务商适配，复用账户与 Model 页面。

**Acceptance criteria:**

- [ ] 按照其真实认证、协议和扩展字段保存与验证，不凭名称猜 URL。
- [ ] 有模型发现接口就映射；没有则支持手动模型且清楚说明能力信息来源。
- [ ] 未核实的特有行为保持不可用，不写死为 OpenAI 兼容或伪造成功。

**Verification:**

- [ ] 运行 Rust `command_code_goat` 模拟服务测试与表单行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实连接依赖准确文档和明确提供的测试凭据，未满足时保留待验状态。

**Dependencies:** P01,P13。

**Files likely touched:**

- `src-tauri/src/providers/command_code_goat.rs`
- `src-tauri/src/providers.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C15: 三家 Provider 接入（P13–P15）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

### Task P16: 校验 Provider 特有参数

**Description:** 补充模型字段表单与注入前校验，确保不同 Provider 的推理/模态设置没有错误等价映射。

**Acceptance criteria:**

- [ ] 根据协议声明允许的字段类型、枚举和范围，区分默认、继承与显式覆盖。
- [ ] 保留可验证的自定义请求头/参数；拒绝危险认证覆盖、未知注入字段和不支持的值。
- [ ] UI 显示能力信息来源与具体校验原因，不因手动标签就宣称实际支持。

**Verification:**

- [ ] 运行 Rust `model_parameters` 测试，覆盖 effort/预算、图像、工具与输出限制的差异。
- [ ] 运行 ModelForm 测试、`check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P12,P13,P14,P15。

**Files likely touched:**

- `src-tauri/src/models.rs`
- `src-tauri/src/providers.rs`
- `src/lib/desktop/models.ts`
- `src/features/models/ModelForm.tsx`
- `src/features/models/ModelForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

## 阶段 D：Pi 配置

### Task P17: 展示本机 Agent 状态

**Description:** 提供只读发现与安装路径手动选择，让用户看见两个 Agent 的配置能力与覆盖情况。

**Acceptance criteria:**

- [ ] 根据已核实的来源/路径/版本发现 Pi 和 Grok Build，未安装时允许选择已确认的路径。
- [ ] 显示版本、能力和配置优先级；发现不运行不可信二进制或输出敏感配置。
- [ ] 未知版本和 unsupported 明确区分，不修改任何 Agent 文件。

**Verification:**

- [ ] 运行 Rust `agent_discovery` 临时目录/平台样本测试及 AgentList 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 在可用本机安装中只读核对发现结果。

**Dependencies:** P02,P03,P10。

**Files likely touched:**

- `src-tauri/src/agents.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/AgentList.tsx`
- `src/features/agents/AgentList.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P18: 预览 Pi 模型配置

**Description:** 将选择的 Provider/Model 编译为 Pi 配置计划，展示脱敏差异，不写文件。

**Acceptance criteria:**

- [ ] 按 P02 约定映射原生字段，保留无关配置；不支持的组合拒绝并说明。
- [ ] 展示变更文件、覆盖来源、重启要求、密钥写入方式与潜在明文存放。
- [ ] 计划绑定原文件摘要和账户/模型 revision；预览不读取后回传明文密钥。

**Verification:**

- [ ] 运行 Rust `pi_plan` 测试，覆盖格式保存、优先级、不兼容字段与脱敏。
- [ ] 运行 ConfigPreview 测试、`check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 预览临时 Pi 样本，确认未发生写入。

**Dependencies:** P16,P17。

**Files likely touched:**

- `src-tauri/src/agents/pi.rs`
- `src-tauri/src/config_plan.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C18: 参数映射与 Pi 预览（P16–P18）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

### Task P19: 建立可恢复配置写入

**Description:** 实现文件写入安全机制，供 Pi 应用与其他注入复用，不扩展为通用文件管理器。

**Acceptance criteria:**

- [ ] 应用前检查摘要/权限/目标身份，同目录临时写入后原子替换，私有备份可定位。
- [ ] 多文件操作记录步骤，部分失败可补偿；重启能发现未完成操作。
- [ ] 备份访问受限，敏感内容不进入差异/错误/普通导出；不静默跟随意外 symlink。

**Verification:**

- [ ] 运行 Rust `config_apply` 故障注入测试，覆盖并发、替换失败、权限和部分提交。
- [ ] 运行 Rust fmt/Clippy；阶段检查点执行原生构建。

**Dependencies:** P05,P06,P18。

**Files likely touched:**

- `src-tauri/src/config_apply.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/credentials.rs`
- `src-tauri/src/config_apply_tests.rs`

**Estimated scope:** M：4 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P20: 应用 Pi 模型配置

**Description:** 让用户在确认预览后应用选择的模型与认证配置，满足原有启动方式。

**Acceptance criteria:**

- [ ] 只应用有效且未过期的预览计划，遇到文件/模型 revision 变化要求重新预览。
- [ ] 通过 Rust 按原生方式注入凭据，UI 明示目标明文限制；不隐式依赖 vibemate 启动器。
- [ ] 写入后重读验证，显示成功/部分失败/待重启；连续提交不会重复破坏配置。

**Verification:**

- [ ] 运行 Rust `pi_apply`/`config_apply` 测试和 UI 重复提交/失败测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 使用可恢复的测试配置，在真实 Tauri/Pi 中确认原生启动与模型选择。

**Dependencies:** P11,P18,P19。

**Files likely touched:**

- `src-tauri/src/agents/pi.rs`
- `src-tauri/src/config_apply.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P21: 回滚 Pi 配置

**Description:** 提供历史操作结果和恢复预览，将误操作恢复为备份版本。

**Acceptance criteria:**

- [ ] 可查看脱敏操作状态并预览恢复；受限备份内容不直接返回前端。
- [ ] 恢复前检查用户后续修改，冲突时拒绝覆盖并给出处理步骤。
- [ ] 恢复配置和必要凭据关系，失败可继续恢复；清理失败不隐瞒残留状态。

**Verification:**

- [ ] 运行 Rust `config_restore` 故障测试与 RestoreView 行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实 Tauri 使用临时配置完成应用→恢复→原生启动检查。

**Dependencies:** P20。

**Files likely touched:**

- `src-tauri/src/config_apply.rs`
- `src-tauri/src/agents/pi.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/RestoreView.tsx`
- `src/features/agents/RestoreView.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C21: Pi 应用与恢复（P19–P21）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

## 阶段 E：Grok Build

### Task P22: 预览 Grok Build 配置

**Description:** 接入第二个 Agent 的原生映射，复用配置差异组件。

**Acceptance criteria:**

- [ ] 按 P03 的准确版本与格式生成计划，保留其他字段并执行协议/参数校验。
- [ ] 展示覆盖来源、密钥存放、生效/重启方式；未知版本或缺少接口明确报错。
- [ ] 复用计划/脱敏机制而不复制 Pi 配置假设；预览不写文件。

**Verification:**

- [ ] 运行 Rust `grok_plan` 样本测试与已有预览行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 本机版本可用时对照实际配置验证；资料缺失保持任务未完成。

**Dependencies:** P03,P18,P21。

**Files likely touched:**

- `src-tauri/src/agents/grok_build.rs`
- `src-tauri/src/agents.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P23: 应用 Grok Build 配置

**Description:** 让第二个 Agent 使用已有可靠写入与回滚机制，验证原有启动方式。

**Acceptance criteria:**

- [ ] 有效计划能写入并重读，处理并发变化、认证方式和配置优先级。
- [ ] 可以应用和恢复，异常沿用明确的操作状态与恢复路径。
- [ ] 真实启动证据支持该版本/Provider 组合；需启动器/桥接时先决定范围。

**Verification:**

- [ ] 运行 Rust `grok_apply`/`config_restore` 测试与 UI 行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 在真实 Tauri/Grok Build 中应用、重启、恢复，记录安装版本。

**Dependencies:** P19,P21,P22。

**Files likely touched:**

- `src-tauri/src/agents/grok_build.rs`
- `src-tauri/src/config_apply.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P24: 验证 Provider 与 Agent 组合

**Description:** 检查三家 Provider × 两个 Agent 的实际兼容矩阵与切换行为，不承诺不存在的协议转换。

**Acceptance criteria:**

- [ ] 每个组合有 supported/unsupported/unverified 状态及原因、版本与证据。
- [ ] 切换账户或模型保持稳定绑定，并正确提示项目/环境变量覆盖。
- [ ] 回归工具调用、推理/图像参数等已支持能力，未支持项不被静默丢弃。

**Verification:**

- [ ] 运行 Rust `compatibility` 映射样本测试与 UI 切换测试。
- [ ] 运行完整项目检查；真实用例仅使用许可的测试凭据。

**Dependencies:** P13,P14,P15,P16,P20,P23。

**Files likely touched:**

- `src-tauri/src/agents/compatibility_tests.rs`
- `src-tauri/src/agents.rs`
- `src/features/agents/AgentList.test.tsx`
- `docs/integrations/compatibility.md`

**Estimated scope:** M：4 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C24: 两个 Agent 的配置兼容性（P22–P24）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

## 阶段 F：Skill

### Task P25: 登记中央 Skill 来源

**Description:** 让用户导入已有本地 Skill 并维护准确来源，支持未知来源的明确状态。

**Acceptance criteria:**

- [ ] 中央库记录稳定 ID、内容摘要、来源 URL/revision 和本地修改；原目录不被移动或删除。
- [ ] 兼容本地已安装的 GitHub/npx Skill，格式和目录合法性按 Agent 约定验证。
- [ ] 不能自动识别的来源可补充，更新能力明确；列表有界分页。

**Verification:**

- [ ] 运行 Rust `skill_import` 测试，覆盖路径、未知来源、同名和解析失败。
- [ ] 运行 SkillLibrary 测试、`check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 导入临时 Skill，确认原目录不变。

**Dependencies:** P05,P17。

**Files likely touched:**

- `src-tauri/src/skills.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillLibrary.tsx`
- `src/features/skills/SkillLibrary.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P26: 从已确认来源安装 Skill

**Description:** 在隔离目录获取 GitHub 或经核实的安装器结果，再加入中央 Skill 库。

**Acceptance criteria:**

- [ ] Git 来源固定 revision；npx 来源记录准确安装器/版本/命令和退出状态，执行前显示命令。
- [ ] 不执行 Skill 内容中的命令；校验路径遍历、失效来源和结果格式。
- [ ] 失败不破坏已有库；成功记录来源及可更新方式，不声称未知来源可更新。

**Verification:**

- [ ] 运行 Rust `skill_install` 模拟源/安装器测试与安装状态 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 使用已确认测试来源检查隔离安装和失败清理，网络/命令依赖拆成子任务。

**Dependencies:** P25。

**Files likely touched:**

- `src-tauri/src/skills/install.rs`
- `src-tauri/src/skills.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillLibrary.tsx`
- `src/features/skills/SkillLibrary.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P27: 向 Pi 部署 Skill

**Description:** 将中央 Skill 通过链接或明确的复制同步部署到 Pi 支持的目录。

**Acceptance criteria:**

- [ ] 预览新增/冲突与目标路径，symlink 优先；不可用时让用户选择 copy-sync。
- [ ] 保留同名用户内容，记录所有权；禁用只撤销自己管理的目标。
- [ ] 报告目标已同步的 revision，失败可恢复，不把部分部署显示为整体成功。

**Verification:**

- [ ] 运行 Rust `skill_deploy_pi` 临时目录/链接/复制测试与部署 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 本机 Pi 能发现测试 Skill，确认卸载不损害已有内容。

**Dependencies:** P02,P19,P25。

**Files likely touched:**

- `src-tauri/src/skills/deploy.rs`
- `src-tauri/src/agents/pi.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillDeployment.tsx`
- `src/features/skills/SkillDeployment.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C27: 中央 Skill 与 Pi 部署（P25–P27）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

### Task P28: 向 Grok Build 部署 Skill

**Description:** 按照已核实的第二个 Agent 目录与格式部署，复用所有权和恢复机制。

**Acceptance criteria:**

- [ ] 仅对 P03 已确认支持的格式部署，未知能力明确为待决，不创建无效目录。
- [ ] 正确处理目标冲突、symlink 权限与 copy-sync，并记录部署 revision。
- [ ] 撤销、重试和部分失败不会影响其他 Agent 或用户 Skill。

**Verification:**

- [ ] 运行 Rust `skill_deploy_grok` 与部署 UI 回归测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实 Agent 支持时检查可发现性；缺口由维护者决定，任务不假装完成。

**Dependencies:** P03,P27。

**Files likely touched:**

- `src-tauri/src/skills/deploy.rs`
- `src-tauri/src/agents/grok_build.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillDeployment.tsx`
- `src/features/skills/SkillDeployment.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P29: 预览 Skill 更新

**Description:** 获取新版本并展示上游变化、本地修改和目标部署状态，不直接覆盖。

**Acceptance criteria:**

- [ ] 只对来源明确的 Skill 检查更新，Git/npx 使用已记录的获取方式和版本。
- [ ] 比较原 revision、中央本地修改与候选内容，显示差异与可选处理策略。
- [ ] 更新预览不写中央或 Agent 目标；超时/失效/版本变化有明确错误。

**Verification:**

- [ ] 运行 Rust `skill_update_plan` 三方内容样本测试与更新预览 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P26,P27,P28。

**Files likely touched:**

- `src-tauri/src/skills/update.rs`
- `src-tauri/src/skills.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillUpdate.tsx`
- `src/features/skills/SkillUpdate.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P30: 应用 Skill 更新

**Description:** 确认更新计划后替换中央版本并刷新其受管部署，保留恢复能力。

**Acceptance criteria:**

- [ ] 应用前重新检查内容摘要，不能覆盖未处理的本地修改或过期计划。
- [ ] 中央与链接/复制目标按操作日志更新，显示逐目标状态；部分失败可恢复。
- [ ] 失败和回滚保留来源/revision/所有权一致性，不删除其他 Agent 内容。

**Verification:**

- [ ] 运行 Rust `skill_update_apply` 故障注入测试与提交/恢复 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 测试 Git/npx 样本和一次 copy-sync 部分失败恢复。

**Dependencies:** P19,P29。

**Files likely touched:**

- `src-tauri/src/skills/update.rs`
- `src-tauri/src/skills/deploy.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillUpdate.tsx`
- `src/features/skills/SkillUpdate.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C30: Skill 跨 Agent 更新（P28–P30）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

## 阶段 G：MCP

### Task P31: 维护中央 MCP 定义

**Description:** 提供服务器配置表单，保存结构化定义与凭据引用，但不启动服务器。

**Acceptance criteria:**

- [ ] 支持经过核实的 stdio/HTTP 字段，校验命令参数、URL、env/header 和传输类型。
- [ ] 敏感字段使用系统凭据引用，普通列表/差异/错误不暴露值；查询有界分页。
- [ ] 保存、编辑、禁用可重读，预览与导入不启动进程或发送认证请求。

**Verification:**

- [ ] 运行 Rust `mcp_definition` 字段/敏感值测试与 McpForm 行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 保存包含测试命令的定义，确认没有进程启动。

**Dependencies:** P05,P06,P17。

**Files likely touched:**

- `src-tauri/src/mcp.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/mcp.ts`
- `src/features/mcp/McpForm.tsx`
- `src/features/mcp/McpForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P32: 向 Pi 注入 MCP 配置

**Description:** 基于 P02 支持矩阵，将 MCP 定义映射为 Pi 接受的配置或显式记录缺口。

**Acceptance criteria:**

- [ ] 原生支持时按实际格式预览并应用，保留无关字段和已有 MCP 条目。
- [ ] 凭据在 Rust 中解析，差异脱敏，写入/撤销/恢复复用安全机制。
- [ ] 无原生支持时先决定桥接/范围；未实现所需能力前任务保持受阻，不以提示替代交付。

**Verification:**

- [ ] 运行 Rust `mcp_pi` 映射/恢复测试及部署 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实 Pi 用无敏感测试服务器确认发现/禁用；无原生支持时记录决策所需证据。

**Dependencies:** P02,P19,P21,P31。

**Files likely touched:**

- `src-tauri/src/agents/pi.rs`
- `src-tauri/src/mcp.rs`
- `src/lib/desktop/mcp.ts`
- `src/features/mcp/McpDeployment.tsx`
- `src/features/mcp/McpDeployment.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P33: 向 Grok Build 注入 MCP 配置

**Description:** 按第二个 Agent 的真实格式实现 MCP 部署与恢复，验证其独立能力。

**Acceptance criteria:**

- [ ] 原生支持时独立映射传输与参数，不能照搬 Pi 配置或丢弃不支持字段。
- [ ] 预览、应用、撤销与恢复保持原配置、所有权和凭据脱敏。
- [ ] 无原生支持时保持待决，不能宣称 MCP 已完成；额外桥接需先拆计划。

**Verification:**

- [ ] 运行 Rust `mcp_grok` 与部署 UI 回归测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实 Grok Build 验证测试服务器可用性与恢复结果。

**Dependencies:** P03,P23,P31。

**Files likely touched:**

- `src-tauri/src/agents/grok_build.rs`
- `src-tauri/src/mcp.rs`
- `src/lib/desktop/mcp.ts`
- `src/features/mcp/McpDeployment.tsx`
- `src/features/mcp/McpDeployment.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C33: MCP 能力与部署（P31–P33）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

## 阶段 H：维护与验收

### Task P34: 恢复未完成操作

**Description:** 在启动时发现上次中断的 Agent 配置或 Skill 部署操作，提供明确的恢复入口，不静默重放敏感操作。

**Acceptance criteria:**

- [ ] 显示未完成操作的脱敏状态、受影响目标与可恢复动作，不把部分成功显示为全部完成。
- [ ] 恢复前核对目标和中央状态变化，冲突时保留当前内容并给出处理步骤；重试具有幂等性。
- [ ] 恢复失败仍可继续处理，日志与引用一致，未知操作版本拒绝自动执行；只操作 vibemate 管理的条目。

**Verification:**

- [ ] 运行 Rust `operation_recovery` 中断/重启/冲突/重复重试测试及 RecoveryView 行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 使用临时配置模拟一次中断，重启后完成恢复并验证目标内容。

**Dependencies:** P21,P23,P30,P33。

**Files likely touched:**

- `src-tauri/src/config_apply.rs`
- `src-tauri/src/skills/deploy.rs`
- `src/lib/desktop/settings.ts`
- `src/features/settings/RecoveryView.tsx`
- `src/features/settings/RecoveryView.test.tsx`

**Estimated scope:** M：5 个建议主文件；若操作日志需迁移或注册接线，先拆子任务。

**执行记录：** 尚未实施。

### Task P35: 约束被引用配置的删除

**Description:** 完善 Provider/Model/Skill/MCP 的删除行为，防止已有 Agent 引用突然失效。

**Acceptance criteria:**

- [ ] 展示被引用关系，禁止静默级联删除或自动重写全部 Agent 配置。
- [ ] 用户选择先解除绑定/撤销受管部署，再按对应机制清理；失败保留可恢复状态。
- [ ] 凭据清理与数据库删除遵守补偿约定；用户文件和共享凭据不误删。

**Verification:**

- [ ] 运行 Rust `config_lifecycle` 引用/清理失败测试和删除确认 UI 测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P21,P23,P30,P33。

**Files likely touched:**

- `src-tauri/src/config_lifecycle.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/settings.ts`
- `src/features/settings/DeletionPreview.tsx`
- `src/features/settings/DeletionPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P36: 检查前端实际可用性

**Description:** 用真实业务页面检查长内容、键盘、分页与错误状态，修复有证据的问题。

**Acceptance criteria:**

- [ ] 中英文在 720×560、缩放和长 URL/模型 ID 下可操作，列表加载/空/失败/分页有明确反馈。
- [ ] 表单与确认界面键盘可达、焦点恢复正确，状态不只靠颜色；符合严格样式规则。
- [ ] 仅修复发现的问题，若跨多个 feature 超过五个文件，先按页面拆子任务。

**Verification:**

- [ ] 运行受影响页面行为测试与 `pnpm run check:frontend`。
- [ ] 用 ego-browser 做最小窗口、键盘、长内容和错误状态检查并记录证据。
- [ ] 用实际 Tauri 确认桌面错误/原生行为没有被浏览器 mock 隐藏。

**Dependencies:** P34,P35。

**Files likely touched:**

- `docs/frontend.md`
- `src/App.css`
- `src/features/<受影响页面>/*.tsx`
- `src/features/<受影响页面>/*.module.css`
- `src/features/<受影响页面>/*.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C36: 恢复、删除与实际界面（P34–P36）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。

### Task P37: 验证三平台系统行为

**Description:** 把文件、凭据与部署测试接入原生 CI，并列出 CI 无法覆盖的人工验证。

**Acceptance criteria:**

- [ ] 原生验证路径/大小写、权限、替换、链接/复制和失败恢复，不用交叉编译冒充平台证据。
- [ ] macOS/Windows/Linux 的凭据不可用路径可测，真实 OS 密钥库的交互验证单独记录。
- [ ] 检查四发布目标的依赖与运行条件，CI 结果和未完成手工项有清楚状态。

**Verification:**

- [ ] 运行完整前端与 Rust 检查及本机原生构建。
- [ ] 推送已授权的远端后核对三个原生 CI 结果；无远端时保持待验证，不标通过。
- [ ] 人工验证各 OS 的权限/密钥库/安装条件，记录版本与缺口。

**Dependencies:** P20,P23,P30,P33,P35。

**Files likely touched:**

- `.github/workflows/ci.yml`
- `src-tauri/src/config_apply_tests.rs`
- `src-tauri/src/skills/deploy_tests.rs`
- `src-tauri/src/credentials.rs`
- `docs/verification/platforms.md`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P38: 完成真实接入验收

**Description:** 使用维护者指定的安装环境与测试凭据，确认自动使用配置、Skill 更新和 MCP 可用性。

**Acceptance criteria:**

- [ ] 三 Provider/两 Agent 每个受支持组合有版本和实际结果，不把 unsupported/unverified 当通过。
- [ ] 验证原生启动、模型参数、回滚、Skill 来源更新与 MCP；记录未实现的桥接缺口。
- [ ] 真实请求在允许范围内，使用临时或可恢复配置，不将密钥或敏感输出放入记录。

**Verification:**

- [ ] 按 documented matrix 人工完成真实 Agent/Provider 验收。
- [ ] 运行完整项目检查；缺凭据/平台或能力时保留未完成项并明确原因。

**Dependencies:** P24,P30,P32,P33,P34,P37。

**Files likely touched:**

- `docs/verification/phase-1.md`
- `docs/integrations/compatibility.md`
- `docs/plans/todo.md`

**Estimated scope:** S：3 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Task P39: 准备首个草稿 Release

**Description:** 按 CHANGELOG 规则整理实际已交付功能，检验安装包草稿流程而不自动发布。

**Acceptance criteria:**

- [ ] 版本、锁文件和 CHANGELOG 条目一致；更新说明不包含未实现功能或未通过的平台承诺。
- [ ] 在维护者授权创建远端/标签后运行四目标草稿构建，检查资产与安装启动结果。
- [ ] 签名/公证未配置时明确记录限制，维护者检查后决定是否发布，不默认启用应用更新。

**Verification:**

- [ ] 运行 `pnpm run check:frontend`、完整 Rust 检查和发布说明预览。
- [ ] 检查四个真实 Actions job 与安装包；没有远端授权时记录待执行，不创建远端或推标签。

**Dependencies:** P36,P37,P38。

**Files likely touched:**

- `CHANGELOG.md`
- `docs/releases.md`
- `docs/verification/release.md`
- `.github/workflows/release.yml`
- `docs/plans/todo.md`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

**执行记录：** 尚未实施。

### Checkpoint C39: 第一阶段交付审阅（P37–P39）

- [ ] 本组任务验收与验证有实际证据；受阻项没有勾选为完成。
- [ ] 运行适用的完整前端/Rust 检查；原生阶段核对实际 Tauri 行为与恢复路径。
- [ ] 本组结果已报告维护者，审阅意见已记录；下一组必需的协议或范围决策已解决。
