# P19–P21：配置写入、应用与回滚

任务状态只记录在 [todo.md](todo.md)，本文件记录设计与依据。首个使用方是 Claude Code（P18 预览），
其后是 Pi、Grok Build 适配器、Skill 部署（P27）与 MCP 注入（P32）。

## 范围

本模块把已经预览过的计划落到磁盘，并且只在能证明安全时落盘：应用前重新核对计划令牌与目标身份、
同目录临时写入后原子替换、替换前留下私有备份、失败时给出明确状态。它不生成计划内容
（`config_plan.rs`）、不读取凭据（调用方在应用阶段从 OS 凭据存储取值）、不含界面：确认界面属 P20，
回滚视图属 P21，未完成操作的恢复入口属 P34。

安全规则：

- 不跟随目标路径最后一段的符号链接：链接目标可能已被替换，原子替换会写到别处。
- 不静默改变用户文件权限：已存在的目标沿用其权限位，新建目标默认 0600（可能承载凭据）。
- 只读目标拒绝替换，即使目录可写时 `rename` 仍能成功。
- 错误与日志只带稳定错误码和路径，不带文件内容或值。备份文件可能含用户已有密钥，因此只存放在
  私有目录（0700 / 0600），界面只拿到可定位路径与摘要。

## 单文件写入顺序

1. 读取目标，复用 `config_plan::read_target_bytes`：存在性、SHA-256、符号链接标记、1 MiB 上限。
   两边共用同一个读取器，避免「计划认为未变、写入认为已变」的分歧。写入器只比较精确字节，
   **不解析文档**：解析属于各自的适配器，否则第一个适配器的 JSON 假设会变成后来者的门槛
   （Grok Build 的设置是 TOML，按 JSON 解析会直接被当成不可读目标）。
2. 拒绝符号链接、只读目标与非普通文件。
3. 比对计划令牌：文件出现、消失或内容变化一律返回 `target_changed`，绝不覆盖。
4. 新内容与现状一致时返回 `changed: false`：不写盘、不备份、不改时间戳，连续提交因此安全。
5. 目标存在时先把当前字节写入私有备份（`<backups>/<id>/<文件名>`，目录 0700、文件 0600）；
   备份失败则不写目标。
6. 同目录创建临时文件（`.<文件名>.vibemate-<id>.tmp`，权限沿用目标或 0600），写入后 `sync_all`，
   再 `rename` 覆盖目标；成功后同步父目录（Unix）。
7. 重新读取目标并回报新摘要；重读失败或摘要不符返回 `outcome_unknown`，不报成功。

`rename` 在同一文件系统内原子，所以目标文件只会是旧内容或新内容，不会出现半截文件。Windows 上
`rename` 同样替换已存在的文件，但目标被其他进程独占打开时会失败；平台差异在 P37 记录。

## 多文件操作（P19.b）

一个计划可能覆盖多个目标（Pi 的设置与模型定义、技能部署路径、MCP 配置）。多文件按步骤应用：
每步记录目标、期望摘要、结果与备份引用；任一步失败后回滚已应用的步骤，无法回滚时保留明确的
未完成状态，绝不报告整体成功。操作与步骤写入 SQLite，重启后能列出未完成操作供 P34 处理；
补偿只操作 vibemate 自己写过的目标。

## 批次

### P19.a：目标校验、私有备份与原子替换（2026-10-14 交付）

`config_apply.rs` 与 `config_apply/tests.rs`：`WriteTarget::from_plan`（令牌直接取自计划的
`TargetFile`）、`BackupStore`、`apply_file`、`AppliedFile`、`WriteError`。故障注入测试覆盖并发变化、
文件出现/消失、符号链接、只读目标、目录目标、只读父目录导致的替换失败、备份唯一性与私密权限、
残留临时文件与错误不外泄内容。本批没有 IPC 与界面，用户可见变化随 P20 出现，因此
`CHANGELOG.md` 不新增条目。

### P19.b1：操作日志（2026-10-14 交付）

SQLite schema v16 新增 `config_operation` 与 `config_operation_step`，仓储在
`config_apply/operations.rs`，测试在 `config_apply/operation_tests.rs`：

- 每次操作在替换任何文件之前写入：操作行为 `planned`，每个目标一行 `pending` 步骤，含路径、
  期望摘要与时间戳。步骤按序号排列，序号必须连续。
- 步骤结果分开记录：应用成功写入新摘要与备份引用，失败写入稳定错误码；两个写入都在事务里
  同时刷新操作的 `updated_at`。
- 未完成 = 不是已知的已结束状态（`applied`/`failed`/`compensated`）。这样本版本不认识的取值会
  以 `invalid_stored_operation` 报出，而不是从列表里静默消失。
- 数据库只保存路径、摘要与错误码：有测试写入真实备份后扫描数据库文件，确认密钥不出现在其中。
- 读取重新校验每一行（身份形状、digest 格式、错误码形状、序号连续、状态与字段搭配、路径可显示），
  畸形行报错而不当作真实结果。

本批不含编排与补偿（P19.b2）与界面（P34）。

### P19.b2：多文件编排与补偿（2026-10-14 交付）

`config_apply/service.rs` 与 `config_apply/service_tests.rs`：

- `apply_files(storage, kind, files, backups)`：先用 `begin_operation` 记录整个计划，再按顺序对每个
  文件 `apply_file` 并记录该步骤的结果；全部成功记为 `applied`。
- 任一步失败时记录失败步骤与稳定错误码，然后调用 `compensate_operation` 回滚，并返回最终状态而不是
  错误：`failed` 表示没有任何文件被替换，`compensated` 表示全部回滚，`partial`／`unknown` 表示有文件
  无法回滚（例如用户后来改过它），此时操作留在未完成列表里供 P34 处理。只有操作日志本身不可信时才
  返回错误，因为那时无法诚实报告结果。
- `compensate_operation` 也可用于更早留下的未完成操作（P34 的恢复入口）：它按记录的步骤倒序处理，
  从不触碰未应用的步骤。
- `restore_file` 读取备份并校验其摘要，再通过 `apply_file` 写回，因此回滚同样受令牌检查、私有备份与
  原子替换保护；`remove_created_file` 只删除内容仍等于本模块写入值的文件，文件已不存在按成功处理。
- 回滚会丢弃它自己产生的那份备份（操作记录已说明发生了什么）；创建文件的那次写入没有备份，回滚时
  直接删除。

已知边界：步骤状态只有 `pending`／`applied`／`failed`，回滚结果记在操作状态上，不逐步骤标记；界面若
需要逐步骤结果，用 `ALTER TABLE ADD COLUMN`（v17）补一列即可。日志目前只增不减，已结束操作的清理
策略仍未定。

## P20：应用 Claude Code 配置

首个把计划写到磁盘的功能。Rust 侧（`agents/claude_code.rs`，P20.a 已交付 2026-10-14）：

- `apply_claude_code_settings(storage, credentials, input)`：先用与预览完全相同的 `build()` 重新读取
  当前 Provider、模型与设置文件，再把界面回传的四个令牌（目标路径、文件摘要、Provider `revision`、
  模型内容摘要）逐一比对；任一不符返回 `plan_stale`，不写任何字节。
- 令牌通过后才读凭据：引用名由存储的 Provider ID 生成（`provider-<id>`），取值放入计划显示的占位符
  位置。密钥不进入计划、返回值、错误或日志；返回值只带状态、逐文件结果与备份路径。
- 写入复用 P19 的 `apply_files`（操作类型 `claude_code_settings`），备份根目录为应用数据目录下的
  `backups/`，因此替换前的设置内容可在该目录定位，操作也会记入日志。
- 结果 `ClaudeCodeApplied`：`state`（`applied` 或回滚后的状态）、逐文件 `steps`（含 `errorCode` 与
  `backupPath`）、`backupPath` 与 `restartRequired`。文件级失败是结果而不是错误：界面按 `errorCode`
  翻译并提示重新预览或处理冲突。
- 错误码：`plan_stale`、`credential_missing`、`credential_unavailable`，加上预览已有的
  `gateway_unverified`／`base_url_unverified`／`target_unreadable`／`target_conflict`／
  `provider_not_found`／`model_not_found`／`location_unavailable`／`storage_unavailable`／
  `invalid_request`／`operation_failed`。

后续批次（均已交付 2026-10-14）：

- **P20.b IPC 命令与凭据锁**：`apply_claude_code_settings` 命令按严格载荷解析请求，在阻塞线程上持有
  应用级凭据锁读取密钥，备份目录固定为应用数据目录下的 `backups/`。
- **P20.c 前端边界**：`applyClaudeCodeSettings` 与 `reviewedPlanTokens`；响应按精确键集合校验（操作 id
  形状、状态闭集、步骤序号连续、步骤状态与错误码互相一致），并做三项跨字段检查：回报的备份属于某个
  步骤、`applied` 表示每个文件都已写入、`failed` 表示没有任何文件被写入。浏览器预览抛
  `desktop_required`，不发 IPC，也不声称已应用。
- **P20.d 确认界面与双语文案**：计划卡片新增「应用这些设置」按钮，应用期间按钮标记 `aria-disabled`
  并由同一个 pending 守卫拦住重复点击；结果卡片显示状态、失败步骤的文件与原因、备份位置，并且只在
  文件确实写入新键时提示重启会话。任何一次应用都会消费掉当前计划，再次写入必须重新预览。

命令层与应用步骤共用应用级凭据锁，因此密钥替换与配置应用不会交错；界面明示密钥将以明文写入
`~/.claude/settings.json`（预览已有该提示），且不依赖 vibemate 启动 Agent。

维护者已于 2026-10-14 验收真实 Tauri/Claude Code 中的原生应用与模型使用；Windows/Linux 行为仍在 P37 验证。

## P21：回滚 Claude Code 配置

目标：让用户看到 vibemate 写过的 Claude Code 配置操作，预览把某个操作写过的文件恢复成它替换前的内容，
冲突时拒绝覆盖。首个界面只服务 Claude Code；机制放在 `config_apply`，供之后的 Pi/Grok 适配器与 MCP 注入复用。

本模块仍只读写自己记录过的东西：恢复的前提是操作日志里有这个操作，用户手动改回的文件不会变成一条记录。

### 已确认决定（2026-10-14）

- **逐步骤回滚状态（schema v17）。** `config_operation_step` 新增 `restore_state`，取值为 `restored`
  （已写回原内容或已删除新建文件）、`conflict`（文件在写入后被改成别的内容，保持原样）、`unavailable`
  （备份缺失或摘要不符）、`failed`（写回没有完成）、`unknown`（结果无法核对）；空值表示还没尝试。
  没有这一列时，`partial` 操作重试补偿会把已回滚的步骤再恢复一次，`restore_file` 的令牌检查随即返回
  `target_changed`，操作永远停在 `partial`。回滚因此跳过已经是 `restored` 的步骤；每次尝试覆盖同一行，
  不保存尝试历史。
- **写入前是否存在也要记录（同一迁移）。** 一个应用成功的步骤可能没有备份：它新建了文件，或者发现计划
  内容已经在文件里（空操作）。两者在旧日志里长得一样，而把「空操作」当成「新建」会在恢复时删掉用户本
  来就有的配置文件。v17 因此还加了 `previous_existed`：`0` 表示该步骤新建了文件，`1` 表示文件当时已存在，
  空值表示旧版本写的行没记录这件事。空值不猜：预览把它标为 `blocked`，理由码 `restore_history_unknown`。
- **凭据只随文件内容回写。** 恢复不读取 OS 凭据、不改凭据引用：写回的就是备份里的原内容，可能仍含之前
  那家 Provider 的明文密钥。`config_operation` 新增 `owner_ref`（写入该操作的 Provider ID，可为空），
  恢复预览用摘要链判断「恢复后的内容由哪次操作写入」，据此说明密钥归属；判断不出来时明确显示未知。
  全程不读文件内容、不做密钥值比较。
- **日志只增不减。** 本次不删除已结束操作，也不加清理策略；界面按上限读取（默认 10 条，最新在前）。

### 回滚语义与记录

- 恢复**不新增操作行**：它更新原操作与其步骤（操作状态、`restore_state`）。如果把一次恢复记成新操作，
  恢复失败就会留下一个新的未完成操作，P34 的恢复入口会去「补偿」它，等于把用户刚撤掉的配置写回去。
- 原操作的备份保留：它记录的是那次应用替换掉的内容，恢复之后仍可再恢复或用于审计。只有**恢复过程中**
  新产生的副本（写回时被替换掉的当前内容）在写回成功后删除。
- 可恢复 = 至少一个步骤 `state = applied` 且 `restore_state` 不是 `restored`。`applied`、`partial`、
  `unknown` 与写了一半的 `planned` 都可能符合；`failed`（没有步骤写过盘）与 `compensated` 不符合。
- 清理失败不再静默：`BackupStore::discard` 改为返回结果，恢复结果带 `leftovers`（未能删除的副本路径），
  界面显示残留位置并提供继续恢复的入口。
- 竞态与重试：写回时若文件当前摘要已等于备份摘要，说明这一步已经被恢复过（另一次点击或另一次恢复），
  记为 `restored`；只有文件确实是第三种内容时才记 `conflict`。

### 恢复预览（只读）

`config_apply/restore.rs` 的 `plan_restore(storage, operation_id)` 读取操作记录并逐步分类；操作不存在时返回
`None`，由命令层报 `operation_not_found`：

| 步骤情况                                              | 预览动作   | 说明                             |
| ----------------------------------------------------- | ---------- | -------------------------------- |
| 当前摘要 = 记录的应用后摘要，且有可读且摘要相符的备份 | `restore`  | 写回备份内容                     |
| 同上，但该步骤当时新建了文件（没有备份）              | `remove`   | 删除 vibemate 新建的文件         |
| 文件存在但摘要不等于应用后摘要                        | `conflict` | 用户或其它工具改过，拒绝覆盖     |
| 备份缺失/摘要不符，或目标是符号链接、超限、不可读     | `blocked`  | 带稳定原因码与处理提示           |
| 没有备份、旧日志未记录写入前是否存在                  | `blocked`  | 理由码 `restore_history_unknown` |
| 没有备份，且旧日志说明文件当时已存在（空操作）        | `skip`     | 没有需要恢复的内容               |
| 步骤不是 `applied`，或已经是 `restored`               | `skip`     | 不触碰                           |

预览只返回路径、摘要、动作与状态，以及由 `owner_ref` 解析出的密钥归属说明；**不返回备份内容**。
摘要不是秘密，路径是用户自己的文件路径，因此历史列表同样只带这两类信息。

### 恢复应用

`apply_restore(storage, operation_id, reviewed, backups)` 先用与预览相同的代码重新构建计划，再逐项核对
界面回传的令牌（步骤序号 + 预览时的当前摘要），任一不符返回 `plan_stale` 且不写任何文件。通过后按应用顺序的
**倒序**处理可恢复步骤，每步写入 `restore_state`；`conflict`、`blocked`、`skip` 步骤不动。操作最终状态：
全部恢复为 `compensated`，否则 `partial`；任何一步结果无法核对时为 `unknown`。返回值带逐步骤结果、操作状态与
`leftovers`。

`compensate_operation` 保持原语义（自动回滚未完成操作，事务失败后调用），但改为复用同一份逐步骤回滚实现，
因此两条路径的冲突判定与记录方式一致。

### 命令与界面（P21.c / P21.d）

- `commands/agents.rs`：`claude_code_operations`（脱敏历史）、`plan_claude_code_restore`、
  `apply_claude_code_restore`。三个命令都走 `with_restore_storage`；恢复不读凭据，所以不取凭据锁。
  当前日志只认得 `claude_code_settings` 一种 kind，未知 kind 的行按 `invalid_stored_operation` 拒绝，
  因此没有「恢复别的功能的操作」的路径；第二个 kind 接入时各适配器必须按自己的 kind 过滤。
- `src/lib/desktop/agents.ts`：三个边界函数加精确键集合与跨字段校验（`restorable` 必须等于实际会改动文件
  的步骤数、`reason` 只出现在 `conflict`/`blocked`、`errorCode` 与 `restored` 互斥、`leftovers` 与步骤回报
  一致）；浏览器预览返回 `{ kind: "preview" }`，不声称可恢复。
- `src/features/agents/RestoreView.tsx`（P21.d 交付）：历史列表（状态、本地时间、服务商、文件、
  「预览恢复」入口与「重新检查」）、恢复预览（逐步骤动作与理由、冲突处理提示、密钥归属、确认按钮）、
  结果（状态、逐步骤结果、残留副本路径与说明、重新预览提示）。只有 `restorable` 的行才提供预览入口，
  预览里没有可恢复步骤时不显示确认按钮，因此「已回滚」的行不会被误点。
- 错误码：沿用 `WriteError` 的稳定码，命令层新增 `operation_not_found`；过期预览沿用 P20 的 `plan_stale`；
  预览理由码新增 `restore_history_unknown`。

### 批次

- P21.a 设计与本文件（2026-10-14）。
- P21.b1 schema v17（`owner_ref`、`restore_state`、`previous_existed`）、日志读写与迁移测试
  （2026-10-14 交付）。
- P21.b2 `config_apply/restore.rs`：恢复预览、恢复应用、共享回滚实现与故障注入测试（2026-10-14 交付）。
- P21.c 三个命令（`claude_code_operations`、`plan_claude_code_restore`、`apply_claude_code_restore`）、
  `with_restore_storage`、前端边界与双端测试（2026-10-14 交付；无界面，CHANGELOG 未新增条目）。
- P21.d RestoreView、双语文案与行为测试（2026-10-14 交付）。
- P21.e 原生验收已通过（维护者 2026-10-16 确认）：临时配置 apply → restore → 原生启动。

已知边界：恢复只覆盖 vibemate 记录过的操作；`planned` 操作可能从未写盘，预览按步骤状态如实显示；
写入失败时 apply 阶段为被替换内容留下的那份副本不再被任何步骤引用，清理失败也不会进入恢复结果
（恢复路径自己产生的副本会进 `leftovers`）；跨平台替换、权限与文件锁差异在 P37 记录。

## 检查点 C19

- P19.a 与 P19.b 的故障注入测试通过：并发变化、符号链接、只读目标、替换失败与部分提交。
- 备份权限与内容不外泄有测试；成功与失败都不残留临时文件。
- 原生 macOS 行为与真实 Agent 文件由维护者验收；跨平台差异在 P37 记录。

## 检查点 C21

- P21 的预览、冲突判定、逐步骤回滚状态与残留清理结果都有故障注入测试。
- 恢复预览与结果不包含文件内容；过期预览被拒绝且不写盘。
- 维护者于 2026-10-16 确认在真实 Tauri/Claude Code 中的 apply → restore → 原生启动验收通过，C21 关闭；跨平台差异仍在 P37 验证。

## Pi 复用（PI1）

Pi 的最小配置预览、原配置备份与应用作为一个任务交付，状态见 `todo.md`。
`OperationKind::PiSettings` 使用同一日志与补偿机制。凭据写入模型目录时采用私有写入模式，
Unix 替换文件为 0600；Claude Code 原有写入模式保持既有约定。历史列表、恢复预览与恢复应用均按
Agent 操作类型隔离；恢复结果按实际执行的倒序步骤校验，不把步骤编号当作数组位置。

## Grok Build 预览（P22）

Grok 把所有用户设置放在一个 `config.toml` 里，所以本适配器的计划只有一个目标文件。
`config_plan::read_target_bytes` 从原来的 JSON 读取器里拆出「文件身份 + 精确字节」这一步，
JSON 与 TOML 读取器共用它，避免「计划认为未变、写入认为已变」的分歧在第二种格式上重现。

- **保留原文件形态**：解析与合并用 `toml_edit`（已在锁定的依赖图里，不新增 crate 版本），
  它按文档结构改写目标字段，保留用户自己的注释、键顺序与无关表；换成纯值模型会丢掉这些。
  解析失败的文档直接拒绝，不改写、不「修复」。
- **合约检查跟着安装版本**：Grok 启动时把随版本发布的配置参考解包到
  `<root>/docs/user-guide/26-config-reference.md`，并写 `<root>/.metadata_version`。适配器要求
  下载文件名版本、`.metadata_version` 与该参考里的字段行三者一致，任一不符返回
  `agent_version_unverified`；两个文件都按有界普通文件读取，符号链接即拒绝。
- **别名归属**：`[model.<alias>]` 是配置身份。schema v19 `grok_model_binding` 按
  `(config_root, model_alias)` 记录中心 Provider/模型对；没有记录的同名条目属于用户，
  候选别名会跳过它另取一个，而不是接管。归属按配置根隔离，`GROK_HOME` 指向另一套安装
  时不会继承另一套的声明。
- **只写有文档依据的字段**：`model`、`base_url`、`name`、`api_backend`、`api_key` 与
  已声明的 `context_window`，加 `[models].default`。请求参数（`temperature`、
  `top_p`、`max_completion_tokens`）与 `reasoning_efforts` 留给 P24 的逐项核实映射。
  已有 `env_key` 的条目保留该行不改写，但明示 `api_key` 会接管认证来源。
- **会写但不能选中的组合先拒绝**：别名出现在 `[models].disabled_models`，或
  `[models].allowed_models` 非空且既不含别名也不含模型 ID 时，返回 `target_conflict`。
  这比写入后再告诉用户「写了但选不到」诚实。
- **未核实的协议不猜**：`api_backend = "messages"` 时官方只示范把 `x-api-key` 放进
  `extra_headers`，没有说明模型自己的 `api_key` 会被放进哪个请求头，因此
  Anthropic Messages 路由返回 `protocol_unsupported`。
- **覆盖来源只声明存在**：`managed_config.toml` 与 `requirements.toml` 只报告
  「不存在 / 存在 / 不可读」，不读内容——那里可能有另一家的明文密钥。`GROK_CONFIG`
  覆盖层与环境变量、CLI 参数在本批不探测，界面按「未读取的高优先级来源」说明。

### 应用与回滚（P23）

`OperationKind::GrokSettings`（`grok_settings`）复用同一份日志、私有备份与补偿实现；
`config_operation.kind` 只限长度、由 Rust 判定合法值，因此接入第三种操作类型不需要迁移。
应用先重建计划并逐令牌比对（路径、文件摘要、别名、Provider revision、模型摘要），任一变化
返回 `plan_stale` 且不写字节；随后才读 OS 凭据，把预览里唯一的掩码位置换成真实密钥，其余内容与
预览所见一致，由 `toml_edit` 负责引号与控制字符的转义。写入走私有模式（Unix 替换后为 0600），
原内容进私有备份，重读失败或摘要不符报 `outcome_unknown`。

别名归属只在步骤确实写盘（`applied`）之后记录；被拒绝或已回滚的写入不认领别名，因此下一次预览
不会去改一个从未被本适配器写过的条目。记录后即使恢复原文件也保留该行：它描述发生过什么，
并让后续预览沿用同一别名，而不是留下一份孤立条目和一个新名字。恢复按 Agent 隔离，
Claude Code 与 Pi 的入口拒绝 Grok 的操作 id，反之亦然。

界面侧：Grok 卡片的应用按钮与结果卡沿用 P20/P21 的机制，只有文案按 Grok 自己的说法
（`grokOutcomeApplied` 与 `grokRestartRequired`）；`RestoreView` 的命令名改为按 Agent 查表，
新增适配器不再往三个函数里各加一个三元分支。

仍未做：`requirements.toml` / MDM 与 `GROK_CONFIG` 覆盖层的内容检查（只报告存在）、
`[model_providers.<id>]` 共享块的接管、请求参数与推理档位映射（P24），以及本机真实 Grok
会话的应用/恢复验收。
