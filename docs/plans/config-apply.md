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

1. 读取目标，复用 `config_plan::read_target_document`：存在性、SHA-256、符号链接标记、1 MiB 上限。
   两边共用同一个读取器，避免「计划认为未变、写入认为已变」的分歧。
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

后续批次：P20.b IPC 命令与凭据锁、P20.c 前端边界（严格载荷校验）、P20.d 确认界面与双语文案。
命令层与应用步骤共用应用级凭据锁，因此密钥替换与配置应用不会交错；界面明示密钥将以明文写入
`~/.claude/settings.json`（预览已有该提示），且不依赖 vibemate 启动 Agent。

## 检查点 C19

- P19.a 与 P19.b 的故障注入测试通过：并发变化、符号链接、只读目标、替换失败与部分提交。
- 备份权限与内容不外泄有测试；成功与失败都不残留临时文件。
- 原生 macOS 行为与真实 Agent 文件由维护者验收；跨平台差异在 P37 记录。
