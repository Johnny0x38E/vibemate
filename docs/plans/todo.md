# vibemate 第一阶段执行清单

状态：P00–P09 与基础外观 I02–I14 已完成。P10/P11 的真实凭据验收仍有待验项；P12 模型页和手动添加、P31 中央 MCP 管理的代码/自动检查已完成。BR1/BR2 已完成测试与命令入口拆分，隔离 Linux 原生 IPC 已验证无密钥 fixture 的模型/MCP 保存与重启；真实服务商 UI、凭据库及其他平台验收按各任务继续待验。Agent 部署等后续业务尚未开始。当前设计见 [desktop-shell-design.md](desktop-shell-design.md)，实现与验证记录见 [frontend.md](../frontend.md) 和 [backend-modularity.md](backend-modularity.md)。
这里是唯一任务状态来源，不能在其他文件维护第二份勾选清单。

## 执行约定

- 按依赖顺序实施，一次一个可检查的行为；默认不使用子代理。
- 外观由维护者人工验证；未经另行要求，不运行浏览器或截图视觉检查。格式、Lint、类型、行为测试与适用构建检查照常执行。原生功能和跨平台验证单独记录，外观确认不代表这些检查通过。
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
- [x] 确认第一条兼容路径；缺少 Command Code 资料时列出具体待补链接，不猜测端点。

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

2026-10-10 维护者确认基础外观本轮收尾；主题最终名称、顺序和保存交互见 I11/I12。

### 插队任务 I07：调整 macOS Dock 图标大小

- [x] 增加 macOS 专用 SVG 留白，整体缩小至原来的 85%，保持 V 形状和配色。
- [x] 从专用 SVG 重新生成 ICNS，其他平台图标使用原有母版。
- [x] 检查 ICNS 与本地 macOS app 包中的图标资源一致。
- [x] Dock 外观随 2026-10-10 基础外观收尾确认。

**Files:** `assets/brand/app-icon-macos.svg`、`assets/brand/README.md`、`src-tauri/icons/icon.icns`、`CHANGELOG.md`、本清单。
**Verification:** 文档格式、图标生成与本地 app 打包资源检查；不运行浏览器或截图视觉检查。

### 插队任务 I08：平滑 macOS 图标圆角

- [x] 普通圆角矩形改为连续曲率轮廓，扩大圆角过渡至母版的 160 px，保持现有图标大小与留白。
- [x] 重新生成 ICNS、打包 macOS app 并检查资源一致性。
- [x] Dock 圆角外观随基础外观收尾确认。

**Files:** `assets/brand/app-icon-macos.svg`、`assets/brand/README.md`、`src-tauri/icons/icon.icns`、`CHANGELOG.md`、本清单。
**Verification:** SVG 曲线接点、文档格式与 app 图标资源；不运行截图视觉检查。

### 插队任务 I09：清除 i 点缀色下方残留

- [x] 为界面 Logo 遮罩补上与展开 SVG 母版一致的原始圆点清除区域。
- [x] 保持橙色圆点位置、大小与文字间距。
- [x] 运行前端检查：96 项 UI 测试、8 项发布测试、格式、Lint、类型检查和构建通过。
- [x] i 圆点修正随基础外观收尾确认。

**Files:** `src/components/BrandLogo.tsx`、`CHANGELOG.md`、本清单。
**Verification:** `check:frontend`；不运行浏览器或截图视觉检查。

### 插队任务 I10：重新设计关于页

- [x] I10.a：与常规 Tab 同宽，展开 Logo、说明与信息行建立层次；GitHub 图标入口、打开中与失败反馈。
- [x] I10.b：Rust 固定仓库打开命令，使用稳定 Tauri opener，仅暴露固定地址，不增加通用 URL/文件打开权限。
- [x] I10.c：更新交互测试、中英资源、Octicons MIT 声明与前端文档；运行前端、Rust 和本地桌面构建检查。
- [x] 维护者确认基础外观本轮收尾；未运行浏览器或截图检查。

**I10.a Files:** `AboutPanel.tsx`、`AboutPanel.module.css`、`AboutPanel.test.tsx`。
**I10.b.1 Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`。
**I10.b.2 Files:** `src/lib/desktop.ts`、`src/lib/desktop/repository.test.ts`。
**Verification:** `check:frontend` 通过 99 项 UI 测试和 8 项发布测试；Rust fmt/Clippy 通过、31 项测试通过（既有 OS 密钥库测试忽略）；locked macOS 桌面构建和本地 unsigned app 打包通过。外观已收尾；实际系统浏览器打开与 Windows/Linux 原生验收仍待验证。

**I10.c Files:** `src/locales/en.json`、`src/locales/zh-CN.json`、`assets/licenses/octicons-MIT.txt`、`docs/frontend.md`、`CHANGELOG.md`；本清单同步更新。

### 插队任务 I11：增加 Notion 风格主题

- [x] I11.a：参照 Notion 官方公开浅/深色语义色，新增主题 token、ID 与中英文名称。
- [x] I11.b：增加 Rust Theme 与 v4 迁移，保留 v3 外观、语言及无关数据；验证新增主题保存和重开读取。
- [x] I11.c：更新选择/IPC 测试、文档及 CHANGELOG；完整前端/Rust 检查和本地 macOS 构建。
- [x] 维护者确认基础外观本轮收尾；未运行浏览器或截图检查。

**Verification:** 前端 101 项 UI 测试、8 项发布测试、格式/Lint/类型/构建通过；Rust fmt/Clippy、33 项测试通过（既有 OS 密钥库测试忽略）；locked macOS 构建与 unsigned app 打包通过。文字和焦点 token 数值对比度通过；外观已收尾；纸墨主题的原生重启选择与 Windows/Linux 验收仍待验证。

**I11.a Files:** `src/themes.css`、`src/lib/desktop/appearance.ts`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**I11.b Files:** `src-tauri/src/appearance.rs`、`src-tauri/src/storage.rs`、`src-tauri/src/settings.rs`。
**I11.c.1 Files:** `src/lib/desktop/appearance.test.ts`、`src/features/settings/AppearanceControl.test.tsx`。
**I11.c.2 Files:** `docs/frontend.md`、`CHANGELOG.md`；本清单同步更新。

### 插队任务 I12：主题命名、排序与无闪烁保存

- [x] 「Notion 风格」改名为「纸墨 / Ink」，排列第二，保留已有保存 ID。
- [x] 保存时不显示短暂提示文字，保持操作禁用、失败和未确认反馈。
- [x] 完成前端检查（101 项 UI 测试、8 项发布测试、格式/Lint/类型与构建）和 locked macOS 构建、unsigned app 打包。
- [x] 维护者确认基础外观本轮收尾；未运行浏览器或截图检查。

**I12.a Files:** `src/lib/desktop/appearance.ts`、`src/features/settings/AppearanceControl.tsx`、`src/features/settings/AppearanceControl.test.tsx`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**I12.b Files:** `docs/frontend.md`、`CHANGELOG.md`、本清单。

### 插队任务 I13：关于页仓库行与纯图标

- [x] GitHub 按钮改为 24 px 无边框、无方块背景的纯图标，仓库行恢复与版本、许可证相同的 56 px 最小高度。
- [x] 保持点击、键盘焦点、保存无关的打开状态与失败反馈。
- [x] 前端检查（101 项 UI 测试、8 项发布测试、格式/Lint/类型与构建）和 locked macOS 构建、unsigned app 打包。
- [x] 关于页外观随基础外观收尾确认。

**Files:** `src/features/settings/AboutPanel.module.css`、`CHANGELOG.md`、`docs/frontend.md`、本清单。

### 插队任务 I14：微调 Dock 图标 V 的视觉重心

- [x] macOS 专用 V 向下偏移，512 px 画布上约 6.8 px；底板、大小、圆角与配色保持一致。
- [x] 生成 ICNS、检查本地 app 包资源一致性和文档格式。
- [x] Dock 视觉重心随基础外观收尾确认；未运行截图检查。

**Files:** `assets/brand/app-icon-macos.svg`、`assets/brand/README.md`、`src-tauri/icons/icon.icns`、`CHANGELOG.md`、本清单。

### Task P09: 让现有界面支持中英文

**Description:** 把现有 App 的文本与 metadata 状态接入 i18n，并将翻译一致性和切换回归检查纳入完成标准。

**Acceptance criteria:**

- [x] 现有导航/标题/状态/提示、可访问名称均有中英文；语言偏好入口可用，语言切换保留页面状态。
- [x] Rust 后续返回稳定错误 code + 安全参数，由前端翻译；当前已有 metadata 错误按边界处理，不向用户直接显示未知原始错误或翻译 key。
- [x] 自动检查缺失 key、空翻译和插值参数不一致，接入 `check:frontend`；后续业务 UI 必须随功能同时补齐中英文。

**Verification:**

- [x] 运行 `pnpm run test:ui src/App.test.tsx`，覆盖双语、切换、回退、启动偏好及错误状态。
- [x] 运行 `check:frontend`；按需把资源校验/脚本/CI接线拆成 P09 子任务后验证。
- [x] 维护者人工检查双语布局、720×560 与长文本；适用的 Tauri 功能验证检查日期/数字、可访问名称及模型 ID/配置内容保留。未经要求不运行浏览器或截图视觉检查。

**Dependencies:** P04,P07,P08。

**Files likely touched:**

- `src/main.tsx`
- `src/App.tsx`
- `src/App.test.tsx`
- `src/locales/zh-CN.json`
- `src/locales/en.json`

**Estimated scope:** M：5 个建议主文件；额外校验脚本、包命令和注册接线先拆子任务。

P09 超过五个文件，按以下顺序拆分；每个子任务单独检查。全部子任务通过完整检查后勾选上方自动验收项；维护者人工检查项与 C09 待人工确认后再勾选。
盘点结论：现有 JSX 可见文本与可访问名称均已走翻译键；未翻译的仅有品牌 `vibemate`、`MIT` 与
Provider/Agent 品牌名（按规则保留原值）。缺口在资源校验、metadata 边界错误形状、遗留 key/术语与 App 级双语回归。
预计不改 Rust（`get_app_info` 不会失败，`open_project_repository` 已返回 `open_failed`），不新增依赖。

### P09.a：翻译资源自动校验

- [x] 以 Node 内置能力检查两份资源：key 集合一致、无空/纯空白值、叶子只能是字符串、插值参数集合一致、复数后缀覆盖 `Intl.PluralRules` 要求的形式。
- [x] 用错误样例证明每类问题都会报出 key 路径；`check:frontend` 在 UI 测试前运行该检查，CI/Release 沿用同一入口不另改工作流。
- [x] 移除 `src/i18n/index.test.ts` 中被取代的插值比对，保留类型等同与运行时行为测试。

**Files:** `scripts/i18n-resources.mjs`、`scripts/i18n-resources.test.mjs`、`package.json`（`test:i18n`、`check:i18n`）、`src/i18n/index.test.ts`。
**Verification:** `pnpm run check:i18n`、脚本测试（`test:i18n`）、`pnpm run check:frontend`。
**Dependencies:** P07,P08。

### P09.b：metadata 错误在桌面边界收敛

- [x] `getAppInfo` 把 invoke 拒绝与畸形响应转为稳定 code（如 `operation_failed`/`invalid_response`），不保留原始诊断；仓库打开失败同样只暴露 code。
- [x] 补边界测试：预览、有效响应、畸形响应、带原始诊断的拒绝；关于页继续只显示翻译后的失败与重试文案。

**Files:** `src/lib/desktop.ts`（`MetadataRequestError`）、`src/lib/desktop/metadata.test.ts`（新）、`src/lib/desktop/repository.test.ts`；关于页每个操作只有一条失败文案，无需按 code 区分，`AboutPanel.tsx` 未改。
**Verification:** 目标边界测试、`AboutPanel.test.tsx`、`pnpm run check:frontend`；不改 Rust。
**Dependencies:** P09.a。

### P09.c：清理遗留资源与统一术语

- [x] 删除已不使用的旧首页 `app.*` key；`app.planned` 迁到 `desktop.overview` 分组，复数/Intl 测试改用测试内资源而不是保留无用 key。
- [x] 按维护者决定，中文统一使用导航术语「服务商 / Agent / 技能 / MCP 服务器」，英文不变；品牌、模型 ID 与 URL 不翻译。顺带更正 `index.test.ts` 中名不副实的测试名。

**Files:** `src/locales/en.json`、`src/locales/zh-CN.json`、`src/features/overview/RelationshipOverview.tsx`、`src/i18n/index.test.ts`。
**Verification:** `pnpm run check:i18n`、`pnpm run check:frontend`。
**Dependencies:** P09.a。

### P09.d：App 级双语回归测试

- [x] 在 `src/App.test.tsx` 覆盖：已保存中文/跟随中文系统启动、经真实选择器切换后保留页面/页签/折叠与输入、缺失中文条目回退英文而不显示 key。
- [x] 覆盖中英文错误状态：启动读取失败与重试、关于页 metadata 失败与重试、语言保存失败；断言界面不出现原始诊断或翻译 key。
- [x] 若测试暴露组件缺陷，修复所在组件并计入本子任务文件数（本次未发现缺陷，未改组件）。

**Files:** `src/App.test.tsx`（仅替换 desktop 边界，React 与译器保持真实）。
**Verification:** `pnpm run test:ui src/App.test.tsx`（记录条数）、`pnpm run check:frontend`。
**Dependencies:** P09.b,P09.c。

### P09.e：验收记录与勾选

- [x] 更新 i18n 说明（校验命令、metadata 错误 code、过时的 App 测试描述）、开发计划基础现状与英文 changelog `Unreleased`。
- [x] 完整检查通过后勾选自动验收项；维护者人工双语/720×560/长文本检查完成前不勾选该项与 C09。

**Files:** `docs/frontend.md`、`CHANGELOG.md`、`docs/plans/development-plan.md`；本清单随子任务更新。
**Verification:** `pnpm run check:frontend`、格式与差异检查；无 Rust 变更则不跑 Rust 检查。
**Dependencies:** P09.d。

备注：自动检查通过（119 项 UI 测试，其中 App 18 项；i18n 校验 14 项；发布 8 项）；未改 Rust。维护者于 2026-10-10 人工确认双语界面。

### Checkpoint C09（P07–P09）

- [x] P07–P08 已勾选；P09 完成后勾选本检查点。

## 阶段 C：Provider 与 Model

### Task P10: 保存一个 Provider 配置

**Description:** 让用户从表单保存非敏感账户设置，重启后仍可查看，并为同一服务商创建多个实例。

**Acceptance criteria:**

- [ ] 校验名称、URL、协议和扩展字段，持久化稳定 ID；错误通过类型化 IPC 返回。
- [ ] 列表使用有界分页和稳定排序，编辑时保持实例身份，不按显示名称关联。
- [ ] 提供加载、空、错误与保存反馈，浏览器模式不伪造保存成功。

**Verification:**

- [x] 运行 Rust `providers` 过滤测试与 ProviderForm UI 测试，覆盖非法 URL 和持久化重读。
- [x] 运行 `pnpm run check:frontend` 与 Rust fmt/Clippy。
- [ ] 实际 Tauri 保存非敏感配置，重启验证；需要额外注册/样式文件时先拆子任务。

**Dependencies:** P04,P05,P09。

**Files likely touched:**

- `src-tauri/src/providers.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。
P10 超过五个文件，按以下顺序拆分；每个子任务单独检查，全部完成后勾选上方验收项。
维护者决定（2026-10-10）：三家 kind 的扩展字段允许列表为空，非空 `extensions` 以 `extension_field_not_supported` 拒绝，P10 不建扩展子表，有证据后再增加键与存储；
Command Code 实例暂只允许 `chat_completions`，按模型覆盖留给 P12/P15；新增直接依赖 `url`（锁文件已有）；ID 用 SQLite `lower(hex(randomblob(16)))`，不加 uuid；
服务商 kind 为 `command-code`（品牌名 "Command Code"），不按订阅套餐命名：GOAT 只是 Command Code 的套餐之一；改名直接修改未发布的 v5 定义，不另加迁移。
不做删除；游标分页（limit 1..=100，`created_at ASC, id ASC`）；`revision` 乐观并发；未知 kind/protocol 返回稳定错误码；错误码精简到前端需要区分提示的粒度。

### P10.0：拆分记录

- [x] 在本清单写入子任务与维护者决定；在 `docs/integrations/providers.md` 记录 Command Code 暂只允许 Chat Completions 的理由。

**Files:** `docs/plans/todo.md`、`docs/integrations/providers.md`。
**Verification:** 格式与差异检查。
**Dependencies:** P05,P09。

### P10.a.1：领域类型与校验

- [x] `ProviderKind`/`ProviderProtocol`/`ProviderId`、内置模板、名称/URL/协议/扩展字段校验与稳定错误码；不访问数据库，不依赖 Tauri。

**Files:** `src-tauri/src/providers.rs`（新）、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`。
**Verification:** `cargo test --manifest-path src-tauri/Cargo.toml --locked providers`（条数非零）、fmt、Clippy。
**Dependencies:** P10.0。

### P10.a.2：schema v5 与持久化

- [x] v5 迁移 `provider_instance`；创建/更新/读取/游标分页，编辑保持 `id`/`created_at` 并以 `revision` 拒绝过期写入；覆盖重开重读、写失败、坏数据与 v4→v5 升级。

**Files:** `src-tauri/src/providers.rs`、`src-tauri/src/storage.rs`、`src-tauri/src/settings.rs` 与 `src-tauri/src/appearance.rs`（两处写死的 schema 版本断言）。
**Verification:** `cargo test ... providers`（条数非零）、fmt、Clippy。
**Dependencies:** P10.a.1。

### P10.b：Tauri 命令与类型化 IPC 包装

- [x] 薄命令 `list_provider_templates`/`list_providers`/`get_provider`/`create_provider`/`update_provider` 与注册；TS 包装运行时校验响应、收敛错误码，浏览器预览不调用 IPC、不伪造保存。

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src/lib/desktop/providers.ts`（新）、`src/lib/desktop/providers.test.ts`（新）。
**Verification:** Rust fmt/Clippy/全部测试、`pnpm run check:frontend`。
**Dependencies:** P10.a.2。

### P10.c.1：Provider 表单（前端）

- [x] `ProviderForm` 从模板创建/编辑，字段错误按错误码中英文提示，保存中/成功/失败/结果未知反馈，预览禁用保存。

**Files:** `src/features/providers/ProviderForm.tsx`、`ProviderForm.module.css`、`ProviderForm.test.tsx`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** `pnpm run test:ui src/features/providers`、`pnpm run check:frontend`。
**Dependencies:** P10.b。

### P10.c.2：Provider 列表与页面（前端）

- [x] `ProvidersView` 加载/空/错误+重试/加载更多，按 `id` 选择编辑，丢弃过期异步结果；不显示为已连接。

**Files:** `src/features/providers/ProvidersView.tsx`、`ProvidersView.module.css`、`ProvidersView.test.tsx`、两份 locale。
**Verification:** 同 P10.c.1。
**Dependencies:** P10.c.1。

### P10.c.3：App 接线

- [x] 用 `ProvidersView` 替换 providers 占位并保持挂载；App 级双语导航回归。

**Files:** `src/App.tsx`、`src/App.test.tsx`。
**Verification:** `pnpm run test:ui src/App.test.tsx`、`pnpm run check:frontend`。
**Dependencies:** P10.c.2。

### P10.c.4：服务商页交互重构（前端）

- [x] 按用户反馈改为列表主页 + 二级页面（页面内切换，无路由库）：「新建配置」直接进入表单，首个字段为「服务商」下拉（默认选中模板列表第一项并填入其默认值，切换时只替换未改动的默认值），编辑页按分组组织（目前仅「基本信息」，P11「密钥」/P12「模型」各自成组，不加长单个表单），服务商只读；「服务商类型」统一改称「服务商」；每个视图只有一个主按钮（实心强调色），取消/返回为次按钮，行内编辑为弱按钮；label/value/提示/错误分层；保留焦点管理、结果未知先刷新、冲突重载、过期结果丢弃、保存中 aria-disabled/readOnly、隐藏保持挂载；返回链接使用 `Icon` 的 back 箭头，可见文字为「返回」、可访问名称为「返回服务商列表」；「新建配置」和行内「编辑」带 18px 的 plus/edit 图标（保存/取消保持纯文字）；列表行名称前、详情页只读服务商值前、新建页服务商下拉旁显示官方品牌图标（`assets/providers/`，20×20、`object-fit: contain`、`alt=""`，OpenRouter 按深浅色切换两个官方文件，未知类型不显示）；去掉列表页常驻的「已保存」提示，改为应用级通知（新增 `src/components/Notifications.tsx`，右上角，约 3.5 秒自动消失，悬停/聚焦暂停，可手动关闭，`role="status"` 播报不抢焦点）；保存成功后焦点回到该行「编辑」按钮，新项不在已加载页时聚焦页标题。
- [ ] 维护者在 Tauri 中目视检查各主题按钮对比度、文本层级与 720×560 布局。

**Files:** `src/features/providers/`（`ProvidersView`、新增 `ProviderPage`/`providerButtons.module.css`、`ProviderForm` 及测试）、两份 locale、`src/App.test.tsx`、`src/components/Icon.tsx`、`assets/providers/`、`src/components/Notifications.*`、`src/App.tsx`、`docs/frontend.md`、`CHANGELOG.md`。
**Verification:** `pnpm run test:ui src/features/providers src/App.test.tsx`、`pnpm run check:frontend`（208 项 UI 测试）。
**Split note:** 本项超过 5 个文件，因为它是按同一轮用户反馈对已完成页面做的一次整体交互重构，各文件改动需同时落地才能通过同一组 UI 测试；事后不再拆分，后续前端项仍按 1–5 个文件拆分。
**Dependencies:** P10.c.3。

### P10.d：真实验证与文档

- [x] 按实际实现更新学习说明（含错误码与中英文提示表）、架构现状、开发计划与 phase-1 现状、英文 changelog。
- [x] 完整自动检查：Rust fmt/Clippy、全部 Rust 测试（60 通过，钥匙串 smoke 测试保持 ignored）、`check:frontend`（188 项 UI 测试）、`pnpm run tauri build --no-bundle -- --locked`（macOS）。
- [ ] 维护者先备份 app-data，再在实际 Tauri 中保存非敏感配置→完全退出→重启重读；通过后勾选上方 P10 验收项。

**Files:** `docs/frontend.md`、`docs/architecture.md`、`docs/plans/development-plan.md`、`docs/plans/phase-1.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** 完整检查与实际 Tauri 运行。
**Dependencies:** P10.c.3。

### P10.0.1：Command Code 改名（代码）

- [x] kind `command-code-goat` → `command-code`，品牌名 "Command Code"（GOAT 只是 Command Code 的订阅套餐之一）；直接修改未发布的 v5 CHECK 约束，不加迁移；同步测试、TS 类型和总览页名称。

**Files:** `src-tauri/src/providers.rs`、`src-tauri/src/storage.rs`、`src/lib/desktop/providers.ts`、`src/features/overview/RelationshipOverview.tsx`、`docs/plans/desktop-shell-preview.html`（设计稿中的名称）。
**Verification:** `git grep --untracked -i goat` 只剩对 GOAT 套餐的准确描述（另有 logo 内嵌 base64 的偶然匹配）；Rust fmt/Clippy/全部测试与 `providers` 过滤测试（条数非零）、`pnpm run check:frontend`、`pnpm run tauri build --no-bundle -- --locked`。
**Dependencies:** P10.c.3。

### P10.0.2：Command Code 改名（文档）

- [x] 文档改用 "Command Code"；GOAT 套餐的事实保留并写作“Command Code 的 GOAT 套餐”，明确证据只覆盖 GOAT 套餐，Pro/Provider 套餐待 P15 核实；P15 改名并加入该核实项。

**Files:** `README.md`、`AGENTS.md`、`CHANGELOG.md`、`docs/frontend.md`、`docs/integrations/providers.md`、`docs/integrations/compatibility.md`、`docs/plans/development-plan.md`、`docs/plans/phase-1.md`、`docs/plans/todo.md`。
**Verification:** 同 P10.0.1 的 `git grep` 检查与 `pnpm run check:frontend`（含 Prettier）。
**Dependencies:** P10.0.1。

### P10.e.1：评审修正（Rust）

- [x] 名称拒绝零宽/BOM/双向控制等隐藏格式字符并要求可见字符；URL 去掉首尾空白后拒绝内部空白与控制字符；更新影响行数不为 1 时返回 `revision_conflict`；游标时间戳只接受 ASCII 数字；模板按 kind 对应的测试；settings/appearance 的 schema 版本断言改用最新迁移版本。

**Files:** `src-tauri/src/providers.rs`、`src-tauri/src/storage.rs`（测试用 `latest_schema_version`）、`src-tauri/src/settings.rs`、`src-tauri/src/appearance.rs`。
**Verification:** Rust fmt/Clippy、全部测试（60 通过，1 ignored）、`providers` 过滤测试（27 条）、`pnpm run check:frontend`。
**Dependencies:** P10.0.1。

### P10.e.2：评审修正（文档）

- [x] 在 `docs/integrations/providers.md` 记录 URL/名称校验边界：结尾只去一个 `/`、拒绝内部空白与控制字符、回环与私有地址暂允许并留给 P13。

**Files:** `docs/integrations/providers.md`、`docs/plans/todo.md`、`CHANGELOG.md`。
**Verification:** `pnpm run check:frontend`（含 Prettier）。
**Dependencies:** P10.e.1。

### Task P11: 维护 Provider 密钥

**Description:** 让用户在创建 Provider 时必填密钥，并在详情页替换密钥，同时只显示凭据存在状态。清除密钥随以后删除 Provider 一起实现（维护者决定，2026-10-10）。

**Acceptance criteria:**

- [ ] UI 临时输入密钥，提交/取消后清理；配置、错误、日志与列表不回传密钥。
- [ ] 数据库保存引用；凭据/元数据任一步失败按约定补偿并报告结果。
- [ ] 创建时 Provider 行与凭据条目同时成功，否则两者都不留下；补偿失败返回明确的结果未知错误码。

**Verification:**

- [ ] 运行 Rust `credentials`/`providers` 测试，注入两种资源的保存失败。
- [ ] 运行 UI 测试与 `check:frontend`，检查敏感字段清理。
- [ ] Tauri 使用临时凭据完成创建/替换并重启，确认日志脱敏。

**Dependencies:** P06,P10。

**Files likely touched:**

- `src-tauri/src/providers.rs`
- `src-tauri/src/credentials.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。
P11 超过五个文件，按以下顺序拆分；每个子任务单独检查，全部完成后勾选上方验收项。
维护者决定（2026-10-10）：状态只来自 SQLite 引用行 `provider_credential`，不查询凭据库；不加 zeroize/secrecy，沿用脱敏的 `Secret(String)`；不建清理表；
没有“待配置”状态：创建时密钥必填，顺序为“全部校验 → 事务外 `randomblob` 生成 ID → 写凭据 `provider-<id>` → 同一事务插入 `provider_instance` 与 `provider_credential` → 失败则删除凭据”，删除也失败返回 `create_outcome_unknown`；
详情页只显示状态并替换（`replace_then_commit`），无清除命令，清除随以后删除 Provider 实现；替换不改 Provider `revision`；
密钥字符由服务商决定：只去首尾空白，拒绝空值与控制字符（含 CR/LF），上限为 Windows 凭据 2560 字节（UTF-16，即 1280 码元），各平台统一；
P10 遗留的无密钥测试行不在迁移中删除，状态报告为 `missing`，替换动作兼作设置。

### P11.0：拆分记录

- [x] 在本清单写入子任务与维护者决定；完整设计（流程、补偿、错误码、IPC 草案）见维护者保存的 P11 计划。

**Files:** `docs/plans/todo.md`。
**Verification:** 格式与差异检查。
**Dependencies:** P10。

### P11.a.1：凭据层准备

- [x] `Secret` 从 IPC 字符串反序列化；`validate_secret`；创建用 `save_new_then_commit`（失败删除新条目并报告是否清理成功）；`replace_then_commit` 泛型返回值并在旧值损坏时继续替换；共享测试用 `FakeStore`（可注入读取失败、计数调用）。

**Files:** `src-tauri/src/credentials.rs`。
**Verification:** `cargo test --manifest-path src-tauri/Cargo.toml --locked credentials`（条数非零）、全部测试、fmt、Clippy。
**Dependencies:** P11.0。

### P11.a.2：schema v6 与错误码

- [x] v6 迁移 `provider_credential`（外键级联、`credential_ref = 'provider-' || provider_id`），不改遗留行；v4→v5 测试只运行前五个迁移；`ProviderError` 增加密钥与凭据库错误码。

**Files:** `src-tauri/src/storage.rs`、`src-tauri/src/providers.rs`。
**Verification:** `cargo test ... storage`、`cargo test ... providers`（条数非零）、fmt、Clippy。
**Dependencies:** P11.a.1。

### P11.a.3：创建时必填密钥（Rust）

- [x] `CreateProviderRequest.secret`；凭据与 SQLite 的创建补偿；注入凭据写入、SQLite 写入与补偿删除失败；数据库文件不含密钥；命令使用 OS 凭据库与进程内凭据写锁。

**Files:** `src-tauri/src/providers.rs`、`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`。
**Verification:** `cargo test ... providers`（条数非零）、全部测试、fmt、Clippy。
**Dependencies:** P11.a.2。

### P11.a.4：密钥状态与替换（Rust）

- [x] `get_secret_status` 只读 SQLite（`set`/`missing`）；`replace_provider_secret` 先确认 Provider 存在，再替换并 upsert 引用行，不改 Provider `revision`。Tauri 命令移到 P11.b.1。

**Files:** `src-tauri/src/provider_secrets.rs`（新）、`src-tauri/src/lib.rs`（模块注册）。
**Verification:** `cargo test ... provider_secrets`（条数非零）、全部测试、fmt、Clippy。
**Dependencies:** P11.a.3。

### P11.b.1：状态与替换的 TS 包装

- [x] 薄命令 `get_provider_secret_status`（参数 `providerId`）/`replace_provider_secret`（参数 `request: { providerId, secret }`，凭据写锁、OS 凭据库）与注册。
- [x] TS 包装运行时校验状态响应（严格键集合），浏览器预览不调用 IPC、不伪造保存（与 P11.b.2 一起和前端协调）。`providerSecrets.ts` 复用 `providers.ts` 的错误类与错误码白名单；`providerId` 必须与请求一致，`set` 带非负整数时间、`missing` 为 `null`，替换成功必须为 `set`（29 个测试）。

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src/lib/desktop/providerSecrets.ts`（新）、`src/lib/desktop/providerSecrets.test.ts`（新）。
**Verification:** Rust fmt/Clippy/全部测试、`pnpm run check:frontend`。
**Dependencies:** P11.a.4。

### P11.b.2：`createProvider` 带密钥

- [x] `CreateProviderInput.secret` 与新错误码；错误与响应不含密钥。与 P11.c.1 连续完成，否则表单类型检查失败。`validateRecord` 改为严格键集合（多出字段即 `invalid_response`）；`providers.test.ts` 29 → 39。

**Files:** `src/lib/desktop/providers.ts`、`src/lib/desktop/providers.test.ts`。
**Verification:** `pnpm run test:ui src/lib/desktop`，P11.c.1 后 `check:frontend`。
**Dependencies:** P11.a.3。

### P11.c.1：创建表单的必填密钥（Frontend Developer）

- [x] 仅创建模式显示密码字段；提交（任何结果）、取消、卸载、隐藏后清空；新错误码的字段/表单级提示；结果未知时重新保存需重新输入密钥。前端只检查去首尾空白后非空；`secret_invalid` 在字段旁，`credential_store_*` 在表单顶部（`ProviderForm.test.tsx` 21 → 31）。

**Files:** `src/features/providers/ProviderForm.tsx`、`ProviderForm.test.tsx`、`src/locales/en.json`、`src/locales/zh-CN.json`。
**Verification:** `pnpm run test:ui src/features/providers`、`pnpm run check:frontend`。
**Dependencies:** P11.b.2。

### P11.c.2：密钥区块（Frontend Developer）

- [x] `ProviderKeys` 显示“已设置 · 更新时间”或遗留的“未设置”，替换（`missing` 时为设置），加载/错误/结果未知刷新/预览禁用；无清除按钮。成功后用应用级通知，按钮为次按钮（页面唯一主按钮仍是基本信息的「保存」）；22 个测试。

**Files:** `src/features/providers/ProviderKeys.tsx`、`ProviderKeys.module.css`、`ProviderKeys.test.tsx`、两份 locale。
**Verification:** 同 P11.c.1。
**Dependencies:** P11.b.1。

### P11.c.3：接入详情页（Frontend Developer）

- [x] 编辑页表单下方独立“密钥”区块；页面隐藏或返回时清空未提交的密钥。替换进行中阻止返回；替换不触发基本信息保存、不离开页面（`ProvidersView.test.tsx` 24 → 27，`App.test.tsx` 覆盖跨页面隐藏清空）。真实 Tauri 运行仍待 P11.d 人工验证。

**Files:** `src/features/providers/ProvidersView.tsx`、`ProvidersView.test.tsx`。
**Verification:** 同 P11.c.1。
**Dependencies:** P11.c.2。

### P11.d：文档与真实验证

- [ ] 更新架构/计划现状与英文 changelog（`docs/frontend.md` 由 Frontend Developer 维护）；完整自动检查与 `tauri build --no-bundle`。
- [ ] 维护者先备份 app-data，在实际 Tauri 中带临时合成密钥创建→替换→重启，确认钥匙串条目为 `provider-<id>`、日志无密钥，并手动运行 ignored 凭据冒烟测试。Windows/Linux 未测时在此注明。

**Files:** `docs/architecture.md`、`docs/plans/development-plan.md`、`docs/plans/phase-1.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** 完整检查与实际 Tauri 运行。
**Dependencies:** P11.c.3。

### P11：统一保存调整

维护者随后要求新建、编辑共用一个密钥输入与保存动作，取代 P11.c.1–c.3
中的独立密钥组。已移除 `ProviderKeys`，编辑页只查询 SQLite 中的凭据引用，
已有密钥显示固定掩码和闭眼图标，空输入保留原密钥，新输入与基本信息一起保存。
不再显示更新时间、常驻说明或独立密钥保存按钮。

Rust 在写入新密钥后，将配置与引用提交到同一 SQLite 事务；明确失败恢复旧密钥，
恢复失败或提交不确定则要求刷新和显式重试，不报告成功。仅保存基本信息不访问凭据库。

自动验证通过：306 项 UI 测试、14 项翻译检查测试、8 项发布工具测试、
Rust fmt/Clippy、182 项 Rust 测试，以及 locked macOS no-bundle 构建。
真实 OS 凭据测试仍忽略；原生保存→重启、钥匙串提示与 Windows/Linux 仍待人工验证。
行为说明与测试位置见 [frontend.md](../frontend.md#api-keys)。

### Task P12: 获取模型列表并勾选

**Description:** 用户配置基础 URL 和密钥后，点击「获取模型」拉取服务商的模型列表（三家都用 `GET {base}/models`），逐个勾选要用的模型；手动添加模型作为补充。这是项目第一个网络请求，承接原 P13 的超时、取消、脱敏、模拟服务测试与“只在用户触发时请求”的要求，并吸收原 P14 的 OpenRouter 模型发现。

**Acceptance criteria:**

- [ ] 获取只由用户点击触发；按 Provider ID + 模型 ID 保存，新获取的模型默认不勾选，合并从不修改勾选状态。
- [ ] 完整获取后：已勾选但上游消失的模型保留并标记「上游已不可用 · 自 <日期>」，不自动删除；未勾选且消失的获取行删除；手动行不删除。达到页数或模型数上限、或列表为空时只新增和更新。
- [ ] 连接 10 s、单次 30 s、整次 90 s 超时；响应体最多 8 MiB；每个 Provider 最多 5000 个模型；OpenRouter 每页 500、最多 10 页；不跟随重定向，只用 HTTPS。
- [ ] 取消立即中断进行中的请求（异步 reqwest 与取消信号用 `tokio::select!` 竞争），不写库；获取期间 Provider 设置被修改则不保存（`model_fetch_stale`）。
- [ ] 密钥不出现在错误、日志、URL、响应、数据库与测试快照中；错误码无负载，中英文提示。
- [ ] 生产客户端读取系统代理（环境变量，以及 macOS/Windows 系统设置；Linux 只读环境变量）；测试客户端通过可注入设置使用 `.no_proxy()`，不受 `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`/`NO_PROXY` 及小写变体影响。
- [ ] 手动添加：ID 只去首尾空白，1–256 字符，不含空白或控制字符；与之后获取的同 ID 模型合并为一行，保持手动来源。
- [ ] `list_provider_models` 的搜索为模糊搜索：不区分大小写；匹配模型 ID、显示名称和别名；多个词必须全部匹配；`-`、`_`、`/`、`.` 与空白等价；按 exact > prefix > contains > subsequence 排序，多词先比较是否整体精确匹配，再比较最弱词的档位、档位之和，最后按 `model_id` 稳定排序。搜索在 Rust 内存中执行（SQLite 只按 Provider 和勾选状态过滤），有查询时返回前 `limit` 条与匹配总数、不带游标；无查询时保持稳定游标分页；查询超过 200 个字符返回 `invalid_request`。
- [ ] Command Code 显示每个模型的 `supported_endpoints`（比较时接受带或不带 `/v1` 前缀的值），获取的模型只有路由支持当前协议时才可勾选；含 `/` 的模型 ID（如 `mistral/mistral-large-4`）可正常保存、勾选、分页与搜索。
- [ ] 界面与文案不把“获取成功”说成“密钥正确”：Command Code 的列表不带密钥也能获取，获取成功只说明列表请求成功；三家请求都带 `Authorization: Bearer <key>`（DeepSeek 与 OpenRouter 文档要求认证，Command Code 带上无害且与同一服务商的推理请求一致），脱敏规则不变。

**Verification:**

- [x] Rust `http_client`、`model_catalog`、`models`、`model_search`、`model_fetch` 测试（条数非零），使用本地模拟 HTTP 服务。
- [x] `check:frontend`、Rust fmt/Clippy/全部测试；`cargo tree --locked -i aws-lc-rs` 与 `-i native-tls` 为空。（2026-10-11：Rust 184 通过 / 1 ignored；`check:frontend` 413 UI 测试。）
- [x] 真实 Tauri 中由用户触发获取、勾选、重启后确认勾选保留，并确认日志无密钥。（2026-10-11 macOS：三家获取与勾选保存、重启后勾选保留、日志无密钥，维护者确认。）

**Dependencies:** P10,P11。

**Files likely touched:**

- `src-tauri/src/http_client.rs`
- `src-tauri/src/model_catalog.rs`
- `src-tauri/src/models.rs`
- `src-tauri/src/model_fetch.rs`
- `src/features/providers/ProviderModels.tsx`

**Estimated scope:** L：超过五个文件，按以下子任务拆分。
维护者决定（2026-10-10）：HTTP 使用 `reqwest 0.13.5`（`default-features = false`，特性 `rustls-no-provider` + `system-proxy`），直接依赖 `rustls 0.23`（`ring`、`std`、`tls12`）并在构建客户端前安装 ring 加密实现，不引入 aws-lc 或 native-tls；
读取系统代理；取消立即中断请求；新获取的模型默认不勾选；不单独做“测试连接”按钮（最小推理检查在 P24/P38）；获取后直接合并、不先预览；回环与私有地址继续允许；
P13、P14 并入本任务并保留占位标题；P12 只保存显示字段，完整能力字段在 P16。完整设计见维护者保存的 P12 计划；协议证据见 `docs/integrations/providers.md`「模型列表获取（P12）」。
D16（维护者批准，2026-10-10 完成）：2026-10-10 20:10（UTC+8）不带认证头请求 `GET https://api.commandcode.ai/provider/v1/models`，HTTP 200、17079 字节；结构为 `{object:"list", data:[…]}`，无分页字段，87 个模型，每个模型恰有 `id`、`object:"model"`、`created`（秒级时间戳）、`owned_by:"command-code"`、`name`、`context_length`、`supported_endpoints` 7 个字段；`supported_endpoints` 不带 `/v1` 前缀（与文档写法不同），只有三种组合：`["/chat/completions","/responses"]` 68 个、`["/messages"]` 11 个（均为 Claude）、`["/chat/completions"]` 8 个；65 个模型 ID 含 `/`。原样保存为 `src-tauri/tests/fixtures/command-code-models-2026-10-10.json`（不含密钥，已加入 `.prettierignore` 以保持原字节）。

### P12.0：拆分记录与证据核实

- [x] 写入子任务与维护者决定；按官方文档核实 G1–G3（Command Code、DeepSeek、OpenRouter 的列表结构）与 G9（内置 SQLite 带 JSON 函数），写入 `providers.md`；P13/P14 留占位标题，调整 P15/C15/P16/P24 依赖。G1 当时只部分确认，之后由 D16 的真实响应补全。

**Files:** `docs/plans/todo.md`、`docs/integrations/providers.md`、`docs/plans/development-plan.md`、`docs/plans/phase-1.md`。
**Verification:** Prettier 检查与差异检查。
**Dependencies:** P10、P11.a.4。

### P12.a.1：HTTP 传输层

- [x] 加入依赖（reqwest、rustls、`tokio` 的 `macros`/`time`/`sync`、`serde_json`）；只安装一次 ring 加密实现；`HttpSettings` + `ProxyMode`（生产为 `System`，`#[cfg(test)]` 的 `for_mock_server()` 为 `Disabled` 并调用 `.no_proxy()`，允许 `http://127.0.0.1`）；`build_client` 设置连接超时、不跟随重定向、只用 HTTPS 与 User-Agent；按上限逐块读取；`reqwest::Error` 只用于分类，不输出文本。
- [x] `TcpListener` 模拟服务器测试：超时、慢速响应、超大响应、连接中断、3xx、401/402/403/429/5xx，密钥不进入 URL 与错误；取消时服务器挂起，`select!` 立即返回，并且服务器观察到连接关闭。实现为 `HttpClient::new`/`get_bounded`、`run_cancellable`、`with_deadline` 与可复用的 `test_server`（14 个测试，另含 TLS 握手失败、拒绝明文 HTTP 与 URL 内凭据）。

**Files:** `src-tauri/src/http_client.rs`（新）、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`。
**Verification:** `cargo test --manifest-path src-tauri/Cargo.toml --locked http_client`（条数非零）、全部测试、fmt、Clippy；`cargo tree --locked -i aws-lc-rs` 与 `-i native-tls` 为空；在 `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`（含小写）指向 `http://127.0.0.1:9`、`NO_PROXY` 为空时重跑 `http_client` 测试，仍全部通过。
**Dependencies:** P12.0。

### P12.a.2：模型列表解析

- [x] DeepSeek（`object`/`data[].id/name/context_window/max_output_tokens/input_modalities/output_modalities`）与 OpenRouter（`data`/`total_count`/`links.next`，`limit=500`、自己累加 `offset`、不跟随 `links.next` URL、最多 10 页）解析；跳过无效 ID 并计数、同 ID 保留第一条；Command Code（`object`/`data[].id/name/context_length/supported_endpoints`，单次请求无分页；`supported_endpoints` 去掉可选的 `/v1` 前缀后与 `/chat/completions`、`/responses`、`/messages` 比较，缺少该字段时路由为 `unknown`）。用 D16 夹具测试：87 个模型、65 个 ID 含 `/`、三种路由组合 68/11/8、`chat_completions` 协议下 76 个可勾选与 11 个不可勾选；另用合成数据确认带 `/v1` 前缀的值同样识别。实现为 `model_catalog.rs`：三家解析函数、`route_support`、`models_url`/`openrouter_page_url`、`CatalogCollector`（去重、5000 上限、不完整原因）与 `OpenRouterPager`（14 个测试）；`providers.rs` 中过时的 P13 注释已更新。

**Files:** `src-tauri/src/model_catalog.rs`（新）、`src-tauri/src/lib.rs`、`src-tauri/src/providers.rs`（`is_hidden_format_character` 改为 `pub(crate)`）、`src-tauri/tests/fixtures/command-code-models-2026-10-10.json`（已存在，只读）。
**Verification:** `cargo test ... model_catalog`（条数非零）、fmt、Clippy。
**Dependencies:** P12.0。

### P12.a.3：schema v7 与模型存储

- [x] v7 迁移 `provider_model`（主键 `(provider_id, model_id)`，外键级联，`source`、`selected`、显示字段、`last_seen_at`/`missing_since`）与 `provider_model_fetch`；无查询的稳定游标列表（`ORDER BY model_id`，游标为最后一个 `model_id` 的 UTF-8 字节小写十六进制编码，不用分隔符拆分，因此含 `/`、`.`、`:` 的 ID 都安全）与「全部/已勾选」筛选、批量勾选、手动添加与删除、`ModelError`。测试含跨页边界的带 `/`、`.`、`:` 的模型 ID 与损坏游标。

**Files:** `src-tauri/src/storage.rs`、`src-tauri/src/models.rs`（新）、`src-tauri/src/lib.rs`。
**Verification:** `cargo test ... storage`、`cargo test ... models`（条数非零）、fmt、Clippy。
**Dependencies:** P12.0。

> 完成（2026-10-10）：`models.rs` 提供 `list_provider_models`（无查询，`all`/`selected`，每页 1–200）、`set_models_selected`（单事务全成或全不成，每批 ≤500，勾选获取行按 D9 只允许 `supported`/`not_applicable`）、`add_manual_model`（trim 后复用 `is_valid_model_id`，别名复用 `validate_display_name`，默认勾选）、`delete_manual_model`（只删手动行）与 `ModelError`。`fetch_provider_models` 所需错误码与合并在 P12.a.4 增补。v5→v6 测试改为 `MIGRATIONS[..6]` 固定版本。验证：fmt、Clippy 通过；`storage::` 16、`models::` 10、全量 136 通过 1 忽略；`check:frontend` 通过。

### P12.a.4：合并与异步获取

- [x] 合并规则（完整与不完整获取）；`model_fetch.rs`：登记 → 读快照与密钥 → 下载 → `select!` 取消 → 过期检查 → 在一个事务中合并；`ModelFetchRegistry` 拒绝同一 Provider 的并发获取；`#[tokio::test]` 端到端测试使用 `FakeStore` 与模拟服务器，并检查数据库文件中没有密钥。

**Files:** `src-tauri/src/model_fetch.rs`（新）、`src-tauri/src/models.rs`、`src-tauri/src/lib.rs`；另有两处小改：`src-tauri/src/model_catalog.rs`（`models_url` 无法解析时改为 `invalid_stored_provider`）、`src-tauri/src/http_client.rs`（`run_cancellable`/`with_deadline` 的错误类型改为泛型 `E: From<HttpError>`）。
**Verification:** `cargo test ... model_fetch`、`cargo test ... models`（条数非零）、全部测试、fmt、Clippy。
**Dependencies:** P12.a.1、P12.a.2、P12.a.3。

> 完成（2026-10-10）：`model_fetch.rs` 分为 `ModelFetchRegistry`（每个 Provider 只允许一次获取；`cancel` 只在下载阶段返回 `true`，`enter_merge` 之后返回 `false`）、`prepare_fetch`（在锁内读取 `kind`/`protocol`/`base_url`/`revision` 与凭据引用，释放锁后再读密钥）、`download_catalog`（三家都带 Bearer，OpenRouter 用 `OpenRouterPager` 分页；`run_cancellable(with_deadline(90 s, …))`）、`merge_catalog`（一个 `IMMEDIATE` 事务：`revision` 或 `base_url` 变化返回 `model_fetch_stale`，Provider 已删除返回 `not_found`；然后合并并写入 `provider_model_fetch`）、`fetch_provider_models`（依次执行以上步骤）和 `fetch_status`。P12.b.1 用 `spawn_blocking` 包装快照与合并两步。`ModelFetchSummary.skippedInvalid` 包含无效 ID 与同次重复 ID。
> `ModelError` 当前错误码：`storage_unavailable`、`read_failed`、`write_failed`、`operation_failed`、`invalid_request`、`not_found`、`invalid_stored_provider`、`invalid_stored_model`（a.3 新增）、`model_id_invalid`、`model_alias_invalid`、`model_already_exists`、`model_not_found`、`model_route_not_supported`，以及 a.4 新增的 `secret_missing`、`secret_invalid`、`base_url_invalid`、`credential_store_unavailable`、`credential_store_access_denied`、`credential_store_failed`、`model_fetch_in_progress`、`model_fetch_cancelled`、`model_fetch_stale`、`connection_failed`、`tls_failed`、`request_timed_out`、`auth_rejected`、`insufficient_balance`、`rate_limited`、`upstream_unavailable`、`upstream_response_invalid`、`response_too_large`。网络错误码与 `HttpError::code()` 一致（有测试）。P12.b.2 的错误码白名单以此为准。
> 验证：fmt、Clippy 通过；`model_fetch` 14、`models::` 10、`model_catalog` 14、`http_client` 14，全量 150 通过 1 忽略；代理变量指向 `http://127.0.0.1:9` 时 `model_fetch` 与 `http_client` 28 个测试通过；`check:frontend` 通过。

### P12.a.5：模型列表模糊搜索

- [x] `model_search.rs`：规范化（Unicode 小写；`-`、`_`、`/`、`.` 与空白视为同一分隔符；只按分隔符切词，不按字符切分中文）、单词档位（exact 3 / prefix 2 / contains 1 / subsequence 0）、多词排序键（整体精确匹配、最弱档位、档位之和、`model_id`）。`models.rs` 接入 `query`：SQLite 只按 Provider 和勾选状态过滤，不用 `LIKE`；内存打分排序后返回前 `limit` 条与 `totalMatches`，`nextCursor` 为 `null`；规范化后为空的查询按无查询处理；查询超过 200 个字符，或有查询时 `after` 非空，返回 `invalid_request`。
- [x] 测试覆盖：中文显示名称（“求索”包含、“深求”子序列，“深度求索”是一个词）；空查询、全空白与只有分隔符的查询等同无查询；`%`、`_`、`\`、引号与正则元字符按字面处理，不会匹配全部或报错；分隔符等价；多词 AND（“ds chat” 命中 `deepseek-chat`，不命中 `deepseek-reasoner`，不同词可命中不同字段）；档位顺序与同分时的稳定顺序；200 个字符通过、201 个字符被拒（按字符计数）；匹配数超过 `limit` 时截断并返回总数；与「已勾选」筛选组合；5000 行冒烟测试。

**Files:** `src-tauri/src/model_search.rs`（新）、`src-tauri/src/models.rs`、`src-tauri/src/lib.rs`。
**Verification:** `cargo test ... model_search`、`cargo test ... models`（条数非零）、fmt、Clippy。
**Dependencies:** P12.a.3。

> 完成（2026-10-10）：`model_search.rs` 提供 `SearchQuery::parse`（按字符计数，超过 200 个返回错误；规范化后为空返回 `None`）、`SearchQuery::score`（每个词取各字段中的最高档位，`MatchScore` 依次比较 `full_exact`、`weakest`、`sum`）与 `sort_ranked`（同分按 `model_id` 字节序）。`ListProviderModelsRequest` 增加可省略的 `query`；`ProviderModelPage` 增加 `totalMatches`（无查询时为 `null`）。有查询时 SQLite 只按 `provider_id` 与 `selected` 过滤，不用 `LIKE`，查询文本不进入 SQL。验证：fmt、Clippy 通过；`model_search` 7、`models::` 16，全量 163 通过 1 忽略；5000 行 3 次查询在 debug 构建中约 145 ms（测试上限 10 s，只防止平方级退化）；`check:frontend` 通过。

### P12.b.1：Tauri 命令

- [x] 薄命令 `fetch_provider_models`（async，`select!` 取消；数据库与凭据操作放在 `spawn_blocking`）、`cancel_provider_model_fetch`、`get_provider_model_fetch_status`、`list_provider_models`、`set_provider_models_selected`、`add_manual_provider_model`、`delete_manual_provider_model` 与注册。

验证：Rust fmt、Clippy（all-targets、零警告）与全量测试通过（182 通过、1 个真实 OS 凭据测试忽略）。获取前的凭据读取与替换共用进程内锁；合并任务持有注册令牌直到结束，避免调用方停止等待后提前允许第二次获取。未使用真实密钥或执行原生 IPC 验收；前端包装与模型界面仍待 P12.b.2–c.4。

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`。
**Verification:** Rust fmt/Clippy/全部测试。
**Dependencies:** P12.a.4、P12.a.5。

### P12.b.2：模型 TS 包装

- [x] `models.ts` 运行时校验响应（严格键集合）、错误码白名单；浏览器预览列表返回 `{ kind: "preview" }`，写入与获取抛 `desktop_required`，取消返回 `{ wasRunning: false }`。

验证：新增 85 项模型 IPC 边界测试；`check:frontend` 通过。未验证真实 WebView IPC 与上游请求；模型界面后续在 P12.c 完成。

**Files:** `src/lib/desktop/models.ts`（新）、`src/lib/desktop/models.test.ts`（新）。
**Verification:** `pnpm run test:ui src/lib/desktop`、`pnpm run check:frontend`。
**Dependencies:** P12.b.1。

### P12.c.1：模型列表与勾选（Frontend Developer）

- [x] `ProviderModels` 列表：搜索框（防抖；有查询时按相关度排序并显示“共 N 个匹配，显示前 M 个”）、「全部/已勾选」筛选、无查询时加载更多、复选框、上游消失与路由标记，逐行保存与失败回滚。编辑页已挂载「模型」分组；`check:frontend` 415 项 UI/边界测试通过。

**Files:** `src/features/providers/ProviderModels.tsx`（新）、`ProviderModels.module.css`（新）、`ProviderModels.test.tsx`（新）、两份 locale。
**Verification:** `pnpm run test:ui src/features/providers`、`pnpm run check:frontend`。
**Dependencies:** P12.b.2、P11.c.3。

### P12.c.2：获取、取消与错误状态（Frontend Developer）

- [x] 「获取模型」与「取消」（搜索框后主按钮、36 px 行高）、`aria-busy`、`role="status"` 播报、摘要通知、不完整提示与各错误码文案；页面隐藏时取消并丢弃晚到结果；无密钥时引导到 API 配置。手动添加与部分重试路径仍随 c.3。

**Files:** `src/features/providers/ProviderModels.tsx`、`ProviderModels.test.tsx`、两份 locale。
**Verification:** 同 P12.c.1。
**Dependencies:** P12.c.1。

### P12.c.3：手动添加（Frontend Developer）

- [ ] `ManualModelForm`：模型 ID 与可选别名，字段级错误，删除手动行。代码与自动检查已完成；真实 Tauri 手动添加→重启→取消勾选并保存删除、外观验证仍待维护者确认。

**Files:** `src/features/providers/ManualModelForm.tsx`（新）、`ManualModelForm.test.tsx`（新）、`ProviderModels.tsx`、两份 locale。
**Verification:** 同 P12.c.1。
**Dependencies:** P12.c.2。

### P12.c.3.a：手动添加表单

- [x] 模型 ID、可选别名、字段错误、提交防重与未知结果重读；双语资源与行为测试。

**Files:** `ManualModelForm.tsx`、`ManualModelForm.test.tsx`、两份 locale；复用现有表单样式。
**Verification:** 目标行为测试、`check:frontend`。
**Dependencies:** P12.c.2。

### P12.c.3.b：已选列表接线与删除验证

- [x] 已选视图添加并保存、刷新选中基线和数量；有草稿或请求时阻止添加；手动行取消勾选后沿用批量保存删除。

**Files:** `ProviderModels.tsx`、`ProviderModels.test.tsx`。
**Verification:** 列表行为测试、`check:frontend`。
**Dependencies:** P12.c.3.a。

### P12.c.3.c：说明与验收边界

- [x] 更新前端说明、UX 约定与英文 changelog；真实 Tauri/外观验证单独保留待验。

**Files:** `docs/frontend.md`、`docs/plans/models-tab-ux.md`、`CHANGELOG.md`；本清单随子任务更新。
**Verification:** 格式与差异检查。
**Dependencies:** P12.c.3.b。

### P12.c.5：模型 Tab 双视图与保存式勾选（维护者 2026-10-11 定稿）

完整交互见 [models-tab-ux.md](models-tab-ux.md)。取代 c.1 的「全部/已选」、逐行保存与获取后整表 merge 的 UI 语义。

- [x] **已选视图（默认）**：只读持久化勾选；一次加载全部；工具栏「已选」+ **保存** + **获取模型**；去掉「全部」筛选。
- [x] **上游视图**：点「获取模型」进入；上游分页懒加载；已持久化的 ID 自动钩上；勾选仅 draft，**保存** 后写库并回到已选视图。
- [x] **已选视图保存**：取消勾选 → 保存 → 从本地删除对应模型行（`save_provider_model_selections` + `remove_model_ids`）。
- [x] **未保存离开**：draft 勾选时切 Tab / 返回 → 双语「未保存」确认（`modelsBusy` 仍阻塞进行中请求）。
- [x] **搜索**：已选 = `list_provider_models` + `query`；上游 = `browse_upstream_models_page` + `query`（Rust 会话缓存内排序分页，与已选同一套模糊规则）。
- [x] Rust：`browse_upstream_models_page`、`save_provider_model_selections`；`fetch_provider_models` merge 仍供测试/旧路径，UI 热路径走 browse+save。
- [x] 文档与 `architecture.md` 反映 browse+save 热路径；维护者 Tauri 三家走通 models-tab-ux 流程 1→2→3（已选 → 获取上游 → 保存 → 已选取消勾选保存；含搜索与懒加载）。（2026-10-11 macOS，维护者确认。）

**Files:** `ProviderModels.tsx`、`ProviderModels.test.tsx`、`src-tauri/src/models.rs`、`model_fetch.rs`、`commands.rs`、`src/lib/desktop/models.ts`、locale、`docs/plans/models-tab-ux.md`。
**Verification:** 同 P12.c.1 + 新行为测试；维护者三家 Tauri 走通 1→2→3 流程（已完成）。
**Dependencies:** P12.c.2（c.3 手动添加可并行或稍后接入同一保存模型）。

### P12.c.4：接入详情页（Frontend Developer）

- [x] 新建/编辑共用 `ProviderTabbedView`（API / 模型页签、创建门闩、API 页 Save 在卡片上方工具栏、返回刷新列表行）；二级页头（返回 + 右对齐品牌标题）与各模块主页 `PageModuleHeader` 已统一。页面隐藏或返回时取消获取（c.2）。手动添加模型仍待 c.3。

**Files:** `src/features/providers/ProvidersView.tsx`、`ProvidersView.test.tsx`、`src/App.test.tsx`。
**Verification:** 同 P12.c.1。
**Dependencies:** P12.c.3。

### P12.d：文档与真实验证

- [x] 更新架构/计划现状与英文 changelog；完整自动检查与 `tauri build --no-bundle`。（2026-10-11：已更新 `architecture.md`、`development-plan.md`、`phase-1.md`、`CHANGELOG.md`；`check:frontend` 与 `pnpm run tauri build --no-bundle -- --locked` 通过。）
- [x] 维护者在实际 Tauri 中对三家各做一次由用户触发的获取，确认 G4（真实响应大小）、Command Code 结构与 D16 夹具一致，以及 DeepSeek 不带密钥时的行为；Windows/Linux 未测时在此注明。
      维护者于 2026-10-11 在本机 Tauri（macOS）对 Command Code、DeepSeek、OpenRouter 各完成一次「获取模型」并成功保存模型勾选；Windows/Linux 与本项中的 DeepSeek 无密钥对照未在本轮执行。

**Files:** `docs/architecture.md`、`docs/plans/development-plan.md`、`docs/plans/phase-1.md`、`docs/integrations/providers.md`、`CHANGELOG.md`。
**Verification:** 完整检查与实际 Tauri 运行。
**Dependencies:** P12.c.4。

### Checkpoint C12（P10–P12）

- [ ] P10–P12 均已勾选（**P12.c.3 手动添加代码/自动检查完成，真实 Tauri 验证待验**；P12.c.5 与 P12.d 获取/勾选 Tauri 验证已完成）。

### Task P13: 验证 DeepSeek 连接（已并入 P12）

维护者决定（2026-10-10）：并入 P12。超时、取消、脱敏、模拟 HTTP 服务测试和“只在用户触发时请求”的要求由 P12 承接；“获取模型”成功即说明列表请求成功，不单独做连接检查按钮；由用户触发的最小推理检查放在 P24/P38。保留本标题以免旧引用失效。

### Task P14: 接入 OpenRouter 模型发现（已并入 P12）

维护者决定（2026-10-10）：并入 P12，删除与 P12 重复的模型发现任务。OpenRouter 的分页（每页 500、最多 10 页、每个 Provider 最多 5000 个模型）、勾选与手动补充都在 P12 完成；完整能力与参数字段映射在 P16。保留本标题以免旧引用失效。

### Task P15: 接入 Command Code

**Description:** 使用 P01 的准确接入约定新增服务商适配，复用账户与 Model 页面。

**Acceptance criteria:**

- [ ] 核实 Pro 与 Provider 套餐的 API 访问、模型权限与路由证据；P01 只覆盖 Command Code 的 GOAT 套餐，未核实前不套用其结论。
- [ ] 按照其真实认证、协议和扩展字段保存与验证，不凭名称猜 URL。
- [ ] 基于 P12 获取的 `supported_endpoints` 实现按模型的协议路由（Responses/Anthropic Messages 覆盖）；手动模型清楚说明能力信息来源。
- [ ] 未核实的特有行为保持不可用，不写死为 OpenAI 兼容或伪造成功。

**Verification:**

- [ ] 运行 Rust `command_code` 模拟服务测试与表单行为测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实连接依赖准确文档和明确提供的测试凭据，未满足时保留待验状态。

**Dependencies:** P01,P12。

**Files likely touched:**

- `src-tauri/src/providers/command_code.rs`
- `src-tauri/src/providers.rs`
- `src/lib/desktop/providers.ts`
- `src/features/providers/ProviderForm.tsx`
- `src/features/providers/ProviderForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

### Checkpoint C15（P15；P13/P14 已并入 P12）

- [ ] P15 已勾选（P13、P14 已并入 P12，随 C12 检查）。

### Task P16: 校验 Provider 特有参数

**Description:** 补充模型字段表单与注入前校验，确保不同 Provider 的推理/模态设置没有错误等价映射。

**Acceptance criteria:**

- [ ] 根据协议声明允许的字段类型、枚举和范围，区分默认、继承与显式覆盖。
- [ ] 保留可验证的自定义请求头/参数；拒绝危险认证覆盖、未知注入字段和不支持的值。
- [ ] UI 显示能力信息来源与具体校验原因，不因手动标签就宣称实际支持。
- [ ] 承接 P12 未做的部分：模型别名编辑、手动能力覆盖，以及 后续 schema（当前预留 v9）的能力/参数字段（如 DeepSeek `effort`、OpenRouter `supported_parameters`）。

**Verification:**

- [ ] 运行 Rust `model_parameters` 测试，覆盖 effort/预算、图像、工具与输出限制的差异。
- [ ] 运行 ModelForm 测试、`check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P12,P15。

**Files likely touched:**

- `src-tauri/src/models.rs`
- `src-tauri/src/providers.rs`
- `src/lib/desktop/models.ts`
- `src/features/models/ModelForm.tsx`
- `src/features/models/ModelForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

## 插入任务 BR：后端职责拆分

维护者提出未来 GitHub 配置备份、MCP/Skill 来源安装、给 Agent 的 API 转发，可能直接安装 Agent，以及 Vercel/更多模型服务商接入，优先规划模块边界。
设计见 [backend-modularity.md](backend-modularity.md)。规划已推送，按下列小批任务迁移；勾选仅表示该批次已实施并完成适用回归。
本轮完成 BR1–BR4；BR1/BR2 已完成，BR3/BR4 继续拆分 Provider/MCP 与共享校验。BR5–BR11 按新增功能需求开展。
表中范围包含本清单更新；若实际 import/测试 fixture 迁移超出五个文件，先继续拆批。

- [x] **BR0：检查源码并记录规划。** 基线 `165e262`，记录大小、职责、实际耦合、扩展边界和兼容要求。Files：本清单、`backend-modularity.md`、`architecture.md`、`development-plan.md`；Verification：源码核对、Markdown 格式与 diff 检查；无业务代码变更。

| 待办        | 一次迁移的职责                                                 | 建议实际文件范围（另含本清单）                                                                               | 依赖  |
| ----------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ----- |
| [x] BR1.a   | 原样迁出 Provider/Model 内联测试                               | `providers.rs`、`providers/tests.rs`、`models.rs`、`models/tests.rs`                                         | BR0   |
| [x] BR1.b   | 原样迁出 Fetch/Catalog 内联测试                                | `model_fetch.rs`、`model_fetch/tests.rs`、`model_catalog.rs`、`model_catalog/tests.rs`                       | BR1.a |
| [x] BR1.c   | 原样迁出 Storage/Credentials 测试；保留跨模块 fake helper 入口 | `storage.rs`、`storage/tests.rs`、`credentials.rs`、`credentials/tests.rs`                                   | BR1.b |
| [x] BR1.d   | 原样迁出 HTTP/Provider Secret 测试；保留本地服务 fixture       | `http_client.rs`、`http_client/tests.rs`、`provider_secrets.rs`、`provider_secrets/tests.rs`                 | BR1.c |
| [x] BR2.a   | 命令状态/凭据锁和 app/log 入口拆分                             | `commands.rs`、`commands/state.rs`、`commands/app.rs`、`lib.rs`                                              | BR1   |
| [x] BR2.b   | Provider 和 Models 命令按功能拆分                              | `commands.rs`、`commands/providers.rs`、`commands/models.rs`、`lib.rs`                                       | BR2.a |
| [x] BR2.c   | 偏好和 MCP 命令按功能拆分                                      | `commands.rs`、`commands/preferences.rs`、`commands/mcp.rs`、`lib.rs`                                        | BR2.b |
| [x] BR2.d   | 记录模块布局偏好、原生回归与本轮完成边界                       | `CHANGELOG.md`、`docs/architecture.md`、`docs/plans/backend-modularity.md`、`docs/plans/development-plan.md` | BR2.c |
| [x] BR2.e   | 今日收尾：统一 README/阶段概览/架构与任务状态                  | `README.md`、`docs/plans/phase-1.md`、`docs/architecture.md`                                                 | BR2.d |
| [x] BR3.a   | Provider 类型/错误从实现中提取；保留原入口                     | `providers.rs`、`providers/types.rs`、`providers/tests.rs`                                                   | BR2   |
| [x] BR3.b.0 | Provider 类型通过公开模板访问器解耦                            | `providers/types.rs`                                                                                         | BR3.a |
| [x] BR3.b   | Provider 模板和专用校验拆分                                    | `providers.rs`、`providers/templates.rs`、`providers/validation.rs`、`providers/tests.rs`                    | BR3.a |
| [x] BR3.c   | Provider SQL 与保存编排拆分                                    | `providers.rs`、`providers/repository.rs`、`providers/service.rs`、`providers/tests.rs`                      | BR3.b |
| [ ] BR4.a   | MCP DTO/错误和专用字段校验拆分                                 | `mcp.rs`、`mcp/types.rs`、`mcp/validation.rs`、`mcp/tests.rs`                                                | BR3   |
| [ ] BR4.b   | MCP 查询/SQL 与凭据保存/清理编排拆分                           | `mcp.rs`、`mcp/repository.rs`、`mcp/service.rs`、`mcp/tests.rs`                                              | BR4.a |
| [ ] BR4.c   | 提取共享文字校验，分别映射业务错误；保留 Provider 公开校验入口 | `providers/validation.rs`、`mcp/validation.rs`、`shared.rs`、`lib.rs`                                        | BR4.b |
| [ ] BR4.d   | MCP 独立身份类型与通用 ID 格式校验，保持原有 ID 字符串         | `shared.rs`、`mcp/types.rs`、`mcp/validation.rs`、`mcp/repository.rs`                                        | BR4.c |
| [ ] BR4.e   | 共享时间来源与业务错误映射，保留当前 Provider 时间入口         | `shared.rs`、`providers.rs`、`commands/state.rs`、`commands/models.rs`                                       | BR4.d |

BR1/BR2/BR3 等依赖名表示该组全部子任务。具体入口仍采用原有 `.rs` facade，
统一使用 `feature.rs` + `feature/` 的模块布局，不引入 `mod.rs`。共享格式函数只做格式判断，不统一业务身份类型。
BR4.e 若实际 MCP/Provider 命令调用点也需要编辑，按功能继续拆，不能超过文件范围。

BR1/BR2 验证：测试清单与基线完全相同，Rust fmt/Clippy 与 201 项测试通过（1 项忽略）；29 个注册命令名/顺序不变。完整前端 468 项 UI/边界、8 项 release、14 项 i18n checker 和锁定 debug/release 桌面构建通过。隔离 Linux Tauri 用无密钥 Provider fixture 完成偏好/Provider 编辑、手动模型添加与批量删除、缺密钥/取消状态、MCP 保存编辑与重启重读；未执行服务器命令。真实 OS 凭据与其他平台待验项保持原状态。

后续较大批次的细项在开始时按同样约定继续拆，不一次迁移全部相关文件：

- [ ] **BR5：Models/Fetch。** 分出本地类型/错误、查询与选择 SQL，明确合并接口；拆获取注册/取消、快照、下载、缓存浏览与合并编排。保持现有 ModelError IPC 码，消除获取层对私有 `models::lock` 的借用。开始前按上述职责拆 BR5.a 等批次。
- [ ] **BR6：Catalog/Storage/Credentials。** 各厂商解析局部化；迁移注册顺序和已发布 SQL 保持原样；按需要分离 OS store 实现和各业务补偿策略。开始前分别拆解析、迁移、凭据批次。
- [ ] **BR7：外部服务适配。** GitHub 客户端/账号支持备份目标操作；Vercel 用途待确定。以真实复用需求提取 HTTP transport，保留服务专用权限/限流/错误语义。开始实施前按认证与客户端等职责拆细项。
- [ ] **BR8：配置备份恢复设计。** 明确版本化非敏感快照、GitHub 目标/路径、远端修订冲突、本机身份/凭据重新关联与恢复差异；先显式备份恢复，自动同步另行规划。实施前拆导出、本地恢复、GitHub 存储和 UI 的具体小任务。
- [ ] **BR9：MCP/Skill 来源安装设计。** 确定首个实际来源及契约，拆候选元数据、固定 revision、安装计划/执行/更新/卸载、受管路径和恢复；安装、运行与 Agent 部署状态分开。承接 P25–P30、P31–P33/P34，避免重复实现。
- [ ] **BR10：API 转发设计。** 明确首条同协议转发、gateway 生命周期、Provider/Model 路由、协议适配、流式/工具调用/取消/错误验收；Agent 通过本地 API 访问，Tauri 只管理服务。连接 P15/P16/P18–P24，不扩大为云端多用户代理。实施前拆最小路径任务。

- [ ] **BR11：Agent 安装设计。** 核实首个 Agent 的安装/升级/卸载契约、目标平台/架构与现有安装来源；受管安装记录与发现/能力/配置分开。承接 P17；安装完成后只检测，不自动启动或应用配置。按实际首条路径拆小任务，保护用户已有安装与配置。

**各代码批次的共同验收：** 原有公开业务入口、IPC 命令名/serde/错误码、schema v8、
OS credential service name/引用、并发和补偿行为不变；运行 Rust fmt、all-targets Clippy
零警告、完整测试并核对基线 201 通过/1 项忽略。命令/导出/资源移动额外执行完整
`check:frontend`、锁定桌面构建及适用隔离 IPC 验证。原平台待验项保持待验。

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
- [ ] 提供由用户触发、提示可能计费的最小推理检查（原 P13 的连接验证意图），区分网络、密钥、套餐与模型请求失败。

**Verification:**

- [ ] 运行 Rust `compatibility` 映射样本测试与 UI 切换测试。
- [ ] 运行完整项目检查；真实用例仅使用许可的测试凭据。

**Dependencies:** P12,P15,P16,P20,P23。

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

- [x] 支持经过核实的 stdio/HTTP 字段，校验命令参数、URL、env/header 和传输类型。
- [x] 敏感字段使用系统凭据引用，普通列表/差异/错误不暴露值；查询有界分页。
- [x] 保存、编辑、禁用可重读；显示与保存不启动进程或发送认证请求。本轮无导入，导入留待独立任务。

**Verification:**

- [x] 运行 Rust `mcp_definition` 字段/敏感值测试与 McpForm 行为测试。
- [x] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [x] Tauri 保存包含测试命令的定义，确认没有进程启动。

**Dependencies:** P05,P06；中央管理不依赖 Agent 发现 P17。

**Files likely touched:**

- `src-tauri/src/mcp.rs`
- `src-tauri/src/commands.rs`
- `src/lib/desktop/mcp.ts`
- `src/features/mcp/McpForm.tsx`
- `src/features/mcp/McpForm.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

### P31.0：提前中央 MCP 管理与字段边界

- [x] 维护者授权先做中央 MCP 管理；P32/P33 注入继续保留原安全写入依赖。
- [x] 核对 Pi/Grok 官方 stdio/HTTP 字段；仅存定义，不启动进程、不发认证请求。env/header 值全部按敏感值处理；本轮不做 OAuth、旧 SSE、动态值命令、导入或 Agent 写入。

**Files:** 本清单、`docs/plans/mcp-central.md`。
**Verification:** 官方来源与范围核对。

### P31.a：Rust 校验、迁移与安全保存

- [x] 结构化定义、有界分页、稳定 ID、修订冲突、凭据引用与多资源失败补偿；旧凭据清理失败可重试。

**Files:** `src-tauri/src/mcp.rs`、`src-tauri/src/mcp/tests.rs`、`src-tauri/src/storage.rs`、`src-tauri/src/lib.rs`。
**Verification:** MCP/迁移行为测试、Rust fmt/Clippy/完整测试。
**Dependencies:** P31.0。

### P31.b：Tauri 与 TS 边界

- [x] 注册只读/保存命令，SQLite/凭据访问使用 blocking pool；严格验证非敏感响应、稳定错误码与浏览器预览。

**Files:** `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src/lib/desktop/mcp.ts`、`src/lib/desktop/mcp.test.ts`。
**Verification:** IPC 边界测试、前端检查、Rust 检查。
**Dependencies:** P31.a。

### P31.c.1：表单与双语资源

- [x] stdio/HTTP 表单、动态 env/header 字段、编辑保留/替换/删除、字段错误、凭据输入清理和未知结果重读。

**Files:** `McpForm.tsx`、`McpForm.test.tsx`、`Mcp.module.css`、两份 locale。
**Verification:** 表单行为测试、`check:frontend`。
**Dependencies:** P31.b。

### P31.c.2：列表、启停与 App 接线

- [x] MCP 页面新建/编辑、稳定分页、中央定义状态、错误重试与清理重试；App 页面与隐藏生命周期接线。

**Files:** `McpView.tsx`、`McpView.test.tsx`、`src/App.tsx`、`src/App.test.tsx`；本清单随子任务更新。
**Verification:** 页面/导航行为测试、`check:frontend`。
**Dependencies:** P31.c.1。

### P31.c.3：与现有页面统一样式和交互

- [x] 按维护者补充要求，复用 Providers 的二级标题/返回和按钮样式；保存放在卡片顶部，字段高度、列表间距与现有页面一致。

**Files:** `McpForm.tsx`、`McpForm.test.tsx`、`McpView.tsx`、`Mcp.module.css`、两份 locale（双语作为一个资源变更）。
**Verification:** 完整前端检查；外观保持人工验收。
**Dependencies:** P31.c.2。

### P31.d：文档、构建与原生验证

- [x] 更新架构/前端/计划和英文 changelog；完整自动检查与桌面构建。
- [x] 实际 Tauri 保存测试命令定义→重启→重读，确认没有执行服务器；真实凭据库与各平台人工验证单独记录。

**Files:** `docs/architecture.md`、`docs/frontend.md`、`docs/plans/phase-1.md`、`docs/plans/development-plan.md`、`CHANGELOG.md`。
**Verification:** 完整检查、锁定桌面构建；未执行的真实验收保持待验。
**Dependencies:** P31.c.2。

**本轮验证结果：** Linux 完整前端检查 468 项 UI/边界测试、8 项 release、14 项 i18n checker；Rust fmt/Clippy 与 201 项测试通过（1 项真实凭据库测试忽略）；锁定 debug/release 桌面构建通过。隔离真实 Tauri 完成 stdio 无凭据定义保存/编辑/列表→重启→重读，测试命令未执行。构建的单包大小提示单独记录，不作为外观验收。

- [ ] MCP 多值真实系统凭据库存取/替换/清理，以及 macOS/Windows 原生和人工外观验收；不以模拟存储测试代替。

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

## 插队任务 I15：Base UI 下拉交互

维护者批准引入无样式 Base UI，先只替换共享 FieldSelect，不重写整站。
设计与风险见 [base-ui-select.md](base-ui-select.md)。

### I15.a：稳定依赖

- [x] 核对稳定 `@base-ui/react 1.9.0`、MIT 许可证、React 19 兼容性与 Select API；固定版本并更新锁文件，不安装可选日期依赖，不降低发布年龄策略。

**Files:** `package.json`、`pnpm-lock.yaml`；迁移方案与本清单同步记录。
**Verification:** 官方文档、npm 元数据、已安装许可证和类型声明；冻结安装。

### I15.b：共享组件与回归测试

- [x] 保留控件外观、图标、语言属性和受控值；用 Base UI 处理键盘、焦点、定位与外部关闭。保存期间原生禁用与保留焦点的 blocked 语义不变。
- [x] 弹层通过 Portal 避免卡片裁切；更新共享测试辅助函数，补键盘、关闭、锁状态变化和多实例测试。

迁移测试发现单次合成 click 不能表示 Base UI 的完整指针选择和异步焦点恢复，因此增加仅测试用 `@testing-library/user-event 14.6.7`（稳定、MIT），并将操作辅助函数改为可等待。按实际影响拆分：

- I15.b.1：共享组件、菜单样式、弹层层级 token 和组件行为测试（`FieldSelect.tsx`、`FieldSelect.module.css`、`FieldSelect.test.tsx`、`src/App.css`）。
- I15.b.2：测试依赖、Portal 辅助函数和服务商调用方回归（`package.json`、`pnpm-lock.yaml`、`src/test/fieldSelect.ts`、`ProviderForm.test.tsx`、`ProvidersView.test.tsx`）。
- I15.b.3：等待真实选择操作的设置/启动/壳层回归（`LanguageSelector.test.tsx`、`AppearanceControl.test.tsx`、`LocaleStartup.test.tsx`、`src/App.test.tsx`）；不修改业务保存行为。

**Verification:** 组件与调用方行为测试、严格类型与 Lint。
**Dependencies:** I15.a；b.2 在 b.1 后，b.3 在 b.2 后。

### I15.c：完整检查与说明

- [x] 完整前端检查、适用 Rust 检查和 macOS 桌面构建；更新前端说明与英文 changelog，仅记录实际验证。
- [x] 维护者人工确认本轮下拉迁移无问题；该确认不代替专门的读屏或 Windows/Linux 验证。未经另行要求不运行浏览器或截图视觉检查。

**Files:** `docs/frontend.md`、`CHANGELOG.md`；迁移方案与本清单同步记录。
**Verification:** `check:frontend`、冻结安装、Rust fmt/Clippy/tests、locked no-bundle build；原生人工验收单独记录。
**Dependencies:** I15.b。

自动检查通过：405 项 UI/边界测试（FieldSelect 14 项）、14 项翻译检查测试、8 项发布测试；冻结安装、格式、零警告 Lint、两份 TypeScript 配置、Rust fmt/Clippy、182 项 Rust 测试（1 项真实凭据测试忽略）和 locked macOS no-bundle build。维护者已人工确认本轮迁移无问题；未运行截图检查，专门的读屏和 Windows/Linux 运行验证仍待验。
