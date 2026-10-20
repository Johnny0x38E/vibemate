# vibemate 第一阶段执行清单

这里是唯一任务状态来源。已完成任务只保留一行摘要；实现过程、旧文件清单和逐次测试计数从 Git 历史、CI 与 `CHANGELOG.md` 查阅。

行为参考文档见 [`docs/README.md`](../README.md)：Agent 与发现 [agents](../agents.md)、配置写入与历史 [configuration](../configuration.md)、Skill [skills](../skills.md)、MCP [mcp](../mcp.md)、模型 [models](../models.md)、前端 [frontend](../frontend.md)。

## 当前位置

1. Claude Code、Pi、Grok Build 三个写入适配器的预览、应用、完整分页历史、任意版本恢复、中断核对、历史删除均已交付；H2 已完成，H3 后端与界面已合入本机。
2. Agent 列表当前模型展示已合入本机（维护者 2026-10-19 检查通过）：Claude Code 读取 `env.ANTHROPIC_MODEL`/`model`，Pi 读取最新 session 的模型元数据，Grok Build 读取 `[models].default`；只有仍归属 vibemate 的 Provider/model 才显示服务商与模型，否则显示「自有配置」，未发现或不可读显示「暂无配置摘要」。Grok 写入不再修改 `[models].default`。
3. 六 Agent 固定目录、只读发现、显式与自动版本读取已交付并通过验收（2026-10-13）。
4. 维护者已确认 H1/H3 桌面目视验收通过；H3 已完成，H1 的 Windows/Linux 原生分页验证仍随 P37 跟踪。
5. 下一条业务主线按 P24 → Skill 安装/更新/部署 → MCP 部署 → P35 引用约束的顺序推进；见 [phase-1](phase-1.md)。

## 文档整理（2026-10-19）

- [x] 文档拆分为现行参考页（[`docs/README.md`](../README.md)）与剩余计划；已交付设计文档及其过程记录从 `docs/plans/` 移除，任务状态只保留在本文件。
- [x] 移除被分页历史取代的「最近十条」列表命令（`claude_code_operations`、`pi_operations`、`grok_operations`）、对应域函数/类型与前端封装；相关测试改为断言根限定分页历史。（Rust 572 通过、1 忽略；前端 1046 通过。）

## 执行约定

- 按依赖顺序实施，一次一个可检查行为；不使用未授权子代理。
- 复用已有接入调研，不因 Agent 小版本更新全面重查或阻塞实现；遇到已知弃用、明确冲突或验证失败，再针对性核实（维护者 2026-10-20 决定）。
- 不主动推送远端；仅在维护者当次明确指示时执行 push（2026-10-13 约定）。
- UI 变更同时补中英文和可访问名称；保持既有严格检查，不用 suppression 绕过失败。
- 外观按维护者人工验收策略；原生行为、真实凭据和跨平台验证单独记录。
- Verification 使用 `AGENTS.md` 的适用命令；Rust 过滤测试必须实际运行非零用例。
- 勾选表示该项已验证；代码完成不等于真实安装、推理或其他平台验收完成。
- 任务完成后删除详细执行过程，保留仍未解决的限制；未来路径是建议，不提前创建空壳。
- 所有配置写入/恢复预览统一采用两层展示（字段摘要 + 详情弹窗），见 [configuration.md](../configuration.md#preview-presentation)。

## 已交付摘要

| 任务            | 已交付内容                                                                           | 现行说明                                                                                   |
| --------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| P00             | pnpm 与锁文件迁移                                                                    | 根目录 `AGENTS.md`                                                                         |
| P01–P03 / C03   | Provider 与 Pi/Grok 官方接入证据                                                     | [integrations](../integrations)，实施前重新核实版本                                        |
| P04–P06 / C06   | UI 行为测试、私有 SQLite、OS 凭据接口                                                | [frontend](../frontend.md)、[architecture](../architecture.md)                             |
| P07–P09 / C09   | 中英文资源、偏好、启动与界面接线                                                     | [frontend](../frontend.md)                                                                 |
| I02–I15         | 桌面壳、基础外观、共享表单、Base UI 下拉与概览五卡片                                 | [frontend](../frontend.md)                                                                 |
| P10–P12 / C12   | 服务商与密钥统一保存，模型获取/选择/搜索和手动模型                                   | [models](../models.md)                                                                     |
| P13 / P14       | 并入 P12                                                                             | 最小推理检查仍在 P38，不因合并而标为通过                                                   |
| P15 / C15       | 按模型协议路由与三家多协议模板                                                       | [integrations/providers](../integrations/providers.md)                                     |
| P16 / C16 / P40 | 能力解析与来源、用户声明、别名编辑、上下文/输出/档位                                 | [models](../models.md)、[provider 证据](../integrations/providers.md)                      |
| P17.a–P17.t     | 六 Agent 只读发现、手动路径、显式/自动版本读取与官方标志                             | [agents](../agents.md)                                                                     |
| P18–P21 / C21   | Claude Code 预览、写入、回滚与历史                                                   | [configuration](../configuration.md)                                                       |
| PI1             | Pi 预览、双文件备份、应用与历史                                                      | [Pi 契约](../integrations/pi.md)                                                           |
| P22–P23         | Grok Build 预览、应用与历史恢复                                                      | [Grok 契约](../integrations/grok-build.md)                                                 |
| 当前模型展示    | Claude Code/Pi/Grok Build 列表当前模型读取与归属判定；Grok 不改写 `[models].default` | [agents](../agents.md)、[Pi](../integrations/pi.md)、[Grok](../integrations/grok-build.md) |
| H1 分页与界面   | 快照、核对/解决、未引用副本管理、根限定版本分页、当前版本独立加载并置顶              | [configuration](../configuration.md)                                                       |
| H2              | 独立完整版本选择、严格 IPC、字段差异预览、并发保护                                   | [configuration](../configuration.md)                                                       |
| H3 后端与界面   | 持久删除状态、独占副本删除、严格 IPC、确认对话框                                     | [configuration](../configuration.md)                                                       |
| BR0–BR5         | 命令、Provider/MCP、Models/Fetch 与共享校验拆分                                      | [backend-modularity](backend-modularity.md)                                                |
| P25 / P31       | 中央 Skill 库与本地导入/删除；中央 MCP 定义与删除                                    | [skills](../skills.md)、[mcp](../mcp.md)                                                   |

P15 的真实连接记录来自维护者确认，不代表代理重新执行，也不补写未提供的模型、协议或套餐细节。

## 已交付功能验收

- [x] 2026-10-13 维护者验收通过：C06（存储/凭据审阅）、I16（系统风主题浅/深色）、P17（六家 CLI、授权版本读取、原生运行与外观）、P17.t（品牌条款核对）、P25（原生技能导入）、P31（MCP 凭据库与原生外观）。
- [x] 2026-10-14 维护者验收通过：P16/P40 模型编辑对话框与真实目录刷新；P18/P20 Claude Code 原生预览与应用；P31 中央 MCP 删除；P25 技能删除。
- [x] 2026-10-16 维护者验收通过：P21（Claude Code 原生恢复）、PI1（Pi 1.3.0 预览与可读 Provider 名称）、P22/P23（Grok Build 配置预览、应用与历史恢复，本机 `grok 1.0.60`）。
- [x] 概览 v1 五张统计卡与侧栏跳转（维护者 2026-10-14）。
- [ ] 平台专项：本阶段略过；发布前按 P37–P39 验证 Windows/Linux 原生行为、真实 OS 凭据与专门读屏。

**已知 flaky：** `App.test.tsx`「a save shows an app-level notification that outlives the jump back and page changes」偶发 focus 落在 `document.body`（编辑页卸载与 `refreshList` 重挂 IconButton 的竞态）；重跑通常通过。已核对 Notifications 关闭按钮在 `role=status` 外；暂无不改产品行为的稳妥修复。

## 后续任务

### Task H1: 保留每次改写快照并浏览完整历史

**状态：** 后端、界面与维护者桌面目视验收已完成；Windows/Linux 原生分页验证仍随 P37 跟踪，不视为已通过。

已交付：schema v20 独立逐文件快照、只读核对、schema v21 显式解决、未引用私有副本管理、配置根限定只读分页、只读分页 IPC、前端严格边界、分页历史界面、备份可用性、当前配置置顶。设计与边界见 [configuration.md](../configuration.md#history)。

**Acceptance criteria:**

- [x] 所有实际改写路径（含恢复与补偿）先持久化改写前字节/不存在状态；失败不得继续写入。
- [x] 持久记录手动恢复与原版本、补偿关系；旧日志保持原样且不被误重放。
- [x] 三个已支持 Agent 的界面可访问全部配置版本并分页；恢复/补偿保留在审计日志但不增加版本行；当前版本按完整写入摘要核对并独立置顶，外部修改不误标当前。
- [x] 维护者桌面验收版本分页、置顶、选择可用性与核对入口。
- [ ] Windows/Linux 原生分页验证随 P37。历史行删除界面另在 H3 跟踪。

**Verification:** Rust 分页/核对/并发/重启测试、前端边界测试与隔离 macOS Tauri 运行已通过；平台与桌面目视项见上。

**Dependencies:** P21,PI1,P23。

### Task H2: 从界面恢复任意历史快照

**状态：** 已完成（2026-10-19）。隔离 macOS Tauri 与浏览器人工检查由代理执行，Windows/Linux 仍按发布前专项验证。

**Acceptance criteria:**

- [x] 每个应用版本拥有独立完整结果快照（含 Pi 未变化文件），选择时恢复该版本写入后的内容；每次恢复仍先保存审计/恢复快照，但不新增可见版本。
- [x] 展开以字段差异和「使用此配置」为主，保留并发保护、严格 IPC、多文件/失败重试及简短旧密钥/新会话提示；不重复备份/操作流水说明。
- [x] 缺失/损坏快照和不可证明的旧状态阻止恢复；三 Agent 模型配置复用同一呈现。

**Dependencies:** H1 的独立快照、核对保护及配置根限定历史。

### Task H3: 从界面删除历史记录及备份

**状态：** 已完成；维护者已确认桌面目视验收通过，Windows/Linux 原生验证仍随 P37 跟踪。

**Description:** 提供用户主动清理入口；删除不可恢复，但不修改当前 Agent 配置或模型归属。

- [x] 后端首批：schema v23 持久删除状态与独占备份目标；严格 IPC `delete_configuration_history_entry`；10 项 Rust 与边界测试。
- [x] 删除界面本机合入：压缩历史行、`HistoryDeleteAction` 确认对话框、活动/未解决行禁用、残留副本重试与双语文案。

**Acceptance criteria:**

- [x] 每条可删除版本（含最早/中间/曾恢复版本）提供二次确认，说明仅删除本版本快照、保留审计/恢复数据及不可撤销；不自动过期。
- [x] 已完成的隐藏恢复批次不阻止版本删除；保留活动/未解决保护和别名所有权，删除一条不破坏其它版本，删除当前版本不写 Agent 文件。
- [x] 删除文件和数据库记录有持久状态、幂等重试及界面错误反馈；重试在删副本前重新核验保护，中断后可继续处理残留。
- [x] 维护者桌面目视验收删除界面。

**Verification:** 当前完整 Rust 592 项通过、1 项忽略，Clippy 与完整前端检查通过。三 Agent 独立快照、迁移/重启、删除重试保护有回归；隔离 macOS Tauri 使用合成凭据与临时 Claude Code 根，23 项断言覆盖 A/B/C 切换、分页外当前项、中间/曾恢复/当前版本删除且文件不变，测试凭据已清理。中英浅深色、键盘和 720×560 浏览器目视由代理完成；维护者已确认桌面验收通过；Windows/Linux 原生验证仍待完成。

**Dependencies:** H1,H2。

### Task P24.a: 核实模型字段与 Agent 参数映射

**状态：** 已有证据供 P24.b–d 按字段实施；未核实项继续记录，不作全部字段的前置阻塞。维护者 2026-10-20 决定复用先前调研，遇到实际冲突或已知弃用再针对性查证。[逐字段证据矩阵](../integrations/model-parameter-mapping.md) 记录本机 Claude Code 2.1.304、Pi 1.3.0、Grok Build 1.0.62 与剩余缺口，整项验收暂不勾选。

**Description:** 建立三个写入适配器的逐字段证据矩阵，要求见 [model-parameters.md](model-parameters.md#p24-后续模型信息和使用设置写入-agent)。

**Acceptance criteria:**

- [ ] 核对官方文档与安装版本，覆盖上下文、最大输出能力、请求输出限制、推理支持/可选档/默认档/选用档及其它使用设置。
- [ ] 记录每个 Agent/协议的目标字段、优先级、来源、限制及 supported/unsupported/unverified；区分 Provider API ID 与 Agent alias。
- [ ] 给出各 Agent 最小可交付字段与拒绝原因，不猜测 effort 与 token 数量转换或未知档位。

**Verification:** 复核文档与安装 schema/随版本手册；不读取或输出真实凭据，不启动推理。

**Dependencies:** P16,P23。**Files:** `docs/integrations/`、`docs/plans/model-parameters.md`。

### Tasks P24.b–P24.d: 分 Agent 交付模型信息与使用设置

P24.b = Claude Code；P24.c = Pi；P24.d = Grok Build。每个任务单独实施和验收，必要时按字段继续拆批。

- [x] **P24.b 首批：上下文容量。** 将中央保存的 DeepSeek `deepseek-flash`（无 `[1m]`）容量写入 `env.CLAUDE_CODE_MAX_CONTEXT_TOKENS`；预览/详情/应用使用同一模型快照，其他 ID 保留窗口并说明未映射原因，不改压缩开关。修复重复预览中旧凭据值未脱敏的问题。Rust 600 项通过、1 忽略，Clippy/格式/完整前端（1059 项 UI 测试）与锁定桌面构建通过；隔离 macOS Tauri 14 项检查覆盖容量变更、旧预览拒绝、版本恢复、原文件还原与合成凭据清理，中英浏览器与 720×560 中文、键盘检查通过。真实请求、维护者桌面目视与 Windows/Linux 仍未验证。
- [x] **P24.b 第二批：显式推理档位。** 仅对 DeepSeek `deepseek-flash`、推理支持声明及明确档位列表，提供与原生 env 通道的交集（含 `max`）；默认不覆盖，模型声明的默认值不自动选用，未知/`any`/不兼容值不扩展。选择绑定预览、完整详情 IPC、审批、应用与版本恢复，切换 Provider/model 清空草稿，模型元数据变化拒绝旧审批。双语说明 env 可能影响其他模型及会话可能降档，不声称原生或独立偏好隔离。Rust 607 项通过、1 忽略，前端 1072 项 UI 测试及格式/lint/i18n/类型/构建、Clippy 与锁定桌面构建通过；隔离 macOS Tauri 验证 `max`/`high` 写入、版本恢复、元数据过期和原文件按字节还原，合成 Provider/OS 凭据已删除；中英 720×560、键盘与同名跨账户草稿清空检查通过。修正既有 Agent 路径测试在初始发现未结束时点击禁用按钮的时序，不放宽断言。真实推理、维护者桌面目视与 Windows/Linux 未验证。
- [x] **P24.b 第三批：独立记忆档位起始偏好。** Claude Code 专属 SQLite v26 表按 Provider 实例与原始模型 ID 保存，显式“记住此选择”独立于预览/应用/历史恢复，不写 Agent 文件或访问凭据。读取同一事务快照的模型元数据与偏好，保存同时比较模型摘要和随机偏好版本；清空已有值时保留新版本，模型/Provider 删除级联清理对应偏好。界面支持读取/重试、冲突后重新读取、失效档位明确标记与预览阻止，切换/卸载后忽略旧响应；快速返回尚未重新读完的旧绑定时清空未记住的草稿并等待新读取，已补先失败后通过的回归及延迟读取浏览器验证。仅是 vibemate 起始偏好隔离，不承诺原生 env 隔离，也不替代 P35。完整前端检查通过（1103 项测试）、Rust 617 项通过和 1 忽略、格式/Clippy 与锁定桌面构建通过；隔离 macOS Tauri 22 项检查含真实 IPC、保存/清空、过期令牌拒绝、模型过期、同名账户隔离、删除重建与进程重启持久化。测试未创建或访问 OS 凭据，原生配置按字节未变，隔离应用数据已清理；中英 720×560、过期偏好与键盘检查通过。真实推理、维护者桌面目视与 Windows/Linux 未验证。
- [ ] **P24.b 后续：** 其它已调研模型窗口、其余 Claude Code 局部设置、档位及输出限制逐批交付；以上三批不代表 P24.b 整项完成。

**Acceptance criteria（每个 Agent 分别满足）:**

- [ ] 将 P24.a 已核实支持的上下文、最大输出、推理声明/档位等映射到原生配置；明确不能表达的字段，不静默丢失。
- [ ] 配置界面查看来源、编辑已支持的 Agent 局部设置并选择推理档位/输出限制；支持不覆盖/继承，预览准确，未知/不支持给出原因。
- [ ] 完成 Rust/IPC/双语 UI、过期保护、保留未拥有字段以及 H1–H3 历史操作；同名跨 Provider 模型与原生别名隔离。

**Verification:** 各 Agent 分别运行映射与 UI 行为测试、完整前端/Rust 检查和锁定桌面构建；真实调用单独记录。

**Dependencies:** P24.a 的已有逐字段证据、H1/H2/H3；按 P24.b → P24.c → P24.d 顺序交付，不因其它字段尚未补证阻塞已调研字段。

**Files:** 对应 `src-tauri/src/agents/` 适配器及测试、`src/features/agents/`、IPC、翻译和集成文档。

### Task P24: 验证 Provider 与 Agent 组合

**Description:** 在 P24.a–P24.d 实现后，检查三家 Provider × 三个 Agent 的实际兼容矩阵与切换行为，不承诺不存在的协议转换。

**Acceptance criteria:**

- [ ] 每个组合有 supported/unsupported/unverified 状态及原因、版本与证据。
- [ ] 切换账户或模型保持稳定绑定，并正确提示项目/环境变量覆盖。
- [ ] 回归工具调用、推理/图像参数等已支持能力，未支持项不被静默丢弃。

独立最小推理检查已按维护者决定取消，不实现检查按钮或测试请求；真实调用由维护者在注入 Agent 后验证。

**Verification:** 运行映射样本与 UI 切换测试、完整项目检查；真实用例仅使用许可的测试凭据。

**Dependencies:** P12,P15,P16,P20,P23,P24.b,P24.c,P24.d。**Files:** `docs/integrations/compatibility.md`、`src/features/agents/`。

### Checkpoint C24（P22–P24）

- [ ] H1–H3、P24.a–P24.d 与 P24 均已勾选；最小写入验收不等于历史管理与参数映射完成。

## 阶段 F：Skill

部署目标顺序与配置适配一致（Claude Code、Pi、Grok Build）。中央库行为见 [skills.md](../skills.md)。

### Task P26: 从已确认来源安装 Skill

**Description:** 在隔离目录获取 GitHub 或经核实的安装器结果，再加入中央 Skill 库。

**Acceptance criteria:**

- [ ] Git 来源固定 revision；npx 来源记录准确安装器/版本/命令和退出状态，执行前显示命令。
- [ ] 不执行 Skill 内容中的命令；校验路径遍历、失效来源和结果格式。
- [ ] 失败不破坏已有库；成功记录来源及可更新方式，不声称未知来源可更新。本地导入的技能在来源页补齐地址与获取方式后写入同一字段，供 P29 使用。

**Verification:** Rust 模拟源/安装器测试与安装状态 UI 测试；`check:frontend` 与 Rust fmt/Clippy；使用已确认测试来源检查隔离安装和失败清理。

**Dependencies:** P25（已完成）。

**Files:** `src-tauri/src/skills/install.rs`、`desktop/skills.ts`、`SkillLibrary.tsx` 与测试；获取方式字段需一次迁移。

### Task P27: 向 Pi 部署 Skill

**Acceptance criteria:**

- [ ] 预览新增/冲突与目标路径，symlink 优先；不可用时让用户选择 copy-sync。
- [ ] 保留同名用户内容，记录所有权；禁用只撤销自己管理的目标。
- [ ] 报告目标已同步的 revision，失败可恢复，不把部分部署显示为整体成功。
- [ ] 在 Agent 详情汇总区添加 Skill 只读槽位（部署状态 + 跳转技能模块）。

**Verification:** Rust 临时目录/链接/复制测试与部署 UI 测试；本机 Pi 能发现测试 Skill，卸载不损害已有内容。

**Dependencies:** P02,P19,P25。

### Task P28: 向 Grok Build 部署 Skill

**Acceptance criteria:**

- [ ] 仅对已确认支持的格式部署，未知能力明确为待决，不创建无效目录。
- [ ] 正确处理目标冲突、symlink 权限与 copy-sync，并记录部署 revision。
- [ ] 撤销、重试和部分失败不会影响其他 Agent 或用户 Skill。

**Verification:** Rust 与部署 UI 回归测试；真实 Agent 支持时检查可发现性，缺口由维护者决定。

**Dependencies:** P03,P27。

### Task P29: 预览 Skill 更新

**Acceptance criteria:**

- [ ] 只对来源明确的 Skill 检查更新，Git/npx 使用已记录的获取方式和版本。
- [ ] 比较原 revision、中央本地修改与候选内容，显示差异与可选处理策略。
- [ ] 更新预览不写中央或 Agent 目标；超时/失效/版本变化有明确错误。

**Verification:** 三方内容样本测试与更新预览 UI 测试；`check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P26,P27,P28。

### Task P30: 应用 Skill 更新

**Acceptance criteria:**

- [ ] 应用前重新检查内容摘要，不能覆盖未处理的本地修改或过期计划。
- [ ] 中央与链接/复制目标按操作日志更新，显示逐目标状态；部分失败可恢复。
- [ ] 失败和回滚保留来源/revision/所有权一致性，不删除其他 Agent 内容。

**Verification:** 故障注入与提交/恢复 UI 测试；Tauri 测试样本和一次 copy-sync 部分失败恢复。

**Dependencies:** P19,P29。

### Checkpoint C30（P25–P30）

- [ ] P25–P30 均已勾选。

## 阶段 G：MCP

中央 MCP 行为见 [mcp.md](../mcp.md)。

### Task P32: 向 Pi 注入 MCP 配置

**Acceptance criteria:**

- [ ] 原生支持时按实际格式预览并应用，保留无关字段和已有 MCP 条目。
- [ ] 凭据在 Rust 中解析，差异脱敏，写入/撤销/恢复复用安全机制。
- [ ] 无原生支持时明确标记为不支持、不实现桥接（维护者决定 2026-10-13）。
- [ ] 在 Agent 详情汇总区添加 MCP 只读槽位（部署状态 + 跳转 MCP 模块）。

**Verification:** Rust 映射/恢复测试及部署 UI 测试；真实 Pi 用无敏感测试服务器确认发现/禁用。

**Dependencies:** P02,P19,P21,P31（已完成）。

### Task P33: 向 Grok Build 注入 MCP 配置

**Acceptance criteria:**

- [ ] 原生支持时独立映射传输与参数，不能照搬其他适配器配置或丢弃不支持字段。
- [ ] 预览、应用、撤销与恢复保持原配置、所有权和凭据脱敏。
- [ ] 无原生支持时明确标记为不支持、不实现额外桥接（维护者决定 2026-10-13）。

**Verification:** Rust 与部署 UI 回归测试；真实 Grok Build 验证测试服务器可用性与恢复结果。

**Dependencies:** P03,P23,P31。

### Checkpoint C33（P31–P33）

- [ ] P31–P33 均已勾选。

## 阶段 H：维护与验收

### Task P34: 恢复未完成操作

**Description:** 在启动时发现上次中断的 Agent 配置或 Skill/MCP 部署操作，提供明确的恢复入口，不静默重放敏感操作。Agent 配置的中断核对与显式解决已交付（见 H1），本任务补 Skill/MCP 操作类型与统一入口。

**Acceptance criteria:**

- [ ] 显示未完成操作的脱敏状态、受影响目标与可恢复动作，不把部分成功显示为全部完成。
- [ ] 恢复前核对目标和中央状态变化，冲突时保留当前内容并给出处理步骤；重试幂等。
- [ ] 恢复失败仍可继续处理，日志与引用一致，未知操作版本拒绝自动执行；只操作 vibemate 管理的条目。

**Verification:** 中断/重启/冲突/重复重试测试及 RecoveryView 行为测试；临时配置模拟一次中断并在重启后完成恢复。

**Dependencies:** P21,P23,P30,P33。

### Task P35: 约束被引用配置的删除

**Description:** 完善 Provider/Model/Skill/MCP 的删除行为，防止已有 Agent 引用突然失效。

**Acceptance criteria:**

- [ ] 展示被引用关系，禁止静默级联删除或自动重写全部 Agent 配置。
- [ ] 用户选择先解除绑定/撤销受管部署，再按对应机制清理；失败保留可恢复状态。
- [ ] 凭据清理与数据库删除遵守补偿约定；用户文件和共享凭据不误删。

**Verification:** 引用/清理失败测试和删除确认 UI 测试；`check:frontend` 与 Rust fmt/Clippy。

**Dependencies:** P21,P23,P30,P33。

### Task P36: 检查前端实际可用性

**Description:** 用真实业务页面检查长内容、键盘、分页与错误状态，修复有证据的问题。

**Acceptance criteria:**

- [ ] 中英文在 720×560、缩放和长 URL/模型 ID 下可操作，列表加载/空/失败/分页有明确反馈。
- [ ] 表单与确认界面键盘可达、焦点恢复正确，状态不只靠颜色；符合严格样式规则。
- [ ] 仅修复发现的问题；跨多个 feature 时先按页面拆子任务。

**Verification:** 受影响页面行为测试与 `pnpm run check:frontend`；ego-browser 检查最小窗口、键盘、长内容与错误状态；实际 Tauri 确认原生行为。

**Dependencies:** P34,P35。

### Task P37: 验证三平台系统行为

**Acceptance criteria:**

- [ ] 原生验证路径/大小写、权限、替换、链接/复制和失败恢复，不用交叉编译冒充平台证据。
- [ ] macOS/Windows/Linux 的凭据不可用路径可测，真实 OS 密钥库的交互验证单独记录。
- [ ] 检查四发布目标的依赖与运行条件，CI 结果和未完成手工项有清楚状态。

**Verification:** 完整前端与 Rust 检查及本机原生构建；核对三个原生 CI 结果；人工验证各 OS 的权限/密钥库/安装条件。

**Dependencies:** P20,P23,P30,P33,P35。

### Task P38: 完成真实接入验收

**Acceptance criteria:**

- [ ] 三 Provider/三 Agent 每个受支持组合有版本和实际结果，不把 unsupported/unverified 当通过。
- [ ] 验证原生启动、模型参数、回滚、Skill 来源更新与 MCP；记录未实现的桥接缺口。
- [ ] 真实请求在允许范围内，使用临时或可恢复配置，不将密钥或敏感输出放入记录。

**Verification:** 按 documented matrix 人工完成真实 Agent/Provider 验收；运行完整项目检查。

**Dependencies:** P24,P30,P32,P33,P34,P37。

### Task P39: 准备首个草稿 Release

**Acceptance criteria:**

- [ ] 版本、锁文件和 CHANGELOG 条目一致；更新说明不包含未实现功能或未通过的平台承诺。
- [ ] 在维护者授权创建远端/标签后运行四目标草稿构建，检查资产与安装启动结果。
- [ ] 签名/公证未配置时明确记录限制，维护者检查后决定是否发布，不默认启用应用更新。

**Verification:** `pnpm run check:frontend`、完整 Rust 检查和发布说明预览；检查四个真实 Actions job 与安装包。

**Dependencies:** P36,P37,P38。

### Checkpoint C39（P37–P39）

- [ ] P37–P39 均已勾选；可打第一阶段 release 标签。

## 后续后端与产品扩展

BR0–BR5 已完成；现行边界见 [backend-modularity.md](backend-modularity.md)。以下不是六 Agent 发现的额外前置，维护者已决定先不排期：

- [ ] **BR6：Catalog/Storage/Credentials。** 各厂商解析局部化；迁移注册顺序和已发布 SQL 保持原样；按需要分离 OS store 实现和各业务补偿策略。
- [ ] **BR7：外部服务适配。** GitHub 客户端/账号支持备份目标操作；Vercel 用途待确定。
- [ ] **BR8：配置备份恢复设计。** 版本化非敏感快照、GitHub 目标/路径、远端修订冲突、凭据重新关联与恢复差异。
- [ ] **BR9：MCP/Skill 来源安装设计。** 首个实际来源、固定 revision、安装计划/执行/更新/卸载、受管路径和恢复。路线 A 已确认：更新能力以「来源是否明确」为条件。
- [ ] **BR10：API 转发设计。** 首条同协议转发、gateway 生命周期、路由、协议适配、流式/工具调用/取消/错误验收。
- [ ] **BR11：Agent 安装设计。** 首个 Agent 的安装/升级/卸载契约、目标平台/架构与现有安装来源；安装完成后只检测，不自动启动或应用配置。
