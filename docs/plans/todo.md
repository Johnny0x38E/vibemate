# vibemate 第一阶段执行清单

状态：P00–P08、I02（桌面壳、窗控、设置 UX）、I03（自有品牌图标）、I04（主题）、I05（品牌组合）已完成。**未完成**：P09 全站 i18n/资源校验。业务 Provider/Agent 等功能未开始。勾选与文件范围见下文；设计见 [development-plan.md](development-plan.md)、[desktop-shell-design.md](desktop-shell-design.md)。
这里是唯一任务状态来源，不能在其他文件维护第二份勾选清单。

## 执行约定

- 按依赖顺序实施，一次一个可检查的行为；默认不使用子代理。
- 每个任务通常 1–5 个实际文件；建议路径不是强制架构。测试、注册、迁移和锁文件也计数。
  实际范围超过约五个文件或一个专注会话，先拆 Pxx.a/Pxx.b 子任务并补依赖。
- 所有 UI 任务必须同时补齐中英文、可访问名称和错误文案；资源更新也计入文件范围，
  超过约五个文件先拆本任务子项，而不是等功能全部完成后再统一翻译。
- 每项 Verification 加上本阶段适用的 `development-plan.md` 完成标准。
  Rust 过滤测试必须确认执行条数非零；UI 命令在 P04 后才存在。
- 勾选表示验收已通过；验证细节以 Git 提交、CI 与 `CHANGELOG.md` 为准，不在清单里维护长篇执行记录。
  进行中的任务若有关键限制（平台未测、已知缺口），可在该任务下写一两句备注，完成后删除。
- 检查点只确认本阶段任务是否已全部勾选、是否有未决范围问题；不写叙述、不重复各任务的 Verification。

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

### P00.f：修复干净 CI 安装的发布年龄限制

- [x] 使用符合 pnpm 默认 24 小时发布年龄策略的 Tauri API `2.12.1` 与 Vite `8.3.3`，不禁用策略。
- [x] 在隔离 store/cache 中验证冻结安装，重新完成前端与桌面构建。
- [x] 核对实际 GitHub CI 结果，记录补丁版本调整和平台结果。

**Files:** `pnpm-lock.yaml`、`CHANGELOG.md`；本清单记录验证结果。
**Dependencies:** P00.a–P00.e。

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

### Checkpoint C03（P01–P03）

- [x] P01–P03 已勾选；证据见 `docs/integrations/`。

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

### P04.d：Zed 锁文件诊断（已完成，见 `docs/frontend.md`）

- [x] `pnpm-lock.yaml` 在 Zed 中按 Plain Text 打开，避免多文档 YAML 误报。

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

### Checkpoint C06（P04–P06）

- [x] P04–P06 已勾选。
- [ ] 维护者审阅本阶段（可选记录意见）。

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

### I01：四空格与项目配置精简（已完成）

- [x] 适用源码与配置统一 4 空格（`package.json` → `prettier.tabWidth`）；删除冗余 `.editorconfig`、`.prettierrc.json`、`src-tauri/.gitignore`。
- [x] `.zed/settings.json` 仅保留 ESLint、Rust/Clippy 与 `pnpm-lock.yaml` Plain Text 映射；任务与 CI 检查不变。说明见 `docs/frontend.md`「Editing in Zed」。

### 插队任务 I02：固定桌面壳与无独立标题栏设计

维护者要求先固定桌面设计再继续功能：200/88 px 固定侧栏、无右侧标题栏、Settings
常规/关于页签与分组下拉（语言/外观）。macOS 为 Overlay 标题栏与侧栏红绿灯；
Windows/Linux 为无边框与顶栏自绘窗控。设计基准见
[desktop-shell-design.md](desktop-shell-design.md)；静态预览仅作布局参考，部分控件样式未与生产同步。

#### I02.a：草图审查与可查看的设计基准

- [x] 对照草图固定左侧导航、上下 Provider/Agent、两侧 Skills/MCP、中央配置关系与下方统计预留；语言入口改为 Settings。
- [x] 制作浅/深色预览，检查 720×560、固定侧栏、主区滚动、导航/Settings 与键盘。
- [x] 根据补充要求更新预览：收起侧栏、窗控示意位置、移除右侧标题栏（生产壳后为 200/88 px 与下拉设置，见 b.6）。
- [x] 维护者确认修订后的视觉基准后，再开始生产壳替换，不以文档批准代替视觉确认。

**Files:** `docs/plans/desktop-shell-design.md`、`docs/plans/desktop-shell-preview.html`、`docs/plans/development-plan.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** ego-browser 浅/深色截图与导航、键盘、溢出检查；格式与差异检查。
**Dependencies:** 草图、维护者窗口控件/主题决策。

#### I02.b：生产桌面壳与设置入口

- [x] 开始前按实际边界拆分每组最多五个文件的子任务与测试；移除宣传式首页，而不是仅调整旧页面颜色。
- [x] 实现可折叠侧栏（200/88 px）与主区独立滚动，不设右侧标题栏；关系图/预留统计和未实现页明确标注，不能伪造连接或启用。
- [x] Startup 只负责偏好/译器门禁；语言选择器移入 Settings，保留保存、重读、输入与 HTML 语言行为；外观与语言控件见 b.6。

**Verification:** 实际 UI 行为测试、完整前端检查、720×560 双语/长内容/键盘验收。
**Dependencies:** I02.a 的视觉确认。

外观与导航折叠为窗口内状态；语言偏好仍由 P08 Rust 持久化。

##### I02.b.0：共用装饰图标

- [x] 为导航与外观两个真实使用者提供小型 SVG 图标组件，不引入图标库或 UI 框架。

**Files:** `src/components/Icon.tsx`。

##### I02.b.1：外观控制与双语资源（外观 UI 已由 b.6 改为下拉）

- [x] 窗口内 appearance（`data-appearance`）；补齐壳层双语资源。

**Files:** `src/features/settings/AppearanceControl.tsx`、`AppearanceControl.test.tsx`、`src/locales/*.json`。
**Verification:** `pnpm run check:frontend`。

##### I02.b.2：启动门禁与布局解耦

- [x] Startup 提供已校验的初始偏好与系统语言，由调用方组合设置入口，不再在 App 顶部插入选择器。
- [x] 保留启动等待、失败重试、StrictMode 过期响应、HTML lang 与真实输入保留测试。

**Files:** `src/features/settings/LocaleStartup.tsx`、`src/features/settings/LocaleStartup.module.css`、`src/features/settings/LocaleStartup.test.tsx`、`src/main.tsx`。
**Verification:** P08 启动回归测试；main 同步更新函数式 children 契约，最终 Settings 组合属于 b.4。

##### I02.b.3：诚实的关系首页

- [x] 按草图排布 Provider/Agent/Skills/MCP，中性计划节点与虚线关系；下方明确统计未实现。

**Files:** `src/features/overview/RelationshipOverview.tsx`、`src/features/overview/RelationshipOverview.module.css`。
**Verification:** 在完整壳中检查双语、长名称与最小尺寸，不增加静态标签快照测试。

##### I02.b.4：实际桌面壳与入口组合

- [x] 实现 200/88 px 导航、内容区独立滚动、无右侧标题栏；品牌与「概览」返回关系首页，业务页明确未实现。
- [x] Settings 保持挂载，用 hidden 控制可见性，避免导航丢失未知保存结果/输入；外观同样保留。
- [x] main 组合真实语言选择器；壳层文案随译器切换，不再显示桌面 metadata 状态行。

**Files:** `src/App.tsx`、`src/App.module.css`、`src/App.css`、`src/App.test.tsx`、`src/main.tsx`。
**Verification:** `pnpm run check:frontend`、本机 Tauri 壳层检查。

##### I02.b.5：记录实现与验收范围

- [x] 更新 `docs/frontend.md`、设计基准、CHANGELOG 与本清单。

##### I02.b.6：设置常规区 UX

- [x] 侧栏展开宽度改为 **200 px**（折叠仍 **88 px**）；取消 I02.e 侧栏边缘拖动调宽（维护者决定）。
- [x] 设置页顶栏增加 **常规 / 关于** 页签（`SettingsView`）；「关于」暂用与其他计划页一致的 `desktop.plannedTitle/Detail` 占位，不调用 `get_app_info`。
- [x] 「常规」内语言、外观改为与参考应用接近的 **左标签右控件** 行：共享 `settingsField.module.css` 分组卡片、36 px 下拉；去掉「已保存」「当前外观」等即时生效场景下的状态说明（失败/预览/重读仍保留）。
- [x] 亮色主题 `--color-hover` 加深，侧栏行 hover 可辨认；中英文键 `settings.tabs.*`、语言标签改为「语言」/ Language。
- [x] 维护者本机 Tauri 目视确认设置页签与分组列表正常（2026-10-10，无单独截图）。

**Files:** `SettingsView.tsx`、`settingsField.module.css`、`LanguageSelector.tsx`、`AppearanceControl.tsx`、`src/locales/*.json`、`App.css`、`main.tsx` 等。

**Verification:** `pnpm run check:frontend`（含 `SettingsView.test.tsx`）。

#### I02.c：平台窗口壳

- [x] macOS：Tauri 2 Overlay/hiddenTitle 与侧栏原生红黄绿。Windows/Linux：无边框与 `TitlebarChrome` 窗控（c.5）。
- [x] 拖动、最小化、最大化/还原、关闭与双击顶栏缩放；拖动区不覆盖可点击控件；启动门禁在 Win/Linux 保留顶栏窗控（c.3）。
- [x] 配置、IPC 边界、组件、双语与权限按子任务拆分（c.1–c.5）。

**Verification:** `pnpm run check:frontend`、三平台 CI 桌面构建、本机 Tauri 验收。
**Dependencies:** I02.b。

- [x] **I02.c.1：应用壳拖动区与权限**：`WindowDragRegion`；侧栏与主区顶栏拖动区；`core:window:allow-start-dragging`。
    - Files：`src/components/WindowDragRegion.tsx`、`src/App.tsx`、`src/App.module.css`、`src/App.test.tsx`、`src-tauri/capabilities/default.json`。
    - Verification：新增“拖动区不包含按钮/输入等交互控件”的行为测试；`pnpm run check:frontend`；原生运行截图；拖动是否生效如实记录。
- [x] **I02.c.2：macOS 覆盖式标题栏**：`tauri.conf.json` 设置 `titleBarStyle: Overlay`、`hiddenTitle: true` 与 `trafficLightPosition`，保留 `decorations: true`，以截图校准坐标。
    - Files：`src-tauri/tauri.conf.json`。
    - Verification：macOS 无独立标题栏行、红黄绿在侧栏顶部；展开/折叠坐标不变。
- [x] **I02.c.3：加载与失败门禁的拖动区**：`LocaleStartup` 的门禁提供顶部拖动条，重试按钮位于拖动区之外，门禁中仍可拖动。
    - Files：`src/features/settings/LocaleStartup.tsx`、`LocaleStartup.module.css`、`LocaleStartup.test.tsx`。
    - Verification：门禁行为测试（重试仍可用）、`pnpm run check:frontend`、原生门禁截图（若能稳定触发）。
- [x] **I02.c.4：记录实现与验收范围**：更新开发说明、设计实现状态、英文 CHANGELOG 与本清单，写明未验证项。
    - Files：`docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`CHANGELOG.md`、`docs/plans/todo.md`。
- [x] **I02.c.5：Windows/Linux 无边框窗控**：`tauri.windows.conf.json` / `tauri.linux.conf.json`（`decorations: false`）；`TitlebarChrome`、`WindowControls`、`src/lib/desktop/window.ts`；启动门禁顶栏。

#### I02.d：设置中的「关于」（已完成）

- [x] Settings 顶栏「关于」页签与诚实占位文案（与计划页同源键，非真实版本信息）。
- [x] 展示品牌、真实版本、MIT 许可证与可选中复制的 GitHub 仓库地址；复用 `get_app_info` / `getAppInfo`，包含双语加载、预览、失败重试。

##### I02.d.1：关于页元数据行为与双语

- [x] 新增独立 AboutPanel，复用 getAppInfo；加载、真实版本、预览、失败重试与过期响应均明确处理。
- [x] 双语资源与行为测试随组件实现；品牌、版本和仓库地址保持原值。

**Files:** `AboutPanel.tsx`、`AboutPanel.module.css`、`AboutPanel.test.tsx`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** 目标 UI 测试；完整前端检查在 d.2 接线后执行。
**Dependencies:** I02.b.6。

##### I02.d.2：设置接线与键盘操作

- [x] 替换占位内容；关于页首次打开才读取，离开后保持已加载状态。
- [x] 页签支持左右方向键、Home/End；验证页签切换保留设置输入。

**Files:** `SettingsView.tsx`、`SettingsView.test.tsx`、`SettingsView.module.css`。
**Verification:** 目标测试、`check:frontend`、ego-browser 双语/720×560/键盘；macOS Tauri 真实版本查询。
**Dependencies:** I02.d.1。

##### I02.d.3：记录验收与学习说明

- [x] 更新 frontend 文档、设计实施状态、开发计划与英文 changelog；只记录实际验证结果。

**Files:** `docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`docs/plans/development-plan.md`、`CHANGELOG.md`；本清单随任务更新。
**Verification:** 格式与差异检查。
**Dependencies:** I02.d.2。

#### I02.e：侧栏拖动调宽（已取消）

固定 **200 / 88 px**（`App.module.css` `--sidebar-width`），仅折叠按钮切换 `data-collapsed`。

### 插队任务 I03：vibemate 自有品牌图标

维护者选择最初方案 1：深绿/鼠尾草绿 V 图形与圆润字标，替换 Tauri 默认图标。

#### I03.a：可维护的矢量母版

- [x] 保存选定参考稿，重绘透明 V 图形与深绿底应用图标 SVG；记录颜色、来源与重建命令。

**Files:** `assets/brand/mark.svg`、`assets/brand/app-icon.svg`、`assets/brand/concept.png`、`assets/brand/README.md`。
**Verification:** 与选定稿目视比较；小尺寸与单色轮廓检查。

#### I03.b：桌面图标与侧栏字标

- [x] 从同一 SVG 母版生成全部现有桌面 PNG/ICO/ICNS；侧栏继续复用打包图标。
- [x] 字标沿用原创 SVG 轮廓，加粗为圆润风格；移除旧 Tauri 黄点。

**Files:** `src-tauri/icons/` 全部已有桌面图标（生成文件逐项计入范围）；`src/components/BrandWordmark.tsx`、`src/App.module.css`（同步尺寸注释）。
**Verification:** `check:frontend`、macOS 原生构建；ego-browser 浅/深色、720×560、展开/折叠与品牌返回首页。

桌面图标是同一母版的生成产物，本子任务集中更新该产物集，不逐个修改几何形状。

#### I03.c：验收记录

- [x] 更新品牌/前端说明、设计状态与英文 changelog，如实记录平台限制。

**Files:** `docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`CHANGELOG.md`；本清单随任务更新。
**Verification:** 格式与差异检查。

### 插队任务 I04：内置主题配色与持久化

维护者要求同时提供简约、时尚和有设计感的主题。内置 forest/graphite/linen/iris/ocean，
每套有浅/深色；外观 system/light/dark 独立选择，通过 Rust 保存，浏览器仅窗口内预览。

#### I04.a：外观与主题存储

- [x] schema v3 单行保存 appearance/theme，保留语言与其他数据；校验未知值，读默认不写入。
- [x] Rust 类型化领域边界覆盖重启恢复、迁移保留、失败写入和损坏读取。

**Files:** `src-tauri/src/appearance.rs`、`storage.rs`、`settings.rs`（迁移版本断言）、`lib.rs`（模块注册）。
**Verification:** 目标/完整 Rust 测试、fmt/Clippy。

#### I04.b：IPC 接线

- [x] 注册只读/保存命令，返回安全错误；TypeScript 校验真实响应、匹配保存确认，预览不请求保存。

**Files:** `src-tauri/src/commands.rs`、`lib.rs`（命令注册）、`src/lib/desktop/appearance.ts`、`appearance.test.ts`。
**Verification:** IPC 包装测试、完整 Rust/前端检查与桌面构建。

#### I04.c：主题令牌

- [x] 新增五套浅/深色令牌，复用全站语义颜色；系统外观变化仍通过 CSS 生效。

**Files:** `src/themes.css`、`src/App.css`。
**Verification:** 浏览器全主题/明暗计算颜色、文字对比度与系统外观检查。

#### I04.d：设置控件与双语反馈

- [x] 扩展 AppearanceControl：读取、保存、失败、未知结果重新读取；保持输入与导航状态。
- [x] 中英文名称、加载/失败/预览提示与原生下拉键盘交互完整。

**Files:** `AppearanceControl.tsx`、`AppearanceControl.test.tsx`、`AppearanceControl.module.css`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** UI 行为测试、`check:frontend`、ego-browser 720×560/双语/明暗/键盘；真实 macOS 选择后重启。

##### I04.d.1：控件、样式与双语行为测试

**Files:** I04.d 上述五个文件。

##### I04.d.2：壳层导航回归

- [x] 壳层测试在偏好读取完成后操作，并覆盖主题、外观、已编辑输入一起跨导航保留。

**Files:** `src/App.test.tsx`；`App.tsx` 同步持久化说明。
**Verification:** 壳层 UI 测试与完整前端检查。

#### I04.e：验收说明

- [x] 更新英文 changelog、前端说明、设计状态，保留平台与运行限制。

**Files:** `CHANGELOG.md`、`docs/frontend.md`、`docs/plans/desktop-shell-design.md`；本清单随任务更新。
**Verification:** 格式与差异检查。

### 插队任务 I05：品牌组合 Logo 与无 hover

- [x] 展开时用一体化 V 图形/字标 SVG，折叠时用图形版；两种状态保留品牌返回首页与可访问名称。
- [x] 移除品牌 hover 背景，保留键盘焦点轮廓。

#### I05.a：两种 Logo 母版与组合组件

**Files:** `assets/brand/logo-expanded.svg`、`logo-collapsed.svg`、`selected-reference.jpg`、`README.md`、`src/components/BrandLogo.tsx`；移除 `BrandWordmark.tsx`。

I05.a.1 校正 `mark.svg` / `app-icon.svg` 与同母版生成的桌面图标集：忠实双色 V，不保留先前突出底笔画。

#### I05.b：桌面壳接线与验收

**Files:** `src/App.tsx`、`src/App.module.css`、`src/App.test.tsx`；共享文档/本清单同步更新。
**Verification:** 壳层行为测试、完整前端检查；ego-browser 展开/折叠、明暗、品牌 hover 与键盘返回首页。

### 插队任务 I06：全部主题使用配色圆圈

- [x] 把主题下拉改为全部五套配色同屏的原生 radio 圆圈，选中项有圆环与勾选标记。
- [x] 沿用保存/失败/重新读取和双语名称；保留键盘操作、忙碌禁用与跨导航状态。

**I06.a Files:** `AppearanceControl.tsx`、`AppearanceControl.module.css`、`AppearanceControl.test.tsx`、`src/App.test.tsx`、`src/themes.css`。
**I06.b Files:** `docs/frontend.md`、`docs/plans/desktop-shell-design.md`、`CHANGELOG.md`；本清单同步更新。
**Verification:** 目标 UI 测试、`check:frontend`；外观由维护者人工验收，本任务不运行浏览器/截图视觉检查。

控件与自动检查已完成；圆圈间距、配色与焦点效果待维护者人工验收。

### 插队任务 I07：调整 macOS Dock 图标大小

- [x] 增加 macOS 专用 SVG 留白，整体缩小至原来的 85%，保持 V 形状和配色。
- [x] 从专用 SVG 重新生成 ICNS，其他平台图标使用原有母版。
- [x] 检查 ICNS 与本地 macOS app 包中的图标资源一致。
- [ ] Dock 外观由维护者重启应用后人工验收。

**Files:** `assets/brand/app-icon-macos.svg`、`assets/brand/README.md`、`src-tauri/icons/icon.icns`、`CHANGELOG.md`、本清单。
**Verification:** 文档格式、图标生成与本地 app 打包资源检查；不运行浏览器或截图视觉检查。

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

### Checkpoint C09（P07–P09）

- [ ] P07–P08 已勾选；P09 完成后勾选本检查点。

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

### Checkpoint C12（P10–P12）

- [ ] P10–P12 均已勾选。

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

### Checkpoint C15（P13–P15）

- [ ] P13–P15 均已勾选。

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

### Checkpoint C18（P16–P18）

- [ ] P16–P18 均已勾选。

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

### Checkpoint C21（P19–P21）

- [ ] P19–P21 均已勾选。

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

### Checkpoint C24（P22–P24）

- [ ] P22–P24 均已勾选。

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

### Checkpoint C27（P25–P27）

- [ ] P25–P27 均已勾选。

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

### Checkpoint C30（P28–P30）

- [ ] P28–P30 均已勾选。

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

### Checkpoint C33（P31–P33）

- [ ] P31–P33 均已勾选。

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

### Checkpoint C36（P34–P36）

- [ ] P34–P36 均已勾选。

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

### Checkpoint C39（P37–P39）

- [ ] P37–P39 均已勾选；可打第一阶段 release 标签。
