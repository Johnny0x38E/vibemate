# vibemate 第一阶段执行清单

这里是唯一任务状态来源。已完成任务只保留摘要；实现过程、旧文件清单和逐次测试计数从 Git 历史、CI 与 `CHANGELOG.md` 查阅，不另存重复归档。

## 当前顺序与范围

1. 更新文档并提交 push，再实施六 Agent 固定目录与只读发现（P17.i–P17.n）。
2. 接着做 P18 最小 Pi 配置预览；P16 只按预览需要补模型信息，不是完整前置。
3. 请求参数与必要能力覆盖属于目标 Agent 配置，不放在服务商模型页。
4. 六种发现不等于六种配置注入；第一阶段配置适配仍为 Pi、Grok Build。

## 执行约定

- 按依赖顺序实施，一次一个可检查行为；不使用未授权子代理。
- 每批最多约五个实际文件，测试、资源、注册和迁移也计数；超出先拆批。
- UI 变更同时补中英文和可访问名称；保持既有严格检查，不用 suppression 绕过失败。
- 外观按维护者人工验收策略；原生行为、真实凭据和跨平台验证单独记录。
- Verification 使用 `AGENTS.md` 的适用命令；Rust 过滤测试必须实际运行非零用例。
- 勾选表示该项已验证；代码完成不等于真实安装、推理或其他平台验收完成。
- 任务完成后删除详细执行过程，保留仍未解决的限制；未来路径是建议，不提前创建空壳。

## 已完成摘要

| 任务          | 已交付内容                                            | 现行说明                                                                             |
| ------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------ |
| P00           | pnpm 与锁文件迁移                                     | 根目录 `AGENTS.md`                                                                   |
| P01–P03 / C03 | Provider 与 Pi/Grok 官方接入证据                      | [integrations](../integrations/compatibility.md)，实施前重新核实版本                 |
| P04–P06 / C06 | UI 行为测试、私有 SQLite、OS 凭据接口                 | [frontend](../frontend.md)、[architecture](../architecture.md)                       |
| P07–P09 / C09 | 中英文资源、偏好、启动与界面接线                      | [frontend](../frontend.md)                                                           |
| I02–I15       | 桌面壳、基础外观、共享表单与 Base UI 下拉             | [desktop-shell-design](desktop-shell-design.md)、[base-ui-select](base-ui-select.md) |
| P10–P12 / C12 | 服务商与密钥统一保存，模型获取/选择/搜索和手动模型    | 维护者已验收；[models-tab-ux](models-tab-ux.md)                                      |
| P13 / P14     | 模型发现任务已并入 P12                                | 最小推理检查仍在 P24/P38，不因合并而标为通过                                         |
| P15 / C15     | 按模型协议路由与三家多协议模板                        | 维护者确认真实 Command Code 凭据连接与界面验收                                       |
| BR0–BR5       | 测试、命令、Provider/MCP、Models/Fetch 与共享校验拆分 | [backend-modularity](backend-modularity.md)                                          |
| P17.a–P17.h   | Pi/Grok 只读发现、窗口内手动路径、IPC 和双语详情      | [agent-discovery](agent-discovery.md)；新增六种目录改造未实施                        |
| P31           | 中央 MCP 定义、凭据引用、编辑与启停                   | [mcp-central](mcp-central.md)；未执行或部署服务器                                    |

P15 的真实连接记录来自维护者确认，不代表代理重新执行，也不补写未提供的模型、协议或套餐细节。

## 已交付功能仍待验

- [ ] C06：维护者审阅基础存储/凭据阶段（可选记录意见）。
- [ ] I16：人工确认 macOS/Windows 系统风主题浅/深色；原生选择后重启和 Windows/Linux 显示仍待验。
- [ ] P17：真实 Pi/Grok 安装只读核对、macOS/Windows 与外观验收；既有隔离 Linux 样本不替代真实安装。
- [ ] P31：MCP 多值真实系统凭据库存取/替换/清理，以及 macOS/Windows 原生和人工外观验收。
- [ ] 平台专项：现有各功能的 Windows/Linux 原生行为、真实 OS 凭据与专门读屏验证；按 P36–P38 记录，不因清理已完成任务而勾选。

### Task P16: 补充中央模型能力元数据

**Description:** 补充目录能力解析、来源记录、保存与显示，以及独立的模型别名编辑；不在服务商页面配置请求参数。维护者确认思考选项、请求输出限制和必要的能力覆盖属于目标 Agent 配置，随 P18/P22 的已核实原生映射实现。

**Acceptance criteria:**

- [ ] 按已核实的目录规则解析能力，区分缺失、null、空列表与合法值；未知不显示为已支持。
- [ ] 保存与显示能力来源；重新获取保留用户别名，不改变各 Agent 的使用设置。
- [ ] 提供有并发保护的模型别名编辑；影响配置预览的元数据更新可检测，过期预览不可直接应用。
- [ ] 承接 P12 未做的目录能力字段（如已核实的 OpenRouter `supported_parameters`）；DeepSeek 扩展证据不足保持待验，需要迁移时追加版本，v9 不得改写。

**Verification:**

- [ ] 运行目录/模型/存储行为测试，覆盖能力未知、畸形字段、迁移保留、重新获取保留别名和并发/过期检查。
- [ ] 运行受影响 UI 与 IPC 测试、`check:frontend` 和 Rust fmt/Clippy/测试；请求参数校验在 Agent 任务验证。

**Dependencies:** P12,P15。

**Files likely touched:**

- `src-tauri/src/model_catalog.rs` 与解析测试
- `src-tauri/src/models/` 与模型测试
- `src-tauri/src/storage.rs` 与迁移测试
- `src/lib/desktop/models.ts` 与边界测试
- `src/features/providers/` 的模型信息/别名视图与行为测试

**Estimated scope:** 原建议文件表不足以覆盖当前调用路径；按下列阶段实施。设计、证据缺口与数据边界见 [model-parameters.md](model-parameters.md)。后续阶段开始前继续拆成最多五个实际文件的小批任务。

#### P16.0：现状、证据与计划

- [x] 用 codegraph 核对目录解析、模型记录、IPC 与 UI 依赖；确认模型缺少 revision，schema v9 已被使用。
- [x] 读取 Command Code/OpenRouter 官方页面；记录 DeepSeek 直接访问失败与 Context7 schema 和旧记录不一致，不据此推断字段已删除。
- [x] 保存设计并按维护者确认调整职责：服务商管理模型信息，请求参数放在 Agent 配置；不先实现独立通用参数模块。

**Files:** 本清单、`model-parameters.md`。
**Verification:** Markdown 格式与 diff 检查；仅规划，不表示参数已实现。
**Dependencies:** P12,P15。

#### P16 后续阶段（开始前继续拆批）

原 P16.a 的独立请求参数校验与 P16.e/P16.f 的服务商参数表单安排已撤回，规则随 P18/P22 中实际支持的 Agent 设置实现，不标为已完成。

- [ ] **P16.b：目录能力解析。** 按 Agent 预览实际需要补字段和测试；OpenRouter 已有公开证据，DeepSeek 扩展字段必须重新核实，证据不足保持待验。Dependencies：P16.0。
- [ ] **P16.c：元数据保存。** 分迁移/revision、仓库读写、浏览与获取合并小批；只保存中央能力与来源，不保存全局请求参数。Dependencies：P16.b。
- [ ] **P16.d：模型别名编辑与信息显示。** 薄命令、严格 IPC、并发保护与双语视图分批实现；不增加请求设置控件。Dependencies：P16.c。

#### Checkpoint C16

- [ ] P16.b–P16.d 与 P16 总验收项通过；迁移保留、并发、上游更新保留别名和双语反馈有行为测试。
- [ ] 完整前端/Rust 检查与锁定本机桌面构建通过；维护者完成可见功能验收，平台与真实连接缺口单独记录。
- [ ] 可用能力与未知组合有明确来源；C16 不阻塞 P18 最小预览，也不代表 Agent 设置或推理已验证。

## 后续后端扩展（按业务需要推进）

BR0–BR5 已完成；现行边界见 [backend-modularity.md](backend-modularity.md)。以下不是六 Agent 发现的额外前置：

- [ ] **BR6：Catalog/Storage/Credentials。** 各厂商解析局部化；迁移注册顺序和已发布 SQL 保持原样；按需要分离 OS store 实现和各业务补偿策略。开始前分别拆解析、迁移、凭据批次。
- [ ] **BR7：外部服务适配。** GitHub 客户端/账号支持备份目标操作；Vercel 用途待确定。以真实复用需求提取 HTTP transport，保留服务专用权限/限流/错误语义。开始实施前按认证与客户端等职责拆细项。
- [ ] **BR8：配置备份恢复设计。** 明确版本化非敏感快照、GitHub 目标/路径、远端修订冲突、本机身份/凭据重新关联与恢复差异；先显式备份恢复，自动同步另行规划。实施前拆导出、本地恢复、GitHub 存储和 UI 的具体小任务。
- [ ] **BR9：MCP/Skill 来源安装设计。** 确定首个实际来源及契约，拆候选元数据、固定 revision、安装计划/执行/更新/卸载、受管路径和恢复；安装、运行与 Agent 部署状态分开。承接 P25–P30、P31–P33/P34，避免重复实现。
- [ ] **BR10：API 转发设计。** 明确首条同协议转发、gateway 生命周期、Provider/Model 路由、协议适配、流式/工具调用/取消/错误验收；Agent 通过本地 API 访问，Tauri 只管理服务。连接 P15/P16/P18–P24，不扩大为云端多用户代理。实施前拆最小路径任务。

- [ ] **BR11：Agent 安装设计。** 核实首个 Agent 的安装/升级/卸载契约、目标平台/架构与现有安装来源；受管安装记录与发现/能力/配置分开。承接 P17；安装完成后只检测，不自动启动或应用配置。按实际首条路径拆小任务，保护用户已有安装与配置。

## Agent 发现改造与 Pi 配置

### Task P17: 固定 Agent 目录与六种只读发现

既有 Pi/Grok 发现代码已完成，真实安装和其他平台验收见上方待验项。
维护者要求本次新增 Claude Code、Codex、OpenCode、Hermes 的检测，并固定展示六种。
设计见 [agent-catalog.md](agent-catalog.md)；不安装、不启动、不读取配置或密钥内容、不注入配置。

### P17.i：文档先行

- [ ] 更新六 Agent 范围、状态和分批计划，清理过时/已完成过程记录，先提交并 push，再实施。

**Files:** 计划与状态文档，按本次已确认调整提交，不混业务代码。
**Verification:** 格式、diff、未完成项保留核对；push 成功。
**Dependencies:** 维护者已授权。

### P17.j：六种 Rust 发现

- [ ] 核实六家官方 CLI 入口、已知回退位置、配置路径/环境覆盖、平台差异和版本证据。
- [ ] 扩展固定身份与手动输入，按 Adapter 规则返回六条结果；保留 PATH 有界检查与遮蔽顺序。
- [ ] 测试全部六家自动/手动发现、未发现/坏入口/无效覆盖、元数据边界与无执行/无配置读取。

**Files:** `agents.rs`、`agents/discovery.rs`、`agents/tests.rs`、`commands/agents.rs`、本清单。
**Verification:** Rust fmt、all-targets Clippy、完整测试。
**Dependencies:** P17.i；路径细节必须有官方证据，未知版本不伪造。

### P17.k：严格 IPC 与六家输入接线

- [ ] 严格验证六条身份/顺序/路径/证据/配置观测，拒绝缺失、重复、矛盾和额外字段。
- [ ] 按 Agent 隔离窗口内手动路径；新增身份不得沿用 Grok 输入或配置标签。
- [ ] 更新边界与输入测试，浏览器不调用扫描；本批可暂保留旧结果列表布局。

**Files:** `src/lib/desktop/agents.ts`、其测试、`AgentList.tsx`、其测试、本清单。
**Verification:** 完整 `check:frontend`。
**Dependencies:** P17.j。

### P17.l：固定目录与双语状态

- [ ] 始终按 Claude Code、Codex、OpenCode、Pi、Grok Build、Hermes 顺序显示，未发现条目可进入详情并手动指定。
- [ ] loading/error/preview 不伪装成未发现；刷新失败明确旧结果，发现不等于已连接或可配置。
- [ ] 中英文状态、入口/文件标签与说明同步，保留键盘焦点和异步反馈；不开放安装/注入占位操作。

**Files:** `AgentList.tsx`、其 CSS、两份 locale、本清单。
**Verification:** 格式、翻译、严格类型/Lint；行为测试随 P17.m 补齐后一起完成验收。
**Dependencies:** P17.k。

### P17.m：目录行为与品牌资产回归

- [ ] 测试首屏/未发现/浏览器/失败六项可见、六家手动输入隔离、旧结果、重复刷新、卸载与返回焦点。
- [ ] 更新 App 测试数据与导航回归；新增品牌资产单独核实官方来源/许可，无法核实时不造品牌图标。

**Files:** `AgentList.test.tsx`、`src/App.test.tsx`、本清单；资产与图标若需修改先另拆最多五文件批次。
**Verification:** 完整 `check:frontend`，不存在静态标签专用重复测试。
**Dependencies:** P17.l。

### P17.n：最终验证与现行文档

- [ ] 完整前端、Rust fmt/Clippy/测试和锁定本机桌面构建通过；检查无权限/执行范围扩大。
- [ ] 记录实际版本/平台、六家真实安装结果与待验项；人工核对双语、长路径、最小窗口和键盘。
- [ ] 更新现行说明与英文 Unreleased，删除完成批次的过程记录，不将只读发现描述为安装或配置支持。

**Files:** `agent-discovery.md`、`frontend.md`、`phase-1.md`、`CHANGELOG.md`、本清单。
**Verification:** `AGENTS.md` 规定检查；缺少真实安装/平台时明确保持待验。
**Dependencies:** P17.j–P17.m；所有代码批次完成才称为六种检测已实现。

### Task P18: 预览 Pi 模型配置

**Description:** 在 Agent 配置流程选择 Provider/Model，按已核实 Pi 版本生成配置计划，展示脱敏差异，不写文件。先做最小模型配置预览；使用参数和必要的局部能力声明按 Pi 原生可表达的字段逐项增加，不放到服务商模型页。

**Acceptance criteria:**

- [ ] 按 P02 约定映射原生字段，保留无关配置；不支持的组合拒绝并说明。
- [ ] 展示变更文件、覆盖来源、重启要求、密钥写入方式与潜在明文存放。
- [ ] 计划绑定原文件摘要、Provider revision 和模型 revision/稳定内容摘要；无模型 revision 时不能跳过过期检查，预览不读取后回传明文密钥。
- [ ] 后续使用设置仅属于目标 Agent；能力覆盖标明用户声明，未知/不支持明确反馈，不修改中央能力或其他 Agent 设置。

**Verification:**

- [ ] 运行 Rust `pi_plan` 测试，覆盖格式保存、优先级、不兼容字段与脱敏。
- [ ] 运行 ConfigPreview 测试、`check:frontend` 与 Rust fmt/Clippy。
- [ ] Tauri 预览临时 Pi 样本，确认未发生写入。

**Dependencies:** P02,P12,P15,P17；仅补最小预览必需的模型信息与变化检查，不等待完整 P16。开始前核实 Pi 版本/配置规则并按实际文件拆批。

**Files likely touched:**

- `src-tauri/src/agents/pi.rs`
- `src-tauri/src/config_plan.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件；如需额外文件先按执行约定拆分。

### Checkpoint C18（P17–P18；最小 Pi 预览）

- [ ] P17 与 P18 最小预览已验收，所需模型信息、兼容检查、脱敏与过期保护已验证；P16 剩余目录扩展/别名任务在 C16 单独跟踪。
- [ ] 使用设置随已支持字段逐项验收；尚未支持的选项不开放，不将预览通过当作写入或推理成功。

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

### Task P31: 维护中央 MCP 定义（已完成）

实现摘要与待验项见本文开头；现行字段和安全规则见 [mcp-central.md](mcp-central.md)。
P32/P33 仍需安全 Agent 写入与恢复，中央保存不代表服务器已启动或部署。

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
