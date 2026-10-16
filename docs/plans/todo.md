# vibemate 第一阶段执行清单

这里是唯一任务状态来源。已完成任务只保留摘要；实现过程、旧文件清单和逐次测试计数从 Git 历史、CI 与 `CHANGELOG.md` 查阅，不另存重复归档。

## 当前顺序与范围

1. 六 Agent 固定目录、只读发现与版本读取已交付并通过验收（2026-10-13）；平台专项延后到发布前。
2. Claude Code 配置预览、应用与回滚（P18–P21）已验收，C21 关闭；Pi 最小配置预览、原配置备份与应用作为一个任务交付，复用现有写入与恢复机制。
3. 请求参数与必要能力覆盖属于目标 Agent 配置，不放在服务商模型页。
4. 六种发现不等于六种配置注入；配置适配顺序为 Claude Code、Pi、Grok Build。维护者编码日常使用 Pi，Pi 不作为首个写入目标。
5. 界面归属按关系基数划分：模型配置 1:1，操作在 Agent 模块；Skill/MCP 部署 1:N，操作在技能/MCP 模块。Agent 详情页只读汇总当前配置（模型/Skill/MCP 三个槽位，状态加跳转链接），不放注入操作。模型槽位与配置入口随 P18 交付；Skill/MCP 槽位随 P27/P32 各自添加，现在不留空壳。概览页 v1 五张统计卡片已落地（含模型总数 IPC）；详见下方「概览 v1」。

## 概览 v1（2026-10-14）

维护者确认先读设计再动手；仓库内无独立五卡片设计文档，按确认五卡片范围从空态改为：

- 服务商 / 已添加模型 / 已发现 Agent / 技能 / MCP：图标 + 名称 + 大数字；0 合法；加载占位；失败「无法读取」+ 重试；点击跳转对应侧栏（Agent → 列表；模型 → 服务商）。
- 模型总数：`countProviderModels()` / `count_provider_models`，仅 `selected = 1`（已添加进库，不含未勾选的目录拉取缓存；preview 不发明数字）。
- 技能 / MCP 文案只指中央库，不暗示已注入 Agent；Agent 计数用现有 `discoverAgents`，不自动读版本。

- [x] 维护者验收五张统计卡、已添加模型总数约 18、卡片折行与侧栏跳转。（维护者 2026-10-14）

**已知 flaky：** `App.test.tsx`「a save shows an app-level notification that outlives the jump back and page changes」偶发 focus 落在 `document.body`（编辑页卸载与 `refreshList` 重挂 IconButton 的竞态）；重跑通常通过。已核对 Notifications 关闭按钮在 `role=status` 外；暂无不改产品行为的稳妥修复。

## 执行约定

- 按依赖顺序实施，一次一个可检查行为；不使用未授权子代理。
- 不主动推送远端；仅在维护者当次明确指示时执行 push（2026-10-13 约定）。
- UI 变更同时补中英文和可访问名称；保持既有严格检查，不用 suppression 绕过失败。
- 外观按维护者人工验收策略；原生行为、真实凭据和跨平台验证单独记录。
- Verification 使用 `AGENTS.md` 的适用命令；Rust 过滤测试必须实际运行非零用例。
- 勾选表示该项已验证；代码完成不等于真实安装、推理或其他平台验收完成。
- 任务完成后删除详细执行过程，保留仍未解决的限制；未来路径是建议，不提前创建空壳。

## 已完成摘要

| 任务          | 已交付内容                                                              | 现行说明                                                                             |
| ------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| P00           | pnpm 与锁文件迁移                                                       | 根目录 `AGENTS.md`                                                                   |
| P01–P03 / C03 | Provider 与 Pi/Grok 官方接入证据                                        | [integrations](../integrations/compatibility.md)，实施前重新核实版本                 |
| P04–P06 / C06 | UI 行为测试、私有 SQLite、OS 凭据接口                                   | [frontend](../frontend.md)、[architecture](../architecture.md)                       |
| P07–P09 / C09 | 中英文资源、偏好、启动与界面接线                                        | [frontend](../frontend.md)                                                           |
| I02–I15       | 桌面壳、基础外观、共享表单与 Base UI 下拉                               | [desktop-shell-design](desktop-shell-design.md)、[base-ui-select](base-ui-select.md) |
| P10–P12 / C12 | 服务商与密钥统一保存，模型获取/选择/搜索和手动模型                      | 维护者已验收；[models-tab-ux](models-tab-ux.md)                                      |
| P13 / P14     | 模型发现任务已并入 P12                                                  | 最小推理检查仍在 P24/P38，不因合并而标为通过                                         |
| P15 / C15     | 按模型协议路由与三家多协议模板                                          | 维护者确认真实 Command Code 凭据连接与界面验收                                       |
| BR0–BR5       | 测试、命令、Provider/MCP、Models/Fetch 与共享校验拆分                   | [backend-modularity](backend-modularity.md)                                          |
| P17.a–P17.n   | 六 Agent 固定目录、只读发现、独立手动路径与严格 IPC                     | [agent-discovery](agent-discovery.md)；维护者已验收（2026-10-13）                    |
| P17.o–P17.s   | 用户二次确认的受限 CLI 版本读取、双语交互与完整代码检查                 | [agent-version-probe](agent-version-probe.md)；维护者已验收，自动读取开关验证通过    |
| P17.t         | 四家官方标志与状态/版本同行布局                                         | [assets README](../../assets/agents/README.md)；品牌条款核对通过（2026-10-13）       |
| P25           | 中央 Skill 库、本地导入（单个或多个技能）、来源补充、条目删除与双语列表 | [skill-library](skill-library.md)；安装、更新与部署未实施                            |
| P31           | 中央 MCP 定义、凭据引用、编辑与启停                                     | [mcp-central](mcp-central.md)；未执行或部署服务器                                    |

P15 的真实连接记录来自维护者确认，不代表代理重新执行，也不补写未提供的模型、协议或套餐细节。

## 已交付功能验收

- [x] 2026-10-13 维护者验收通过：C06（存储/凭据审阅）、I16（系统风主题浅/深色）、P17（六家 CLI、授权版本读取、原生运行与外观）、P17.t（品牌条款核对）、P25（原生技能导入）、P31（MCP 凭据库与原生外观）。P31 的中央定义删除功能现已补齐，新增验收见「Task P31」补充；Anthropic/OpenAI 标志不属于项目 MIT 授权。
- [ ] 平台专项：本阶段略过；发布前按 P36–P38 验证 Windows/Linux 原生行为、真实 OS 凭据与专门读屏（维护者决定 2026-10-13）。

### Task P16: 补充中央模型能力元数据

已完成批次与剩余项摘要见 [provider-completion.md](provider-completion.md)。维护者取消独立可用性检查；真实调用由维护者在注入 Agent 后验证，不增加检查按钮或测试请求。

- [x] A–D（2026-10-13 完成）：模型内容摘要与并发安全别名编辑、能力解析与图标（schema v13）、保留别名的已保存模型刷新、服务商安全删除与凭据清理。过程与逐次测试计数见 Git 与 `CHANGELOG.md`；原生 macOS 对话框验收已通过，P35 的 Agent 引用检查仍单独跟踪。

**Description:** 补充目录能力解析、来源记录、保存与显示，以及独立的模型别名编辑；不在服务商页面配置请求参数。维护者确认思考选项、请求输出限制和必要的能力覆盖属于目标 Agent 配置，随 P18/P22 的已核实原生映射实现。

**Acceptance criteria:**

- [x] 按已核实的目录规则解析能力，区分缺失、null、空列表与合法值；未知不显示为已支持（2026-10-13 重新核实 DeepSeek 与 OpenRouter 官方字段后实现）。
- [x] 保存与显示能力来源；重新获取保留用户别名，不改变各 Agent 的使用设置。（2026-10-14 补齐：来源逐字段保存（v13），显示在图标标签；编辑器内按维护者要求不显示状态/说明文字。刷新/合并保留用户别名与用户声明，有行为测试。）
- [x] 提供有并发保护的模型别名编辑；影响配置预览的元数据更新可检测，过期预览不可直接应用。（编辑器已有 SHA-256 摘要并发保护并返回 `model_changed`；预览过期检查按计划留在 P18。）
- [x] 承接 P12 未做的目录能力字段（如已核实的 OpenRouter `supported_parameters`）；DeepSeek 扩展证据不足保持待验，需要迁移时追加版本，v9 不得改写（本次追加 v13，v9 未改写）。
- [x] 模型能力可手动编辑（维护者要求 2026-10-13）：编辑界面提供思考、图像、工具三个勾选框与「恢复默认」；用户勾选标明为用户声明并优先于服务商目录结果，恢复默认后回到目录解析或未知。手动声明同时覆盖证据不足的未知能力。交互语义已确认（2026-10-14，详见 [provider-completion.md](provider-completion.md)）：每次勾选/取消均为用户声明（支持/不支持），未操作项保持目录解析或未知；单个「恢复默认」清除全部声明；编辑器为行内融合表单（与手动添加共用），别名与能力声明一次保存；声明存独立字段，目录刷新不覆盖。（2026-10-14 实现：schema v14 `capability_overrides` 列、`update_provider_model_edit` 一次保存、图标标注来源；表单不显示状态/说明文字，复用统一复选框样式，行为测试覆盖声明/恢复默认/并发/迁移。）

**Verification:**

- [x] 运行目录/模型/存储行为测试，覆盖能力未知、畸形字段、迁移保留、重新获取保留别名和并发/过期检查。（2026-10-14：Rust 全套 286 测试通过，含 v14 迁移、畸形声明拒绝、刷新保留声明与并发冲突。）
- [x] 运行受影响 UI 与 IPC 测试、`check:frontend` 和 Rust fmt/Clippy/测试；请求参数校验在 Agent 任务验证。（2026-10-14：前端 655 测试与 `check:frontend` 全绿，Rust fmt/Clippy/测试通过；锁定 no-bundle 构建见 CHANGELOG。）

**Dependencies:** P12,P15。

**Files likely touched:**

- `src-tauri/src/model_catalog.rs` 与解析测试
- `src-tauri/src/models/` 与模型测试
- `src-tauri/src/storage.rs` 与迁移测试
- `src/lib/desktop/models.ts` 与边界测试
- `src/features/providers/` 的模型信息/别名视图与行为测试

**Estimated scope:** 原建议文件表不足以覆盖当前调用路径；按下列阶段实施。设计、证据缺口与数据边界见 [model-parameters.md](model-parameters.md)。后续阶段开始前继续拆成可检查的小批任务。

#### P16.0：现状、证据与计划

- [x] 用 codegraph 核对目录解析、模型记录、IPC 与 UI 依赖；确认模型缺少 revision，schema v9 已被使用。
- [x] 读取 Command Code/OpenRouter 官方页面；记录 DeepSeek 直接访问失败与 Context7 schema 和旧记录不一致，不据此推断字段已删除。
- [x] 保存设计并按维护者确认调整职责：服务商管理模型信息，请求参数放在 Agent 配置；不先实现独立通用参数模块。

**Files:** 本清单、`model-parameters.md`。
**Verification:** Markdown 格式与 diff 检查；仅规划，不表示参数已实现。
**Dependencies:** P12,P15。

#### P16 后续阶段（开始前继续拆批）

原 P16.a 的独立请求参数校验与 P16.e/P16.f 的服务商参数表单安排已撤回，规则随 P18/P22 中实际支持的 Agent 设置实现，不标为已完成。

- [x] **P16.b：目录能力解析。** 按 Agent 预览实际需要补字段和测试；OpenRouter 已有公开证据，DeepSeek 扩展字段必须重新核实，证据不足保持待验。Dependencies：P16.0。（已由批次 B 交付。）
- [x] **P16.c：元数据保存。** 分迁移/revision、仓库读写、浏览与获取合并小批；只保存中央能力与来源，不保存全局请求参数。Dependencies：P16.b。（已由批次 B/C 交付，v13；v14 追加用户声明列。）
- [x] **P16.d：模型别名与能力编辑、信息显示。** 薄命令、严格 IPC、并发保护与双语视图分批实现；不增加请求设置控件。Dependencies：P16.c。（2026-10-14 交付：`update_provider_model_edit` 一次保存别名与能力声明，行内融合表单提供三勾选框与恢复默认，图标标明来源。）

#### Checkpoint C16

- [x] P16.b–P16.d 与 P16 总验收项通过；迁移保留、并发、上游更新保留别名和双语反馈有行为测试。
- [x] 完整前端/Rust 检查与锁定本机桌面构建通过；维护者完成可见功能验收，平台与真实连接缺口单独记录。（维护者 2026-10-14；Windows/Linux 平台专项继续搁置）
- [x] 可用能力与未知组合有明确来源；C16 不阻塞 P18 最小预览，也不代表 Agent 设置或推理已验证。（维护者 2026-10-14）

### Task P40: 补充模型参数（上下文窗口、思考强度、最大输出）

维护者确认（2026-10-14）：这三项是获取模型和手动模型共用的最终态。目录里有值就覆盖已保存值；手动保存再覆盖；目录缺字段时保留已保存值。能力勾选仍单独保存，刷新不覆盖。每次请求选用的档位仍属于目标 Agent。编辑改为对话框。词表见 [model-parameters.md](model-parameters.md)。

- [x] 上下文窗口与最大输出可在对话框填写；空值清除。目录有正整数时覆盖，没有时保留。
- [x] 思考档位保存为 `any`（OpenRouter `supported_efforts: null`）或显式列表加可选默认档。未知档位不写入。
- [x] 手动添加与已保存模型共用同一对话框和同一次保存。
- [x] 维护者统一验收对话框与真实目录刷新。（维护者 2026-10-14）

**Verification:**

- [x] Rust 测试覆盖目录有值覆盖、缺字段保留、手动保存与清除、畸形档位不写入。前端检查 701 项通过，Rust Clippy 与库测试通过。
- [x] 维护者验收对话框外观，以及用真实目录刷新覆盖手动改过的窗口、输出和档位。（维护者 2026-10-14）

## 后续后端扩展（按业务需要推进）

BR0–BR5 已完成；现行边界见 [backend-modularity.md](backend-modularity.md)。以下不是六 Agent 发现的额外前置：

- [ ] **BR6：Catalog/Storage/Credentials。** 各厂商解析局部化；迁移注册顺序和已发布 SQL 保持原样；按需要分离 OS store 实现和各业务补偿策略。开始前分别拆解析、迁移、凭据批次。
- [ ] **BR7：外部服务适配。** GitHub 客户端/账号支持备份目标操作；Vercel 用途待确定。以真实复用需求提取 HTTP transport，保留服务专用权限/限流/错误语义。开始实施前按认证与客户端等职责拆细项。**维护者 2026-10-13：后续功能，先不排期。**
- [ ] **BR8：配置备份恢复设计。** 明确版本化非敏感快照、GitHub 目标/路径、远端修订冲突、本机身份/凭据重新关联与恢复差异；先显式备份恢复，自动同步另行规划。实施前拆导出、本地恢复、GitHub 存储和 UI 的具体小任务。
- [ ] **BR9：MCP/Skill 来源安装设计。** 确定首个实际来源及契约，拆候选元数据、固定 revision、安装计划/执行/更新/卸载、受管路径和恢复；安装、运行与 Agent 部署状态分开。承接 P25–P30、P31–P33/P34，避免重复实现。**已确认（路线 A）**：更新能力以「来源是否明确」为条件，须定义获取方式字段与地址到来源的映射，使本地导入的技能补齐地址与获取方式后同样可更新；若改为只允许 vibemate 安装的技能更新，须同步修改界面说明，不得保留「填了地址即可更新」的暗示。
- [ ] **BR10：API 转发设计。** 明确首条同协议转发、gateway 生命周期、Provider/Model 路由、协议适配、流式/工具调用/取消/错误验收；Agent 通过本地 API 访问，Tauri 只管理服务。连接 P15/P16/P18–P24，不扩大为云端多用户代理。实施前拆最小路径任务。

- [ ] **BR11：Agent 安装设计。** 核实首个 Agent 的安装/升级/卸载契约、目标平台/架构与现有安装来源；受管安装记录与发现/能力/配置分开。承接 P17；安装完成后只检测，不自动启动或应用配置。按实际首条路径拆小任务，保护用户已有安装与配置。**维护者 2026-10-13：后续功能，先不排期。**

## Agent 发现改造与配置适配

### Task P17: 六种只读发现已交付，验收边界保持独立

固定展示 Claude Code、Codex、OpenCode、Pi、Grok Build、Hermes；全部支持有界自动发现与独立窗口内手动入口。严格 IPC 验证六条身份/顺序/文件/证据，失败不伪装未发现，旧结果明确标注。六家已接入有出处的官方品牌标志；Codex 使用 OpenAI 官方 Blossom，自动发现无法安全关联的版本保持未知。

完整前端、Rust fmt/Clippy/测试与锁定 macOS no-bundle 构建通过；真实安装、人工与其他平台验收由维护者通过（2026-10-13）。

检测规则见 [agent-discovery.md](agent-discovery.md)，范围见 [agent-catalog.md](agent-catalog.md)。自动发现不安装、不启动、不读取配置/密钥内容、不注入配置；配置部署按 Claude Code、Pi、Grok Build 顺序实施。

### P17.o–P17.s：用户主动读取版本

P17.o–P17.s 实现与代码核验完成：固定 `--version`、路径重验、应用级单探测锁、环境/stdin/输出限制、5 秒直接子进程边界、严格 IPC、双语二次确认及生命周期保护。自动发现仍不执行程序；CLI 证据不升级能力、不持久化，重新发现时清除。主动执行不是沙箱，不承诺无副作用。

完整前端、Rust fmt/Clippy/测试与锁定 macOS no-bundle 构建通过；真实/原生/人工验收与「自动读取 Agent 版本」开关由维护者验证通过（2026-10-13）。现行契约与核验范围见 [agent-version-probe.md](agent-version-probe.md)，不保留逐批过程。

### P17.t：新增四家官方品牌标志

已补齐四家官方资产并通过完整前端检查、浅/深色浏览器加载与键盘详情检查；来源/条款见 [assets README](../../assets/agents/README.md)。Codex 使用 OpenAI 官方 Blossom，不宣称为独立 Codex 标志；Pi/Grok 资产未改变。Anthropic/OpenAI 标志不属于项目 MIT 授权；维护者已完成识别用途与品牌条款核对（2026-10-13），不声称取得额外许可。

### Task P18: 预览 Claude Code 模型配置

**Description:** 在 Agent 配置流程选择 Provider/Model，按已核实 Claude Code 版本生成配置计划，展示脱敏差异，不写文件。先做最小模型配置预览；使用参数和必要的局部能力声明按 Claude Code 原生可表达的字段逐项增加，不放到服务商模型页。Claude Code 为第一个配置适配目标：维护者编码日常使用 Pi，Pi 不作为首个写入目标；Pi 与 Grok Build 依次复用同一机制。

Agent 详情页增加「当前配置」只读汇总区，首期只交付模型槽位与配置预览入口；Skill/MCP 槽位随 P27/P32 添加。槽位显示状态并链接到管理模块，注入操作不搬进 Agent 页。

最小预览的映射决定与批次计划见 [p18-claude-code-link.md](p18-claude-code-link.md)；三家 Provider 的 Claude Code 接入证据见 [claude-code.md](../integrations/claude-code.md)。

**Acceptance criteria:**

- [x] 按已核实的 Claude Code 官方 Settings/Setup 约定映射原生字段，保留无关配置；不支持的组合拒绝并说明。（维护者 2026-10-14 原生预览验收）
- [x] 展示变更文件、覆盖来源、重启要求、密钥写入方式与潜在明文存放。（维护者 2026-10-14）
- [x] 计划绑定原文件摘要、Provider revision 和模型 revision/稳定内容摘要；无模型 revision 时不能跳过过期检查，预览不读取后回传明文密钥。（维护者 2026-10-14）
- [x] 后续使用设置仅属于目标 Agent；能力覆盖标明用户声明，未知/不支持明确反馈，不修改中央能力或其他 Agent 设置。（维护者 2026-10-14）
- [x] Agent 详情的模型槽位显示只读状态（未配置/已写入/待重启）与配置入口，配置预览从该入口进入；不显示 Skill/MCP 为已支持。（维护者 2026-10-14）

**Verification:**

- [x] 运行 Rust `claude_plan` 测试，覆盖格式保存、优先级、不兼容字段与脱敏。
- [x] 运行 ConfigPreview 与 Agent 详情汇总行为测试、`check:frontend` 与 Rust fmt/Clippy。
- [x] Tauri 预览临时 Claude Code 样本，确认未发生写入。（维护者 2026-10-14）

**Dependencies:** P12,P15,P17；已核实 Claude Code 官方 Settings/Setup/Model/MCP/Skills 文档与本机 2.1.287 安装，证据见 `docs/integrations/claude-code.md`；未验证边界（三家 Provider 请求协议、非 macOS 平台、完整 key 集）在实施中补验。仅补最小预览必需的模型信息与变化检查，不等待完整 P16。

**Files likely touched:**

- `src-tauri/src/agents/claude_code.rs`
- `src-tauri/src/config_plan.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/AgentPage.tsx`

**Estimated scope:** M：5 个建议主文件。

### Checkpoint C18（P17–P18；最小 Claude Code 预览）

- [x] P17 与 P18 最小预览已验收，所需模型信息、兼容检查、脱敏与过期保护已验证；P16 剩余目录扩展/别名任务在 C16 单独跟踪。（维护者 2026-10-14）
- [x] 使用设置随已支持字段逐项验收；尚未支持的选项不开放，不将预览通过当作写入或推理成功。（维护者 2026-10-14）

### Task P19: 建立可恢复配置写入

**Description:** 实现文件写入安全机制，供 Pi 应用与其他注入复用，不扩展为通用文件管理器。
设计、安全规则与批次见 [config-apply.md](config-apply.md)；P19.a（目标校验、私有备份、原子替换）、
P19.b1（操作日志，schema v16）与 P19.b2（多文件编排与补偿）均已交付 2026-10-14。

**Acceptance criteria:**

- [x] 应用前检查摘要/权限/目标身份，同目录临时写入后原子替换，私有备份可定位。（2026-10-14 P19.a：
      令牌不符即拒绝、符号链接与只读目标拒绝、同目录临时文件 + `rename`、备份 0700/0600 并回报路径。）
- [x] 多文件操作记录步骤，部分失败可补偿；重启能发现未完成操作。（2026-10-14 P19.b：schema v16 操作
      日志在替换任何文件前写入；失败时回滚已应用步骤并记 `compensated`／`partial`／`unknown`；
      `unfinished_operations` 在重启后列出未完成操作，界面入口属 P34。）
- [x] 备份访问受限，敏感内容不进入差异/错误/普通导出；不静默跟随意外 symlink。（P19.a/b：备份目录
      0700、文件 0600，错误只带稳定码，测试扫描数据库文件确认密钥不在其中；目标路径为符号链接即拒绝。）

**Verification:**

- [x] 运行 Rust `config_apply` 故障注入测试，覆盖并发、替换失败、权限和部分提交。（2026-10-14：32 项，
      含并发变化、文件出现/消失、符号链接、只读目标与只读父目录、备份不可用或摘要不符、第二文件失败后
      回滚、回滚遇到用户改动时记 `partial`。）
- [x] 运行 Rust fmt/Clippy；阶段检查点执行原生构建。（2026-10-14 补记：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、
      `cargo test --locked`（378 通过、1 忽略）与 `pnpm run tauri build --no-bundle -- --locked` 均通过；原生真实写入验收已由 P20 覆盖。）

**Dependencies:** P05,P06,P18。

**Files likely touched:**

- `src-tauri/src/config_apply.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/credentials.rs`
- `src-tauri/src/config_apply_tests.rs`

**Estimated scope:** M：4 个建议主文件。

### Task P20: 应用 Claude Code 模型配置

**Description:** 让用户在确认预览后应用选择的模型与认证配置，满足原有启动方式。Pi 作为第二个适配器在其后复用同一写入机制，单独拆任务卡。

**Acceptance criteria:**

- [x] 只应用有效且未过期的预览计划，遇到文件/模型 revision 变化要求重新预览。（2026-10-14：界面回传
      目标路径、文件摘要、Provider revision、模型内容摘要四个令牌，Rust 重新构建计划后逐一比对，任一
      不符返回 `plan_stale` 且不写盘。）
- [x] 通过 Rust 按原生方式注入凭据，UI 明示目标明文限制；不隐式依赖 vibemate 启动器。（凭据引用由
      Provider ID 生成，应用时才从系统凭据存储读取并写入 `env` 块；密钥不进入计划、结果、错误或日志；
      预览卡片与结果卡片都说明明文落盘；没有任何步骤依赖 vibemate 启动 Agent。）
- [x] 写入后重读验证，显示成功/部分失败/待重启；连续提交不会重复破坏配置。（P19 的写入在替换后重读
      比对摘要；结果卡片显示状态、失败文件与原因、备份位置，仅当文件写入新键时提示重启；应用一次即消费
      计划，重复点击由 pending 守卫拦截。）

**Verification:**

- [x] 运行 Rust `claude_apply`/`config_apply` 测试和 UI 重复提交/失败测试。（2026-10-14：Rust 应用域
      8 项 + `config_apply` 32 项，覆盖四个令牌过期、凭据缺失与不可读、未核实服务商、符号链接目标、
      重复应用只写一次；界面测试覆盖令牌回传、拒绝原因、计划过期丢弃、凭据缺失保留计划与重复点击。）
- [x] 运行 `check:frontend` 与 Rust fmt/Clippy。（2026-10-14：前端 40 文件 755 项、0 警告；Rust
      fmt/Clippy 通过，全量测试 371 通过、1 忽略。）
- [x] 使用可恢复的测试配置，在真实 Tauri/Claude Code 中确认原生启动与模型选择。（维护者 2026-10-14：预览、应用、改后须重预览、密钥不泄漏；真实 Claude Code 发消息正常。）

**Dependencies:** P11,P18,P19。

**Files likely touched:**

- `src-tauri/src/agents/claude_code.rs`
- `src-tauri/src/config_apply.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/ConfigPreview.tsx`
- `src/features/agents/ConfigPreview.test.tsx`

**Estimated scope:** M：5 个建议主文件。

### Task P21: 回滚 Claude Code 配置

**Description:** 提供历史操作结果和恢复预览，将误操作恢复为备份版本。机制放在 `config_apply`，
首个界面只服务 Claude Code；设计见 [config-apply.md](config-apply.md) 的 P21 段。
**状态：** 已交付配置历史、恢复预览与应用、逐步骤恢复状态和双语界面；维护者于 2026-10-16 确认原生验收通过。

**Acceptance criteria:**

- [x] 可查看脱敏操作状态并预览恢复；受限备份内容不直接返回前端。
- [x] 恢复前检查用户后续修改，冲突时拒绝覆盖并给出处理步骤。
- [x] 恢复配置和必要凭据关系，失败可继续恢复；清理失败不隐瞒残留状态。

**Verification:**

- [x] 运行 Rust `config_restore` 故障测试与 RestoreView 行为测试。（2026-10-14：`config_apply/restore_tests.rs`
      13 项与 `agents/claude_code/restore_tests.rs` 7 项覆盖分类、冲突、重试继续、过期审批、残留副本与凭据归属；
      `RestoreView.test.tsx` 13 项覆盖列表、预览、确认、重复点击、键盘提交与冲突提示。）
- [x] 运行 `check:frontend` 与 Rust fmt/Clippy。（2026-10-14：前端 42 文件 834 项与 `check:frontend` 全绿；
      Rust fmt/Clippy 通过，全量测试 403 通过、1 忽略。）
- [x] 真实 Tauri 使用临时配置完成应用→恢复→原生启动检查。（维护者 2026-10-16 确认验收通过。）

**Dependencies:** P20。

**Files likely touched:**

- `src-tauri/src/config_apply.rs`
- `src-tauri/src/agents/claude_code.rs`
- `src/lib/desktop/agents.ts`
- `src/features/agents/RestoreView.tsx`
- `src/features/agents/RestoreView.test.tsx`

**Estimated scope:** M：5 个建议主文件。

### Checkpoint C21（P19–P21）

- [x] P19–P21 均已验收，C21 关闭。（维护者 2026-10-16 确认；Windows/Linux 专项仍在发布前验证。）

## 阶段 E：后续适配器

Pi 为第二个配置适配器，复用已验收的 P19–P21 预览/写入/回滚机制；维护者编码日常使用 Pi，实施时须保持现有配置不被破坏。Grok Build 为第三个适配器。

### Task PI1: Pi 最小配置预览、原配置备份与应用

维护者于 2026-10-16 确认合并交付，不再拆分预览、备份和应用批次。接入约定与边界见 [pi.md](../integrations/pi.md)。

- [x] 核实 Pi 1.2.0/1.3.0 的配置格式、凭据优先级与默认模型选择；稳定 Pi 1.x（1.2 起）按安装包发布的模型/设置 schema 检查兼容性，不锁定具体小版本。
- [x] 在 Pi 详情选择已添加模型，展示 `models.json` 与 `settings.json` 的脱敏变更；计划绑定两个文件、只读 `auth.json`、Provider revision 和模型摘要。
- [x] 使用每实例独立且可读的 Provider 名称，冲突时才添加短后缀；旧 UUID 名称经预览、备份后迁移，并同步模型范围与推理档位引用。schema v18 记录别名所有权，保留其他 Provider、模型与无关设置；不接管无操作记录的同名条目，存在更高优先级凭据时拒绝应用。
- [x] 应用时才读取 OS 凭据，按 Pi 规则转义密钥；Unix 写入文件为 0600，原内容保存在私有备份中。多文件失败补偿、历史恢复与冲突保护复用已有机制。
- [x] 配置历史按 Agent 隔离；可预览并恢复 Pi 备份，Claude Code 的入口拒绝 Pi 操作记录。
- [x] `check:frontend`、Rust fmt/Clippy/全量测试及锁定 macOS no-bundle 构建通过。
- [x] 隔离 macOS Tauri 使用合成 OS 凭据完成预览、双文件备份、应用、重启后历史读取、过期恢复拒绝与逐字节恢复；Pi 1.2.0 原生模型运行时离线加载生成结果。Chrome 检查浏览器预览提示和键盘焦点。（2026-10-16，代理验证；未发起真实 Provider 请求。）
- [x] 维护者确认 Pi 1.3.0 升级后的配置预览与可读 Provider 名称验收通过。（2026-10-16。）
- [ ] 维护者使用真实 Provider 在新 Pi 会话中确认模型与消息请求；Windows/Linux 按发布前专项验证。

**范围：** 最小默认 Provider/模型配置；不新增推理档位和请求参数编辑，不把文件写入状态当作会话已生效。

**Dependencies:** P12,P15,P19–P21。

### Task P22: 预览 Grok Build 配置

**Description:** 接入第三个 Agent 的原生映射，复用配置差异组件。

**Acceptance criteria:**

- [ ] 按 P03 的准确版本与格式生成计划，保留其他字段并执行协议/参数校验。
- [ ] 展示覆盖来源、密钥存放、生效/重启方式；未知版本或缺少接口明确报错。
- [ ] 复用计划/脱敏机制而不复制首个适配器配置假设；预览不写文件。

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

**Estimated scope:** M：5 个建议主文件。

### Task P23: 应用 Grok Build 配置

**Description:** 让第三个 Agent 使用已有可靠写入与回滚机制，验证原有启动方式。

**Acceptance criteria:**

- [ ] 有效计划能写入并重读，处理并发变化、认证方式和配置优先级。
- [ ] 可以应用和恢复，异常沿用明确的操作状态与恢复路径。
- [ ] 真实启动证据支持该版本/Provider 组合；需启动器/桥接时先决定范围（维护者 2026-10-13：待定）。

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

**Estimated scope:** M：5 个建议主文件。

### Task P24: 验证 Provider 与 Agent 组合

**Description:** 检查三家 Provider × 两个 Agent 的实际兼容矩阵与切换行为，不承诺不存在的协议转换。

**Acceptance criteria:**

- [ ] 每个组合有 supported/unsupported/unverified 状态及原因、版本与证据。
- [ ] 切换账户或模型保持稳定绑定，并正确提示项目/环境变量覆盖。
- [ ] 回归工具调用、推理/图像参数等已支持能力，未支持项不被静默丢弃。
- 独立最小推理检查已按维护者决定取消，不实现检查按钮或测试请求。真实调用由维护者在注入 Agent 后验证，发现问题再按实际模型与协议修复；取消不等于真实验收通过。

**Verification:**

- [ ] 运行 Rust `compatibility` 映射样本测试与 UI 切换测试。
- [ ] 运行完整项目检查；真实用例仅使用许可的测试凭据。

**Dependencies:** P12,P15,P16,P20,P23。

**Files likely touched:**

- `src-tauri/src/agents/compatibility_tests.rs`
- `src-tauri/src/agents.rs`
- `src/features/agents/AgentList.test.tsx`
- `docs/integrations/compatibility.md`

**Estimated scope:** M：4 个建议主文件。

### Checkpoint C24（P22–P24）

- [ ] P22–P24 均已勾选。

## 阶段 F：Skill

Skill 部署目标顺序与配置适配一致（Claude Code、Pi、Grok Build）；Claude Code 的 Skill 目录与格式核实后单独拆任务卡，现有 Pi/Grok 卡片在各自顺序到位时按已核实格式执行。

### Task P25: 登记中央 Skill 来源

**Description:** 让用户导入已有本地 Skill 并维护准确来源，支持未知来源的明确状态。设计与边界见 [skill-library](skill-library.md)。

**Acceptance criteria:**

- [x] 中央库记录稳定 ID、内容摘要与来源路径；原目录不被移动、修改或删除。
- [x] 「本地导入」调用系统文件夹选择器；所选目录本身是技能时导入它，否则导入其直接子目录中的全部技能，逐个报告被跳过的文件夹与原因。
- [x] 兼容含 `SKILL.md` 的本地 GitHub/npx 安装目录；frontmatter 名称/描述与目录合法性按已核实约定校验。
- [x] 不能自动识别的上游来源可在来源页补充或清除，界面明确显示「不可自动更新」；列表按名称有界分页。
- [x] `origin_path IS NOT NULL OR source_url IS NOT NULL` 与读取时重新校验，畸形库数据不进入界面。
- [ ] 本地修改差异比较：导入摘要已作为基线，三方比较随 P29 实现。
- [ ] 真实桌面与平台验收见「已交付功能验收」。

**Verification:**

- [x] Rust skills 测试 21 项：路径、未知来源、同名、解析失败、链接与上限、原目录不变、摘要确定性、文件夹发现与批量报告、长正文与超大头部、IPC 契约；存储迁移测试覆盖 v11 保留行与放宽上限。
- [x] 真实集合核对：`~/.skills-manager/skills`、`~/.grok/skills`、`~/.agents/skills` 各 13 个技能全部导入、无跳过（含 11 MB / 315 个文件的技能与 87 KB 的 `SKILL.md`）。
- [x] SkillLibrary 测试 13 项（含中英文）、`desktop/skills` 边界测试 9 项；`check:frontend`、Rust fmt/Clippy 与全量测试通过。
- [x] 浏览器预览核对：技能页、导入在预览中被明确拒绝、键盘焦点与窄宽度布局。
- [x] 桌面启动核对：注册 dialog 插件后 `pnpm tauri dev` 正常启动并记录 `storage_ready`，无插件错误。
- [ ] Tauri 原生窗口内点击「本地导入」、在系统选择器中选择真实技能目录并确认结果；选择器交互与人工外观验收留待维护者。

**Dependencies:** P05,P17。

**Files:** `src-tauri/src/skills/`（manifest/validation/repository/service/tests）、`src-tauri/src/commands/skills.rs`、`src-tauri/src/storage.rs`、`src/lib/desktop/skills.ts` 与其测试、`src/features/skills/`、双语资源。

**删除补充（2026-10-14）：** 中央库条目可经双语确认对话框删除（`delete_skill`），revision 保护拒绝过期确认；只删除库记录与 vibemate 受管副本，导入来源目录不变。受管副本先改名停放（`.delete-<id>`）再删记录，被拒绝的删除改名还原；无法删除的停放副本报告 `cleanup_pending` 并可重试清理，`cleanup_deleted_skill_copies` 只清理无对应记录的停放副本。当前无部署关系，删除不涉及 Agent 配置；部署关系实现后需补引用检查。设计见 [skill-library](skill-library.md)。

- [x] Rust skills 测试增至 27 项（删除、过期 revision、副本缺失、停放清理与 IPC 契约）；`SkillLibrary` 与 `desktop/skills` 测试覆盖确认对话框、过期拒绝、清理重试与响应校验；`check:frontend`、Rust fmt/Clippy/全量测试与锁定 no-bundle 构建通过。
- [x] 技能删除新增原生确认对话框/外观验收；既有 P25 验收不覆盖本次新增行为。（维护者 2026-10-14 确认：本地副本与数据库记录均已删除）

### Task P26: 从已确认来源安装 Skill

**Description:** 在隔离目录获取 GitHub 或经核实的安装器结果，再加入中央 Skill 库。

**Acceptance criteria:**

- [ ] Git 来源固定 revision；npx 来源记录准确安装器/版本/命令和退出状态，执行前显示命令。
- [ ] 不执行 Skill 内容中的命令；校验路径遍历、失效来源和结果格式。
- [ ] 失败不破坏已有库；成功记录来源及可更新方式，不声称未知来源可更新。本地导入的技能在来源页补齐地址与获取方式后写入同一字段，供 P29 使用。

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

**Estimated scope:** M：5 个建议主文件。

### Task P27: 向 Pi 部署 Skill

**Description:** 将中央 Skill 通过链接或明确的复制同步部署到 Pi 支持的目录。

**Acceptance criteria:**

- [ ] 预览新增/冲突与目标路径，symlink 优先；不可用时让用户选择 copy-sync。
- [ ] 保留同名用户内容，记录所有权；禁用只撤销自己管理的目标。
- [ ] 报告目标已同步的 revision，失败可恢复，不把部分部署显示为整体成功。
- [ ] 在 Agent 详情汇总区添加 Skill 只读槽位（部署状态 + 跳转技能模块）；首个 Skill 部署任务即交付该槽位。

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

**Estimated scope:** M：5 个建议主文件。

### Checkpoint C27（P25–P27）

- [ ] P25–P27 均已勾选。

### Task P28: 向 Grok Build 部署 Skill

**Description:** 按照已核实的第三个 Agent 目录与格式部署，复用所有权和恢复机制。

**Acceptance criteria:**

- [ ] 仅对 P03 已确认支持的格式部署，未知能力明确为待决，不创建无效目录。
- [ ] 正确处理目标冲突、symlink 权限与 copy-sync，并记录部署 revision。
- [ ] 撤销、重试和部分失败不会影响其他 Agent 或用户 Skill。

**Verification:**

- [ ] 运行 Rust `skill_deploy_grok` 与部署 UI 回归测试。
- [ ] 运行 `check:frontend` 与 Rust fmt/Clippy。
- [ ] 真实 Agent 支持时检查可发现性；缺口由维护者决定（2026-10-13：待定），任务不假装完成。

**Dependencies:** P03,P27。

**Files likely touched:**

- `src-tauri/src/skills/deploy.rs`
- `src-tauri/src/agents/grok_build.rs`
- `src/lib/desktop/skills.ts`
- `src/features/skills/SkillDeployment.tsx`
- `src/features/skills/SkillDeployment.test.tsx`

**Estimated scope:** M：5 个建议主文件。

### Task P29: 预览 Skill 更新

**Description:** 获取新版本并展示上游变化、本地修改和目标部署状态，不直接覆盖。

**Acceptance criteria:**

- [ ] 只对来源明确的 Skill 检查更新，Git/npx 使用已记录的获取方式和版本；本地导入的技能补齐地址与获取方式后同样适用（路线 A，见 [skill-library](skill-library.md)）。
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

**Estimated scope:** M：5 个建议主文件。

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

**Estimated scope:** M：5 个建议主文件。

### Checkpoint C30（P28–P30）

- [ ] P28–P30 均已勾选。

## 阶段 G：MCP

MCP 部署目标顺序与配置适配一致（Claude Code、Pi、Grok Build）；Claude Code 的 MCP 格式核实后单独拆任务卡。

### Task P31: 维护中央 MCP 定义（已完成）

实现摘要与待验项见本文开头；现行字段和安全规则见 [mcp-central.md](mcp-central.md)。
P32/P33 仍需安全 Agent 写入与恢复，中央保存不代表服务器已启动或部署。

**维护者验收（2026-10-13）：** 凭据库操作、原生与外观通过；中央 MCP 删除补充批次见 [mcp-delete.md](mcp-delete.md)。已实现确认、版本保护、事务删除与重启后凭据清理重试；当前无 Agent 注册关系，不修改 Agent 配置。P35 的引用关系展示与阻止删除仍在部署关系实现时完成。

- [x] MCP 删除新增原生/真实凭据/外观验收；既有 P31 验收不覆盖本次新增行为。（维护者 2026-10-14）

### Task P32: 向 Pi 注入 MCP 配置

**Description:** 基于 P02 支持矩阵，将 MCP 定义映射为 Pi 接受的配置或显式记录缺口。

**Acceptance criteria:**

- [ ] 原生支持时按实际格式预览并应用，保留无关字段和已有 MCP 条目。
- [ ] 凭据在 Rust 中解析，差异脱敏，写入/撤销/恢复复用安全机制。
- [ ] 无原生支持时明确标记为不支持、不实现桥接（维护者决定 2026-10-13）；不以提示替代交付。
- [ ] 在 Agent 详情汇总区添加 MCP 只读槽位（部署状态 + 跳转 MCP 模块）；首个 MCP 部署任务即交付该槽位。

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

**Estimated scope:** M：5 个建议主文件。

### Task P33: 向 Grok Build 注入 MCP 配置

**Description:** 按第三个 Agent 的真实格式实现 MCP 部署与恢复，验证其独立能力。

**Acceptance criteria:**

- [ ] 原生支持时独立映射传输与参数，不能照搬其他适配器配置或丢弃不支持字段。
- [ ] 预览、应用、撤销与恢复保持原配置、所有权和凭据脱敏。
- [ ] 无原生支持时明确标记为不支持、不实现额外桥接（维护者决定 2026-10-13）；不能宣称 MCP 已完成。

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

**Estimated scope:** M：5 个建议主文件。

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

**Estimated scope:** M：5 个建议主文件。

### Task P36: 检查前端实际可用性

**Description:** 用真实业务页面检查长内容、键盘、分页与错误状态，修复有证据的问题。

**Acceptance criteria:**

- [ ] 中英文在 720×560、缩放和长 URL/模型 ID 下可操作，列表加载/空/失败/分页有明确反馈。
- [ ] 表单与确认界面键盘可达、焦点恢复正确，状态不只靠颜色；符合严格样式规则。
- [ ] 仅修复发现的问题；跨多个 feature 时先按页面拆子任务。

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

**Estimated scope:** M：5 个建议主文件。

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

**Estimated scope:** M：5 个建议主文件。

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

**Estimated scope:** S：3 个建议主文件。

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

**Estimated scope:** M：5 个建议主文件。

### Checkpoint C39（P37–P39）

- [ ] P37–P39 均已勾选；可打第一阶段 release 标签。
