# 2026-07-16 架构与写法 Code Review（合并版）

> 本文档合并并取代两份既有文档：
>
> - `docs/plans/code-review.md`（三方合并 review，基线 `c3aefc1`，含前端 + 后端深读）；
> - `docs/plans/2026-07-15-resolve-review-open-questions.md`（修复计划，**已核验未执行**）。
>
> 上述两文档的所有发现已于 2026-07-16 对**当前代码**重新逐条核验，行号已更新为当前值。原文档已删除，本文件为唯一合并版。

## 审查范围与方法

- **范围**：当前代码库整体架构与写法（前端 Svelte 5 / SvelteKit / TypeScript，后端 Tauri / Rust），按热点加权。
- **方法**：直接读源码 + grep + 跑权威 lint，逐条核验。**注意**：本次曾尝试用后台子代理并行深审，但该机制在本环境未真正运行（4 个代理 transcript 全 0 字节、`TaskOutput` 返回 "No task found"、无完成通知），故全部发现由人工直接阅读得出，可逐条回溯。
- **直接深读**：治理文档、eslint 配置、`index.css` token、校验边界层（`apiResponse.ts` / `tauri.ts`）、sync 数据流、应用外壳路由、`OnboardingPage`、`SyncPreviewPage`、`SettingsPage`、`syncStatus` lib；后端 `errors.rs` / `lib.rs`、`pack.rs`（`parse_central_directory`）、`sync_engine/apply.rs`（apply 事务顺序 / 恢复 journal / `updated_by` / `batch_apply`）、`github/repository.rs`（`check_vault` / `discover_single_repository`）、`github/credentials.rs`（写路径 401 重放）。
- **框架适配**：前端 2026-07 已从 React 迁至 Svelte 5 / SvelteKit，审查用 Svelte 等价规则（runes、svelte-query、shadcn-svelte）取代 React 规则；规范以 AGENTS.md / CLAUDE.md 为准且具约束力。

## 地面真相（lint 全绿）

- `npm run lint`（typecheck + eslint + responsive + colors + i18n）**通过**。eslint 自定义规则强制分层 `shared -> modules -> app -> routes`、跨模块 deep-import 禁止、`export *` 禁止、循环/自导入禁止--均无违例。
- `rust:clippy --all-targets --all-features --locked -D warnings` **通过**。
- `knip`（手动，不在 pre-commit）报 2 处未使用导出。
- `src-tauri/src/lib.rs:1-4` `#![cfg_attr(not(test), deny(clippy::expect_used, clippy::panic, clippy::unwrap_used))]` **强制生产代码不得 unwrap/expect/panic**。注意：此规则只拦 `.unwrap()`/`.expect()`/`panic!()` 宏，**不拦数组切片下标越界 panic**（见 P1 第 1 条）。
- `src-tauri/Cargo.toml` 无 `[profile.*] panic` 设置 -> 默认 `panic="unwind"`。

> 含义：lint 能抓的规范问题基本干净。本审查价值集中在 lint 抓不到处--架构质量、正确性、thin wrapper、冗余防御、不可信边界解析、恢复事务一致性。

## Findings

### P0

无。凭据（`SecretString` + keyring）、token 双层脱敏（后端 `errors.rs` 序列化 `redact_sensitive` + 前端 `tauri.ts` 日志 `redactLogMessage`）、Tauri 边界响应端 zod 校验、blob sha256 校验、乐观并发均稳健。

### P1

- **[`src-tauri/src/pack.rs:484-521`] `parse_central_directory` 对不可信 zip central directory 做无边界检查的切片，畸形/恶意输入会 panic。**
  循环守卫（`:490` `pos + 46 <= bytes.len()`）只保证固定 46 字节头在界内，但 `name_len` / `extra_len` / `comment_len` 取自头部字段（各可达 65535），随后 `:502-506` 直接切片 `bytes[name_start..name_start+name_len]`、`[..+extra_len]`、`[..+comment_len]`，**从不校验** `name_start+name_len+extra_len+comment_len <= bytes.len()`。构造一个 CD 头声明超长字段的畸形 zip 即触发 slice 越界 panic。可达性：`unpack_skill` 在 `execute_download` 中解包 `fetch_blob` 返回的 blob；`fetch_blob` 只校验 `sha256(bytes)==manifest hash`，故恶意/被攻陷远端 vault 可放入「hash 匹配但 CD 畸形」的 blob 通过校验后进入此 parser。
  **当前缓解**：`unpack_skill` 运行在 `spawn_blocking` 内，默认 `panic="unwind"`，panic 被映射为 `AppError::Vault` 降级为一次下载失败，不崩进程。clippy `deny(unwrap_used)` 不拦切片下标 panic，故 clippy 通过与此不矛盾。
  **风险**：(1) 依赖「unwind + 始终在 spawn_blocking 内」两个隐式前提--`parse_central_directory` 是 `pub(crate)`，未来任何直接调用点会真 panic；(2) 一旦为减体积设 `panic="abort"`（release 常见），同路径立即变硬崩溃 DoS（升级 P0）。建议：读取 name/extra/comment 前显式校验越界返回 `AppError::Vault`，与 `le_u16`/`le_u32`（`:523-535`）同模式。

- **[workspace-ready 门控逻辑四处重复（已漂移）]** `src/routes/+page.svelte:14-21`、`src/routes/app/+page.svelte:13-20`、`src/routes/app/+layout.svelte:19-28`、`src/modules/onboarding/pages/OnboardingPage.svelte:431-435` 各自查询 `['app-state']` 并复算 `isWorkspaceReady` 做 sync/onboarding 重定向。`app/+layout.svelte` 是唯一真正守门人，其余三处冗余复刻且已漂移：root 有错误卡片+重试，`/app/+page.svelte` **完全无错误分支**（模板恒显 spinner，appState 出错时 `$effect` 因 `!data` 提前 return 不跳转 -> 理论无限转圈，仅因 layout 守门未触发）。
  注：`['app-state']` query 声明出现在 5 处（含 SyncPreviewPage）是**正常**的--svelte-query 按 queryKey 共享缓存；问题只在「重定向逻辑」被复制 4 份。建议：root `/` 改 `goto('/app')`、`/app` 索引改 `goto('/app/sync')`、删 `OnboardingPage:431-435` 重复 `$effect`，判定唯一收敛到 `app/+layout.svelte`。

- **[`src-tauri/src/sync_engine/apply.rs:393-928`] `apply_plan` 是 ~536 行的单函数。** apply.rs 总 2449 行里 tests 占 1003-2449（~1446 行，30+ 场景，质量高），生产 ~1002 行；但 `apply_plan` 单函数 536 行，内含 load/validate/clone/working-state/远端结果不明/本地提交/恢复多阶段分支。请求校验块与四类工作项收集循环可抽成 `validate_apply_request` / `collect_work_items` 等纯函数，使校验分支无需搭 store/journal 即可单测。`merge_plan`（`plan.rs`）同理可把 identity/collision/size/root-health 阻断遍历合并。建议：提取校验与收集逻辑，主函数保持线性编排。**复现项**：2026-07-14 review 已标（彼时 ~490 行），至今未修且更长，本次升级 P1。

### P2 -- 前端（TS/Svelte）

- **[`src/app/router/routeConfig.ts:4-6`] 对 `isWorkspaceReady` 的 thin re-export。** 从 `@/shared/lib` 导入后原样 `export`，无路由语义。结果 route 文件从 `@/app/router/routeConfig` 导入它，而 `OnboardingPage.svelte:8` 从 `@/shared/lib` 导入同一函数--同谓词两个导入来源。建议：删 re-export，route 文件直接从 `@/shared/lib` 导入。

- **[`src/modules/settings/index.ts:2`] `getAppState` 横切关注点错位在 settings（与 P1 门控同症候）。** 定义于 `settings/api/configApi.ts:12`，被 5 个非 settings 调用方消费：`SyncPreviewPage.svelte:22`、`routes/+page.svelte:6`、`routes/app/+layout.svelte:8`、`routes/app/+page.svelte:6`、`OnboardingPage.svelte:7`。经 index 导出不算违规，是布局选择。建议：把 `getAppState`（及同类 app-state 读取）提升到 `shared` 客户端。**复现项**：2026-07-14 review 已标，未修。

- **[`src/modules/settings/schemas/config.ts:1-3`] 与 [`src/modules/onboarding/schemas/onboarding.ts:1-25`] 纯透传 schema barrel（空抽象）。** 两文件只把 `@/shared/schemas` 的 schema/type 原样再导出，无模块本地 schema；`configApi.ts` / `onboardingApi.ts` 经 `../schemas/...` 导入，凭空多一层间接。建议：API 文件直接从 `@/shared/schemas` 导入；仅当确有本地 schema 才保留文件（补注释说明是刻意 facade）。

- **[`src/modules/sync/schemas/syncPlan.ts:5`] 与 [`src/modules/skills/schemas/skill.ts:3`] `namespace` 枚举各自重复定义** `z.enum(['agents','codex','claude-code'])`。后端新增命名空间需同改两处，有漂移风险。建议：若确为稳定共享概念，提升 `shared/schemas` 统一维护。

- **[冗余 request 端 zod 校验，且策略不一致]** `src/modules/sync/api/syncApi.ts:22,32`、`src/modules/settings/api/configApi.ts:22-33`、`src/modules/onboarding/api/onboardingApi.ts:85-102 及 113/124/133/144` 对已由 zod 保证的内部类型重复 parse：`resumeSyncRecovery(taskId: string)` 对已是 string 的 taskId 做 `z.string().min(1).parse`；`disconnectGithub` / `listInstallationRepositories` 对已 typed 为 number 的参数做 `z.number().int().nonnegative().parse`；`listGithubRepositoryBranches` / `checkGithubVault` / `initializeGithubVault` / `bindGithubVault` 对前端用已 typed 数据构造的 request 再 parse。这些标量/对象都来自上游已校验的 `appState` / `discovery` / `recovery.task_id`，Rust 侧还会自行 serde 校验，属重复防御，且不一致--`pollGithubDeviceFlow` 就没对 `deviceCode`/`interval` 同样处理。**响应端**校验（`*Schema.parse(await invokeCmd(...))`）正确，保留。另：`onboardingApi.ts:44-59` `normalizeGithubRepository` 处理 GitHub 多形态 payload 是真实边界规范化，保留。建议：删内部确定类型的 request 端重复 parse；若为 FFI 清晰报错保留，请统一策略并注明。**复现项**：2026-07-14 review 已标，未修。

- **[`src/modules/sync/pages/SyncPreviewPage.svelte:164-183,313-318`] `defaultNextPlan` 在「无变化 recheck」后残留为 true 的状态泄漏。** `handleRecheck`（`:314`）置 `defaultNextPlan=true`，但只有 `fingerprint !== lastFingerprint` 的 `$effect` 分支（`:173-182`）会复位 false。若 recheck 得到相同 fingerprint，标志残留 true，直到下一次 fingerprint 变化才被消费--届时对一次**非用户发起**的 plan 变化自动套用默认选择，覆盖用户手动勾选。当前 `refetchOnWindowFocus: false` + apply 成功后 `invalidateQueries` 前已 `clearInteractionState`，触发面窄；但属脆弱状态卫生。建议：recheck 完成后无条件复位 `defaultNextPlan`。

- **[`src/modules/sync/pages/SyncPreviewPage.svelte:288-290`] 直接改 store 内部 `$state` 绕过 API。** 为「取消决策」直接写 `syncDecisions.decisions = nextDecisions`，而 `SyncDecisionsState` 只暴露 `setDecision` / `clear`。建议：给 store 加 `removeDecision(id)`。

- **[`src/modules/sync/pages/SyncPreviewPage.svelte:89,455-507`] `scan` 查询错误未上报。** 模板有 `plan.error` 分支（`:465`）却无 `scan.error` 分支；scan 失败时 `scan.data?.skills ?? []` 静默回空，「已发现」显示 0 无提示，与 plan 错误处理不一致。建议：补 `scan.error` 分支或注释 scan 为 best-effort。

- **[`src/modules/settings/pages/SettingsPage.svelte:79-99`] 自动保存去抖：400ms 内「改后又改回」会持久化中间值，UI 与后端不一致。** `$effect` 在 `snapshot === lastSaved`（`:83`）时**先于** `clearTimeout`（`:88`）提前 return，故上次变更排定的定时器仍触发，把闭包捕获的中间值写入后端并置 `lastSaved=中间快照`。`config` 只 prefill 一次，随后 `invalidateQueries` 不覆盖本地状态，于是 UI 显示回退值、后端存中间值，直到下次编辑才纠正。触发条件低概率但确为真实数据不一致。建议：`clearTimeout` 移到提前 return 之前，或定时器回调内以最新 `effectiveConfig` 为准重读。

- **[`src/modules/sync/schemas/syncPlan.ts:43-77`] `syncSkillEntrySchema` 与 `conflictSchema` 字段大面积重复**（11 个共享字段）。建议：抽 shared base object 再 `.extend()`。

- **[knip 未使用导出]** `src/modules/sync/lib/syncStatus.ts:20` `SYNC_CHANGE_FILTERS`、`:27` `SyncChangeFilter`（后者仅 `:29` 内部派生 `SyncChangeCounts`，无外部消费者）。建议：取消 `export`，对外只留 `SyncChangeCounts` / `EMPTY_SYNC_CHANGE_COUNTS` / `countSyncChanges`。

- **[`src/shared/lib/tauri.ts:15,51`] `appErrorSchema.safeParse(raw)` 对同一 raw 解析两次**（`logInvokeError` + `invokeCmd`）。建议：解析一次复用。

- **[陈旧命名/注释]** `dashboard.*` i18n 命名空间被 SyncPreviewPage 复用（`SyncPreviewPage.svelte:341,380-413` 等），dashboard 模块已并入 sync；`syncDecisions.svelte.ts:3` 注释仍提「Conflicts page」；`src-tauri/src/errors.rs:26-27` 与 `apply.rs:29` 注释引用 "Task 3/6/9/10" 已完成阶段。建议：迁 `sync.*` 命名空间、更新注释、排查 `dashboard.*` 是否有无消费者 key。

- **[`src/modules/onboarding/pages/OnboardingPage.svelte:437-442`] 清理重复 + busy 竞态。** `onMount` 返回 `stopPolling` 又 `onDestroy(stopPolling)`（都销毁时触发，冗余无害）；多步流程里 `busy` 在嵌套调用（`checkVault` 被 `continueWithSelectedRepository` 调用）和轮询间隔内会被内层 finally 提前置 false，存在 UI 短暂可点窗口。建议：二选一清理；`busy` 改计数或顶层独占。OnboardingPage 脚本 ~440 行，device-flow 轮询状态机可抽独立模块。

### P2 -- 后端 Rust

- **[`src-tauri/src/github/repository.rs:200-242`] `check_vault` 的 branches 列举未分页，分支数 >100 时误判 `BranchMissing` 并漏取 `head_sha`。** `check_vault` 走单次 `client.get`+`resp.json()`（`:205,242`），只取 `per_page=100` 首页；而 `list_branches`（`:187-197`）用 `get_paginated_json` 穷尽。目标分支排第 101 个后 -> `branch_exists`（`:250-252`）false -> `BranchMissing`（`:253-254`）。建议：`check_vault` 也用 `get_paginated_json`。

- **[`src-tauri/src/github/repository.rs:139,148-161`] `discover_single_repository` 的 `selection_all` 是死分支，`SelectionAll` 永不产出。** `selection_all` 初始化 false（`:139`），`:150` `let _ = &mut selection_all;` 是占位空操作（注释说「若任一 installation 响应含 selection=all」但从不读 `repository_selection` 字段），`:159-161` `if selection_all` 恒假。前端 `OnboardingPage` 却对 `selection_all -> repository_scope_blocked` 有专门处理，形成后端永不产出、前端在等的契约缺口。功能影响：用户以「all repositories」安装 App 时不被识别为 `SelectionAll`；若其恰好只 1 个可访问仓库，会被当 `SingleRepository` 直接放行，绕过「必须显式单选」（多仓库场景仍因 `total>1` 落 `MultipleRepositories` 被拦，故非安全漏洞）。建议：读 installations 响应 `repository_selection=="all"` 真正实现，或删死变量/死分支并收敛前端。

- **[`src-tauri/src/github/credentials.rs:438-582`] `put_json` / `post_json` / `patch_json` 三方法的 401 刷新-重放块几乎逐字重复（~140 行三份）。** 三者仅 HTTP 动词不同，其余（首次请求、`401 -> force_refresh -> generation 校验 -> 二次请求 -> 二次 401 清凭据`）完全一致。`get`/`retry_get` 已抽离读路径重放，写路径没有。建议：抽按「构造 `RequestBuilder` 闭包」或按动词分派的 `send_json_with_retry` helper。这是有真实语义的收敛，非薄封装。

- **[`src-tauri/src/github/repository.rs:205-211` 及 417-421/440-443 等] 对 `AppError::RateLimited` 的 no-op 重构造/identity map。** `:205-211` RateLimited 臂把错误原样重建返回，等价其后的 catch-all，整段可简化为 `?`；`:440-443` 更是纯 identity map。不改变行为，只增噪音并误导读者以为有特殊 rate-limit 处理。建议：删 RateLimited 特判，直接 `?` 传播。

- **[`src-tauri/src/github/repository.rs:227-234`] `check_vault` 把 branches 端点 404 一律归为 `RepositoryMissing`（注释已承认为简化）。** GitHub 对无权限私有仓库也返回 404（避免泄露存在性），故 404 可能是 missing 或 forbidden。影响有限（都阻止继续），但分类不精确影响前端提示文案。属已知取舍，列此以便决定是否按 design doc 补齐证据分类。

- **[`src-tauri/src/sync_engine/apply.rs:335-339`] `LocalReplaceFailed` 恢复相位已定义却从不写入 journal。** `execute_download` 在 `commit_staged` 失败时返回裸 `Err(AppError::RecoveryPending("local replace failed: {e}"))`，被 `apply_plan` 的 `?` 直接向上传播，**无** `save_recovery_journal`。对比 `trash_move_failed` 路径（`:680-683`）会先存 journal 再返回 `RecoveryRequired`。全仓 `local_replace_failed` 仅出现在 `commands.rs:316` 读侧映射--读侧齐备、写侧缺失。常见情形 `commit_staged` 单次 rename 失败会用 `rollback->target` 复原，apply 干净中止、state 不变；但双重 rename 失败（磁盘满/Windows 占用）下本地内容滞留 `.skill-sync-rollback/<task>/<folder>`、target 消失，且因无 journal 不显示恢复卡片--下次 sync 会把该 skill 视作 `LocalDeleted` 提议从远端删除。无静默销毁（内容仍在 rollback、远端仍有），但状态令人困惑且未追踪。建议：比照 trash 路径 `save_recovery_journal(..., "local_replace_failed", ...)` 后返回 `RecoveryRequired`。

- **[`src-tauri/src/sync_engine/apply.rs:412,827-832,917`] `RemoteChanged` 时本地副作用已落盘而 `sync_state.json` 滞后。** apply 顺序：`working = state.clone()`（`:412`）-> 下载（`commit_staged` 替换本地目录）/ 删本地（`move_to_trash`，`:677-679`）改 working+磁盘 -> 远端提交 `commit_changes`（`:827`）。若提交返回 `RemoteChanged`（`:829-832`）则 `clear_journal` 并返回 `PlanChanged`，但 `*state = working`（`:917`）只在成功路径执行--本地目录/trash 已改而 `sync_state.json` 仍是旧 base。无数据丢失（trash 保留、下载内容可再取），且下次 `build_plan` 经 base-adoption/removal 自愈；但 `PlanChanged` 让用户以为「什么都没做」，实际显式勾选的本地副作用已落盘，需再跑一次 apply 才收敛 state。建议：返回 `PlanChanged` 前对已完成本地副作用持久化 `working`，或在文案明确「部分本地变更已应用，下次同步对齐」。

- **[`src-tauri/src/sync_engine/apply.rs:536,732`] `updated_by` 硬编码字面量 `"device"`，丢失真实设备归属。** `next_manifest.updated_by = "device"`（`:536`）与每个上传 `VaultSkill { updated_by: "device", .. }`（`:732`）都是常量，而 `config.device_id` 就在手边（`apply_plan` 持有 `config: &AppConfig`）。manifest 校验不查该字段，故不破坏同步，但字段失去 provenance/audit 意义。建议：改用 `config.device_id`。

- **[`src-tauri/src/sync_engine/apply.rs:979,1000`] `batch_apply` 重复执行 `prepare_plan`（自身一次 + `apply_plan` 内再一次）。** `batch_apply`（`:970`）先 `prepare_plan`（`:979`）构造 `selected_action_ids`，`drop(prepared)`（`:999`）后调 `apply_plan`（`:1000`），后者在 `:414` 又 `prepare_plan` 一次。使 `upload_skills`/`download_skills` 每次做 2× `fetch_manifest`+scan+pack。两命令已在 `lib.rs:81-82` 注册，但当前前端只经 `apply_sync_plan` 驱动、未调用它们。建议：让 `apply_plan` 接受已 prepared 计划（或抽「按 skill_id 选择 -> 组装 request -> 复用同一 prepared」内部函数）；并确认两命令是否仍需暴露。属效率/表面积，非正确性。

## 合规检查（通过项）

**前端**：`routes -> app -> modules -> shared` 分层正确，无反向/循环/自导入，跨模块消费全经 `index.ts`，无 `export *`；硬编码色值仅在 `index.css` token 定义，组件层全用语义类，无任意 `text-[..]`/`rounded-[..]`，`.ts`/`.svelte` 无中文字面量、UI copy 经 `t('...')`；`theme`/`ui`/`language`/`syncDecisions` 状态类均封装真实副作用（localStorage、`i18next.changeLanguage`、`applyTheme`、`<html lang>`），非薄包装；`invokeCmd` 归一化 `SkillSyncError` + 脱敏日志。

**后端 Rust**：token 全程 `secrecy::SecretString`，`GithubCredential` 不派生 `Serialize`、`Debug` 经测试断言不泄明文，keyring 边界仅 `spawn_blocking` 内 `ExposeSecret`，`errors.rs` 序列化脱敏，device flow 只发 `client_id`；`GithubCredentialManager` `refresh_lock` 单飞刷新 + generation 乐观并发（20 并发单测），`GitHubVaultStore` 不可变 HEAD pinning + `reconcile_update` 三态裁决，读请求 5xx 指数退避重试、写请求不重试；`pack.rs` 两阶段解包（先自解析 raw CD 对抗 `ZipArchive` 去重，再限量 reader 流式校验）、zip-slip/symlink/reparse/大小写折叠 collision 全拒绝、伪造 header 对抗测试齐备（**唯一缺口即 P1 第 1 条**）；`fetch_blob` 校验 sha256、`commit_changes` 网络调用前 `validate_blob_write` 拒绝 `manifest.json`/重复 blob；全后端 `pub(crate)`，测试构造 `#[cfg(test)]` 隔离；`plan.rs` 13 行三方真值表 + golden digest 锁定 fingerprint、删除护栏、namespace root 不健康时降级 `Unknown` 防误删；apply 全程只改 `working` clone，步骤 1-6 校验失败丢弃 clone、原 state 不变，decision 白名单与前端一致，上传按内容寻址 blob 去重，远端提交「先 journal -> 提交 -> 按结果清/留」，`RemoteOutcomeUnknown` 落 journal + `RecoveryRequired`，恢复裁决对不可变 snapshot 为纯函数且 fail-closed；`durable_replace` temp+fsync+原子 rename+父目录 fsync；`VaultManifest::parse_validated` 拒绝重复 skill key 并校验 namespace 前缀/hash/size/folder 折叠碰撞。

## Open Questions

- **`getAppState` 归属**：是否有意让 settings 拥有全部 app-state 查询，还是 `shared` 客户端更符合预期分层？
- **`namespace` 枚举**：`agents/codex/claude-code` 是否已稳定到可提升 `shared/schemas`？
- **`SelectionAll` 检测**：是否真正实现 `repository_selection=="all"` 检测以在单仓库场景也强制显式选择，还是接受「靠 MultipleRepositories 兜底 + 单仓库放行」并删死分支？
- **`panic` 策略**：release 是否会启用 `panic="abort"`？若会，P1 第 1 条的 `parse_central_directory` 越界从「被 spawn_blocking 捕获的下载失败」升级为硬崩溃 DoS，需优先修。
- **`.skill-sync-trash` 回收**：本地删除移入 `<root>/.skill-sync-trash/<task>/` 后无 GC（`cleanup_task_artifacts` 只清 rollback/staging），trash 随「带删除的同步」次数无限增长。是刻意保留作撤销安全网，还是加 TTL/容量回收？（注：2026-07-15 计划曾决定「移除 trash，本地删除直接删除」，但**未执行**，trash 仍在 `local_apply.rs`/`detect.rs`/`apply.rs`。）
- **`dashboard.*` 命名空间**：是否有合并后无消费者的 key？（i18n lint 不查未用 key。）
- **`SyncPreviewPage` 的 `scan.error` 静默**：有意 best-effort 还是遗漏？

## 已规划但未执行的修复（来自 2026-07-15 计划，2026-07-16 核验未落地）

以下修复在 `docs/plans/2026-07-15-resolve-review-open-questions.md` 中已规划，但**均未执行**，故相关发现仍成立：

1. `getAppState` -> `shared/lib`，并新增 ESLint 规则禁止跨 module 引用（含 `scanSkills` 一并提升 shared、skills 模块解散、`namespace` 枚举提升 `shared/schemas`）。
2. `namespace` 枚举统一（并入 1）。
3. `SelectionAll` 真正实现检测（后端读 installations `repository_selection=="all"`）。
4. 移除 trash，本地删除直接 `fs::remove_dir_all`（删 `trash_dir`/`move_to_trash`/`TrashMoveFailed` 相位/`detect.rs` skip 列表引用，改 apply.rs 删本地循环与对应测试）。

## Summary

**整体评价：工程质量高。** lint 纪律严（eslint 自定义规则强制分层与边界、Rust clippy deny unwrap/panic）、校验边界 zod 严密、凭据 `SecretString`+keyring+双层脱敏、单飞刷新与 HEAD-pinned 乐观并发、blob 完整性校验、`pack.rs` P0 级严格的不可信 zip 两阶段校验、`plan.rs`/`apply.rs` 真值表+golden fingerprint+删除护栏+防误删、journal+不可变 snapshot fail-closed 恢复裁决、apply 测试覆盖充分（1446 行）。无 P0。

**主要债务（lint 抓不到处）**：

- P1 三条：`pack.rs` CD 切片越界 panic（潜在 DoS）、workspace-ready 门控四处重复（已漂移）、`apply_plan` 536 行单函数。
- P2 前端：thin re-export、`getAppState` 错位、透传 barrel、`namespace` 重复、冗余 request 端 zod、`defaultNextPlan` 状态泄漏、store 直写、`scan.error` 静默、`SettingsPage` 去抖数据不一致、schema 重复、knip 未用导出、tauri 双重 parse、陈旧命名、OnboardingPage 清理/busy。
- P2 后端：`check_vault` 未分页、`selection_all` 死分支、写路径 401 三重复制、RateLimited no-op、404 简化、`LocalReplaceFailed` 恢复无写侧、`RemoteChanged` 后 state 滞后、`updated_by` 硬编码、`batch_apply` 二次 prepare。
- 其中 3 项（`apply_plan`、`syncApi` 标量 re-parse、`getAppState` 错位）为 2026-07-14 review 复现未修项；4 项已有修复计划但未执行。

**优先处理建议**：P1 `pack.rs` 边界校验、后端 `LocalReplaceFailed` 恢复补 journal、`check_vault` 分页，以及前端两处状态一致性（`SettingsPage` 去抖、`SyncPreviewPage` `defaultNextPlan`）。

**剩余 review 风险（未深读）**：`detect.rs`（扫描/碰撞检测）、`skill.rs`（SKILL.md 解析与 skill_id）、`portable_path.rs`（可移植路径/碰撞 key）、`config.rs`、`vault_binding.rs`、`ignore.rs`、`github/store.rs`、`github/auth.rs` 内部、`plan.rs` 内部、各 onboarding Stage / sync 组件。lint 已保证这些区域规范合规与无 unwrap，但正确性/可维护性的逐行风险未覆盖；`portable_path` 的碰撞折叠与 `detect` 的碰撞产出是 plan/pack 校验正确性的上游依赖，值得后续单独一轮核对。
