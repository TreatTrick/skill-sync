# 代码架构审查（合并版）

> 本文档合并三部分审查：
>
> 1. 既有 `docs/plans/2026-07-14-code-review.md`——已提交架构的结构/正确性/边界；
> 2. `/review-code-architecture` 对 `src/` 前端 TS/Svelte 的独立复核；
> 3. `/review-code-architecture` 对 `src-tauri` 后端 Rust 的深读——含既有审查未逐行验证的四个大文件 `github/store.rs`、`pack.rs`、`github/repository.rs`、`github/credentials.rs`，`remote_store` / `errors` / `github/auth` 等契约文件，以及**同步引擎核心** `sync_engine/{model,plan,apply}.rs`、`sync_state.rs`、`vault_manifest.rs`、`local_apply.rs`、`commands/sync.rs`（三方合并真值表、fingerprint、delete guard、decision 白名单、apply 事务顺序、journal 与 `resume_sync_recovery` 恢复重放）。
>    重叠发现已去重。工作树干净，最近提交多为文档/规划，唯一相关代码变更是 `feat: add github support link`。审查基线为 `HEAD c3aefc1`。

---

## Findings

### P0

无。未发现错误行为、数据损坏、安全暴露、构建失败或常见路径运行时崩溃。凭据（`SecretString` + keyring）、token 脱敏、Tauri 边界响应端 zod 校验、blob hash 校验、乐观并发均稳健。

### P1

- **[src-tauri/src/pack.rs:490-519] `parse_central_directory` 对不可信 zip 的 central directory 做无边界检查的切片，畸形/恶意输入会 panic。** 循环守卫只保证固定 46 字节头在界内（`pos + 46 <= bytes.len()`，第 490 行），但 `name_len` / `extra_len` / `comment_len` 取自头部字段（各可达 65535），随后第 502-506 行直接切片 `bytes[name_start..name_start + name_len]`、`bytes[.. + extra_len]`、`bytes[.. + comment_len]`，从不校验 `name_start + name_len + extra_len + comment_len <= bytes.len()`。构造一个 CD 头声明超长 name/extra/comment 的畸形 zip 即触发 slice 越界 panic。可达性：`unpack_skill`（pack.rs:361）在 `execute_download` 中解包 `fetch_blob` 返回的 blob；`fetch_blob`（store.rs:55-75）只校验 `sha256(bytes) == manifest 声明的 hash`，因此一个恶意/被攻陷的远端 vault 可以放入一个「hash 匹配但 CD 畸形」的 blob，使 bytes 通过 hash 校验后进入该 parser。**当前缓解**：`unpack_skill` 运行在 `spawn_blocking` 内，其 `JoinError` 在 apply.rs:290-294 被映射为 `AppError::Vault`；且本仓库未设置 `[profile.*] panic`，默认 `panic="unwind"`，故 panic 目前被 spawn_blocking 边界捕获、降级为一次下载失败，不会崩溃进程。**风险**：(1) 这依赖「unwind + 始终在 spawn_blocking 内」两个隐式前提——`parse_central_directory` 是 `pub(crate)`，未来任何直接调用点都会真的 panic；(2) 一旦为减小体积/去除 unwind 表而设置 `panic="abort"`（release 常见优化），同一路径立即变为硬崩溃 DoS（升级为 P0）。安全边界解析器本应对越界返回 `Result` 而非依赖 unwind 兜底。建议修改：在读取 name/extra/comment 前显式校验 `name_start + name_len + extra_len + comment_len <= bytes.len()`，越界返回 `AppError::Vault`；`le_u16`/`le_u32` 已是此模式，切片处补齐即可。

### P2 —— 前端（TS/Svelte）

- **[src/app/router/routeConfig.ts:6] 冗余 pass-through re-export，且同一谓词有两个导入来源。** `routeConfig` 从 `@/shared/lib` 导入 `isWorkspaceReady` 后原样 `export { isWorkspaceReady }`，不含任何包装或路由语义。结果 `routes/+page.svelte:5`、`routes/app/+page.svelte:5`、`routes/app/+layout.svelte:7` 从 `@/app/router/routeConfig` 导入它，而 `modules/onboarding/pages/OnboardingPage.svelte:8` 从 `@/shared/lib` 导入同一函数——同一领域谓词出现两个导入来源。`routes` 位于分层最上层，可直接依赖 `shared`，该 re-export 是无价值的间接层。建议修改：删除 `routeConfig.ts:6` 的 re-export，让 route 文件直接从 `@/shared/lib` 导入；`routeConfig` 只保留真正的路由配置（`appRoutes` / `AppRouteConfig`）。

- **[src/modules/settings/schemas/config.ts:1-3, src/modules/onboarding/schemas/onboarding.ts:1-25] 纯透传 schema barrel（空抽象）。** 两个文件只是把 `@/shared/schemas` 的 schema/type 原样再导出，没有任何模块本地 schema 定义。`configApi.ts`、`onboardingApi.ts` 经由 `../schemas/...` 导入，凭空多了一层间接。按 anti-thin-wrapper 与“不加空抽象”规则，这属于空抽象。建议修改：让这两个 API 文件直接从 `@/shared/schemas` 导入；仅当确有模块本地 schema 时才保留该文件（此时补一条注释说明它是刻意的 facade）。

- **[src/modules/sync/api/syncApi.ts:29-37, src/modules/settings/api/configApi.ts:22-33, src/modules/onboarding/api/onboardingApi.ts:85-102 及 113/124/133/144] 对已由 zod 保证的内部类型做重复 request 端校验，且策略不一致。** `resumeSyncRecovery(taskId: string)` 对已是 `string` 的 `taskId` 执行 `z.string().min(1).parse`（syncApi.ts:32）；`disconnectGithub` / `listInstallationRepositories` 对已 typed 为 `number` 的参数执行 `z.number().int().nonnegative().parse`（configApi.ts:25-29 / onboardingApi.ts:88-92）；`listGithubRepositoryBranches` / `checkGithubVault` / `initializeGithubVault` / `bindGithubVault` 对前端用已 typed 数据构造的 request 再 `parse`（onboardingApi.ts:113/124/133/144）。这些标量/对象都来自上游已 zod 校验的 `appState`（`appStateSchema` 已强制 `int().nonnegative()`）、`discovery` 结果或 `recovery.task_id`（`recoveryInfoSchema`）；Rust 侧还会自行 serde 反序列化并校验，故 request 端 `parse` 属于重复防御，且用得不一致——`pollGithubDeviceFlow` 就没有对 `deviceCode` / `interval` 做同样处理。**响应端**校验（`syncPlanSchema.parse` / `applySyncResponseSchema.parse` / 各 `*Schema.parse(await invokeCmd(...))`）完全正确，应保留。另注：`onboardingApi.ts:44-59` 的 `normalizeGithubRepository` 处理 GitHub 多形态 payload（`owner` 可为 string 或 `{login}`、`repository_id`/`id`、`repo`/`name`），是真实边界规范化，不在此列，应保留。建议修改：删除对内部确定类型的 request 端重复 `parse`，直接透传；若确为 FFI 清晰报错刻意保留，请统一策略并注明这是纵深防御。

- **[src/modules/settings/index.ts:2] settings 模块对外导出横切的 `getAppState`，被四个非 settings 调用方消费。** `getAppState` 是全局 app-state 查询（config + 凭据状态 + recovery），经 `@/modules/settings` 被 `OnboardingPage.svelte:7`、`SyncPreviewPage.svelte:22`、`routes/+page.svelte:6`、`routes/app/+page.svelte:6`、`routes/app/+layout.svelte:8` 使用。这迫使概念上位于 settings “之前/之旁”的 onboarding、sync 与 routes 为拿一个通用状态读取而依赖 settings 模块，settings 变成 hub。它经 `index.ts` 导出，不算边界违规，是布局选择。建议修改：考虑把 `getAppState`（及同类 app-state 读取）提升到 `shared` 客户端（`shared` 已含 `appStateSchema`），让 settings 只保留自身能力。见下方 Open Questions。

- **[src/modules/sync/schemas/syncPlan.ts:5, src/modules/skills/schemas/skill.ts:3] `namespace` 枚举在两模块各自重复定义。** 两处都写 `z.enum(['agents', 'codex', 'claude-code'])`。跨模块复制小枚举可避免耦合，但后端新增命名空间时需同时改两处，存在漂移风险。建议修改：若该枚举确为稳定共享领域概念，可提升到 `shared/schemas` 统一维护；否则保留，但需知悉漂移风险。属判断项，非硬性违规。

- **[src/modules/sync/pages/SyncPreviewPage.svelte:167-170,286] `defaultNextPlan` 在“无变化 recheck”后残留为 `true` 的潜在状态泄漏。** `handleRecheck` 置 `defaultNextPlan = true`，但只有 `fingerprint !== lastFingerprint` 的 `$effect` 分支会把它复位为 `false`。若 recheck 得到相同 fingerprint（计划未变），该标志残留 `true`，直到下一次 fingerprint 变化才被消费——届时会对一次**非用户发起**的 plan 变化自动套用默认选择，覆盖用户当前的手动勾选。当前 `QueryProvider` 设了 `refetchOnWindowFocus: false`，且唯一的非 recheck 失效路径（apply 成功后 `invalidateQueries(['sync-plan'])`）之前已先 `clearInteractionState`，故实际触发面很窄、影响低；但这是脆弱的状态卫生问题，未来新增任何 `sync-plan` 失效点都会让它显形。建议修改：recheck 完成后无条件复位 `defaultNextPlan`（或在 effect 的两个分支都复位），使“是否套用默认”严格绑定单次 recheck。

- **[src/modules/settings/pages/SettingsPage.svelte:79-99] 自动保存去抖：400ms 内“改后又改回”会持久化中间值，导致 UI 与后端不一致。** `$effect` 在 `snapshot === lastSaved`（第 83 行）时**先于** `clearTimeout`（第 88 行）提前 `return`，因此上一次变更排定的定时器仍会触发，把闭包捕获的中间值 `current` 写入后端并置 `lastSaved = 中间快照`。由于 `config` 只 prefill 一次（`prefilled` 守卫），随后 `invalidateQueries` 不会覆盖本地状态，于是 UI 显示回退后的值，而后端存的是中间值，直到用户下次编辑才纠正。触发条件为 400ms 内改动并改回，概率低，但确为真实的数据不一致。建议修改：把 `clearTimeout(saveTimer)` 移到 `snapshot === lastSaved` 提前 `return` 之前；或让定时器回调内以最新 `effectiveConfig` 为准重新读取，避免持久化过期闭包值。

### P2 —— 后端 Rust

- **[src-tauri/src/github/repository.rs:200-242] `check_vault` 的 branches 列举未分页，仓库分支数 >100 时会误判 `BranchMissing` 并漏取 `head_sha`。** `list_branches`（repository.rs:187-197）用分页的 `get_paginated_json` 穷尽所有分支，但 `check_vault` 走单次 `self.client.get(...)` + `resp.json()`（第 205、242 行），只取 `per_page=100` 的第一页。若目标 vault 分支排在第 101 个之后，`branch_exists`（第 250-252 行）为 false → 返回 `BranchMissing`（第 253-254 行）；即使命中，`head_sha`（第 256-260 行）也只在首页里找。对专用 vault 仓库通常分支很少，但若用户把一个已有大量分支的仓库当 vault，就会出现假阴性。建议修改：`check_vault` 也用 `get_paginated_json` 穷尽 branches，与 `list_branches` 保持一致。

- **[src-tauri/src/github/repository.rs:137-161] `discover_single_repository` 的 `selection_all` 分支是死代码，`SelectionAll` 实际永不产生。** `selection_all` 初始化为 `false`（第 139 行），第 148-151 行仅有 `let _ = &mut selection_all;` 的占位空操作（注释「简化：若任一 installation 响应含 selection=all」但从未真正读取 `repository_selection` 字段），因此第 159-161 行 `if selection_all { return SelectionAll }` 恒不成立。前端 `OnboardingPage` 却对 `discovery.status === 'selection_all'` 有专门处理（`repository_scope_blocked`），形成后端永不产出、前端却在等的契约缺口。功能影响：用户以「all repositories」安装 App 时不会被识别为 `SelectionAll`；若其恰好只有 1 个可访问仓库，会被当作 `SingleRepository` 直接放行，绕过「必须显式单选仓库」的策略（多仓库场景仍会因 `total>1` 落到 `MultipleRepositories` 而被拦，故非安全漏洞）。建议修改：要么读取 installations 首页响应里的 `repository_selection == "all"` 真正实现该检测，要么删除死变量与死分支并同步收敛前端对该状态的处理。

- **[src-tauri/src/github/credentials.rs:438-582] `put_json` / `post_json` / `patch_json` 三个方法的 401 刷新—重放块几乎逐字重复。** 三者仅 HTTP 动词不同，其余（首次请求、`401 → force_refresh → generation 校验 → 二次请求 → 二次 401 清凭据`）完全一致，约 140 行三重复制。`get`/`retry_get` 已把读路径的重放抽离；写路径没有。维护风险：任何一处 401 语义调整需同步改三份。建议修改：抽取一个按「构造 `RequestBuilder` 的闭包」或按动词分派的 `send_json_with_retry` helper，三方法各自只提供 method 与 body。这是有真实语义的收敛（统一 401 重放不变量），不属于薄封装。

- **[src-tauri/src/github/repository.rs:205-211,267-273,417-421,440-443] 多处对 `AppError::RateLimited` 的 no-op 重构造/identity map。** 例如第 205-211 行 `match ... { Ok(r)=>r, Err(RateLimited{retry_after})=>return Err(RateLimited{retry_after}), Err(e)=>return Err(e) }`——RateLimited 臂把错误原样重建，与其后的 catch-all `Err(e)=>Err(e)` 等价，整段可简化为 `?`；第 440-443 行 `.map_err(|e| match e { RateLimited{retry_after}=>RateLimited{retry_after}, other=>other })` 更是纯 identity map。这些不改变行为，只增加噪音并让读者误以为此处有特殊 rate-limit 处理。建议修改：删除 RateLimited 特判，直接用 `?` 传播；确需在此附加语义时再写实际逻辑。

- **[src-tauri/src/github/repository.rs:227-234] `check_vault` 把 branches 端点的 404 一律归为 `RepositoryMissing`，注释已承认是简化。** GitHub 对「无权限的私有仓库」也返回 404（避免泄露存在性），故 404 可能是 missing 或 forbidden。代码注释「简化：repo 404 -> RepositoryMissing（完整证据分类见 design doc）」已标注取舍。影响有限（都会阻止继续），但分类不精确会影响前端给用户的提示文案。属已知取舍，列此以便决定是否按 design doc 补齐证据分类。见 Open Questions。

- **[src-tauri/src/sync_engine/apply.rs:255-325,591-611] `LocalReplaceFailed` 恢复相位已定义却从不写入 journal，下载 `commit_staged` 失败以裸 `Err(RecoveryPending)` 传播、不落 journal、不呈现 `pending_recovery`。** `execute_download` 在 `commit_staged` 失败时返回 `Err(AppError::RecoveryPending("local replace failed"))`（apply.rs:318-321），被 apply_plan 的 `?`（apply.rs:597）直接向上传播——**没有** `save_recovery_journal`。对比 `trash_move_failed` 路径（apply.rs:620-639）会先存 journal 再返回 `Ok(RecoveryRequired)`。全仓库搜索确认 `phase="local_replace_failed"` 只在模型定义（model.rs:163）、`journal_to_recovery` 的读侧映射（commands.rs:316）和文档注释（local_apply.rs:180）出现，**无任何写入点**——即恢复的读侧齐备但写侧缺失。常见情形（`stage→target` 单次 rename 失败）下 `commit_staged` 会用 `rollback→target` 把 target 复原（local_apply.rs:53-55），apply 干净中止、state 不变，安全；但在窄口的双重 rename 失败（`stage→target` 与随后的 `rollback→target` 都失败，如中途磁盘满/Windows 目标被占用）下，本地 skill 内容会滞留在 `.skill-sync-rollback/<task>/<folder>`、target 消失，且因无 journal 而不显示恢复卡片——下次 sync 会把该 skill 视作「本地已删」（`LocalDeleted`）并提议从远端删除。无静默销毁（内容仍在 rollback 目录、远端仍有），但状态令人困惑且未被追踪。建议修改：在 `commit_staged` 失败处比照 trash 路径 `save_recovery_journal(..., "local_replace_failed", ...)` 后返回 `Ok(RecoveryRequired{ phase: LocalReplaceFailed, ... })`，让滞留状态被 journal 追踪并在 UI 呈现。

- **[src-tauri/src/sync_engine/apply.rs:596-644,732-816] 下载/删本地是发生在远端提交之前的持久化副作用；`RemoteChanged` 时返回 `PlanChanged` 且清 journal、不保存 `working`，本地目录/trash 已改而 `sync_state.json` 滞后。** apply 顺序为：先执行下载（`commit_staged` 替换本地目录，apply.rs:596-611）与删本地（移入 trash，apply.rs:614-644），再发起远端提交（apply.rs:768）。若提交返回 `RemoteChanged`（apply.rs:770-776），代码 `clear_journal` 并返回 `PlanChanged`，但此前 `*state = working` 从未执行、state save 也未触及——于是本地目录/trash 已被改动，而 `sync_state.json` 仍是旧 base。无数据丢失（trash 保留被删内容、下载内容来自远端可再取），且下次 `build_plan` 会通过 base-adoption/removal（0-commit）自愈：下载项 `local==remote 且 base!=local` → adoption，删本地项 → BothDeleted removal。风险点：`PlanChanged` 响应让用户以为「计划变了、什么都没做」，实际用户显式勾选的本地副作用已落盘，且需再跑一次 apply 才能把 state 收敛。建议修改：至少在返回 `PlanChanged` 前对已完成的本地副作用持久化 `working`（或在响应/文案中明确「部分本地变更已应用，将于下次同步对齐」），避免 state 与磁盘长期不一致。

- **[src-tauri/src/sync_engine/apply.rs:475,673] 远端 manifest 与上传 skill 的 `updated_by` 硬编码为字面量 `"device"`，丢失真实设备归属。** `next_manifest.updated_by = "device"`（apply.rs:475）与每个上传 `VaultSkill { updated_by: "device", .. }`（apply.rs:673）都是常量，而 `config.device_id` 就在手边（`apply_plan` 持有 `config: &AppConfig`，`repository.rs::initialize_vault` 也确实用 `self.device_id`）。`updated_by` 字段本用于记录「哪台设备做的变更」（provenance/audit），硬编码后所有设备都写 `"device"`。manifest 校验不检查该字段，故不破坏同步，但字段失去意义。建议修改：改用 `config.device_id`（与 `commands/sync.rs:57` 的 `device_name` 及 initialize 路径一致）。

- **[src-tauri/src/sync_engine/apply.rs:911-942] `batch_apply` 重复执行 `prepare_plan`（自身一次 + `apply_plan` 内再一次），使 `upload_skills`/`download_skills` 每次做 2× `fetch_manifest`+scan+pack。** `batch_apply` 先 `prepare_plan`（apply.rs:920）以构造 `selected_action_ids`，`drop(prepared)` 后调用 `apply_plan`，后者在 apply.rs:397 又 `prepare_plan` 一次。这两个批量命令已在 `lib.rs:81-82` 注册为 Tauri 命令，但当前 Svelte 前端只经 `apply_sync_plan` 驱动、未调用它们。建议修改：让 `apply_plan` 接受一个已 prepared 的计划（或抽出「按 skill_id 选择 → 组装 request → 复用同一 prepared」的内部函数），避免二次 fetch/scan/pack；并确认这两个命令是否仍需暴露。属效率/表面积问题，非正确性缺陷。

- **[src-tauri/src/sync_engine/apply.rs:1-2157, plan.rs:1-1536] `apply_plan`/`merge_plan` 体量较大，校验块与工作项收集循环可提取以便独立单测（沿用并细化既有审查结论）。** `apply_plan` 现约 490 行（apply.rs:376-867），线性带序、阶段注释清晰，不是上帝函数，但请求校验（步骤 1-6，apply.rs:400-468）与四类工作项收集循环（apply.rs:482-580）可抽成 `validate_apply_request` / `collect_work_items` 等纯函数，让校验分支无需搭建 store/journal 即可单测。`merge_plan`（plan.rs:271-559）同理可把「identity/collision/size/root-health 阻断」若干遍历合并表达。建议修改：提取校验与收集逻辑，主函数保持线性编排。属可维护性，非正确性缺陷。

---

## 合规检查（通过项）

**前端（本次复核）**

- **分层与边界**：`routes -> app -> modules -> shared` 方向正确；无反向层导入、无循环/自导入；跨模块消费全部经模块 `index.ts`；无 `export *`。
- **颜色 token / 尺寸 / i18n**：硬编码色值仅在 `src/index.css` token 定义中；组件层全用语义类；无任意 `text-[..]`/`rounded-[..]`；`.ts`/`.svelte` 无中文字面量，UI copy 均经 `t('...')`。
- **状态与瘦包装器**：`theme`/`ui`/`language`/`syncDecisions` 状态类均封装真实副作用（localStorage、`i18next.changeLanguage`、`applyTheme`、`<html lang>`），非瘦包装；`shared/lib` 与 `sync/lib/syncStatus` 助手均含真实语义。
- **错误处理**：`invokeCmd` 归一化为 `SkillSyncError` + `redactLogMessage` 脱敏，调用点统一 `errorMessage` + `toast`。

**后端 Rust（本次深读）**

- **凭据与密钥安全**：token 全程 `secrecy::SecretString`，`GithubCredential` 不派生 `Serialize`、`Debug` 经测试断言不泄明文（credentials.rs:823-832）；keyring JSON 边界用私有 `StoredGithubCredential`，仅在 `spawn_blocking` 内 `ExposeSecret`；`errors.rs` 序列化时 `redact_sensitive` 脱敏 token/secret；device flow 只发 `client_id`、不发 scope/client secret。
- **并发与一致性**：`GithubCredentialManager` 以 `refresh_lock` 实现单飞刷新 + generation 乐观并发（有 20 并发单测）；`GitHubVaultStore` 以不可变 HEAD pinning + `reconcile_update` 三态裁决（candidate/base/other/unreadable）处理 PATCH 结果不明；读请求瞬时 5xx 指数退避重试、写请求不重试（避免重复落库），401 刷新—重放各一次。
- **不可信数据边界（pack.rs）**：这是纵深防御做得好的范例——两阶段解包（先自解析 raw central directory 以对抗 `ZipArchive` 静默去重，再用限量 reader 流式校验实际字节、不信任 header）、zip-slip / symlink / reparse / 大小写折叠 collision 全部拒绝、RAII 清理、伪造 header 的对抗性测试齐备。**唯一缺口即上文 P1 的 CD 切片越界**；除此之外该文件的外部输入校验是 P0 级别应有的严格程度。
- **blob 完整性**：`fetch_blob` 取回后校验 `sha256(bytes)==expected_hash`；`commit_changes` 在任何网络调用前 `validate_blob_write` 并拒绝 `manifest.json`/重复 blob 路径（store.rs:77-87，含 0 请求断言测试）。
- **最小权限**：全后端一致使用 `pub(crate)`，测试专用构造/类型以 `#[cfg(test)]` 隔离（`InMemoryCredentialStore`、`new_with_client` 等）。
- **计划构建与删除安全（plan.rs）**：13 行三方真值表（`derive_entry`）+ golden JSON/digest 锁定的稳定 fingerprint（对展示 warnings/顺序不敏感、optional 显式 null）；删除护栏 `delete_count > max_auto_delete || delete_count*2 > tracked`；**namespace root 不健康时把删除推断降级为 `Unknown`、不推断删除**（plan.rs:398-422，防止扫描不完整导致误删），并按 namespace 各自判定；identity/scan-collision/merged-target-collision/远端 size 超限均在 fetch blob 前 `Blocked`。
- **apply 事务与恢复（apply.rs / local_apply.rs / commands/sync.rs）**：apply 全程只改 `working` clone，步骤 1-6 校验（commit/fingerprint 重核、selected 去重/未知/不可执行拒绝、decision 白名单、delete guard）失败即丢弃 clone、原 state 不变；decision 白名单（`allowed_decisions`）与前端 `conflictDecisionOptions` 完全一致；上传按内容寻址 blob path 去重（apply.rs:650）；远端提交采用「先持久化 intended 状态 journal → 提交 → 按结果清/留 journal」，`RemoteOutcomeUnknown` 落 `remote_outcome_unknown` journal 并 `RecoveryRequired`；恢复裁决 `reconcile_remote_journal` 为对不可变 snapshot 的纯函数（HEAD==base→Unpublished、HEAD==candidate 或 manifest-hash 匹配→Published、否则→Conflict、无 snapshot→Unavailable），Conflict/Unavailable **fail-closed** 保留 journal，均有单测；`durable_replace` 同目录 temp+fsync+原子 rename+父目录 fsync。
- **manifest 与 state 完整性（vault_manifest.rs / sync_state.rs）**：`VaultManifest::parse_validated` 为唯一解析入口，自定义 `deserialize_skills_map` 显式拒绝重复 skill key，校验 schema/key==id/namespace 前缀/hash 格式/blob 推导/size>0/folder NFC-lowercase 折叠碰撞；`SyncState::load_and_validate` 对 installation/repository/branch 不一致只读 `Blocked`、不移动或保存文件。

---

## Open Questions

- **`getAppState` 归属（P2 前端）**：是否有意让 settings 拥有全部 app-state 查询，还是 `shared` 客户端更符合预期分层？
- **`namespace` 枚举（P2 前端）**：`agents/codex/claude-code` 是否已稳定到可提升 `shared/schemas`？
- **`SelectionAll` 检测（P2 后端）**：是否要真正实现「repository_selection == all」检测（读 installations 首页响应）以在单仓库场景也强制显式选择，还是接受当前「靠 MultipleRepositories 兜底 + 单仓库放行」的行为并删除死分支？
- **`panic` 策略**：release 是否会启用 `panic="abort"`？若会，P1 的 `parse_central_directory` 越界会从「被 spawn_blocking 捕获的下载失败」升级为硬崩溃 DoS，需优先修复。
- **`.skill-sync-trash` 回收**：本地删除移入 `<root>/.skill-sync-trash/<task>/` 后无任何 GC（`cleanup_task_artifacts` 只清 rollback/staging，且仅在成功 apply 时按本次 task_id 清理），trash 随「带删除的同步」次数无限增长。这与仍在规划中的删除恢复设计（`docs/plans/2026-07-14-delete-recovery-design.md`）相关：是刻意保留作撤销安全网，还是需要加 TTL/容量上限的回收？

---

## Summary

三部分合并后，整体架构稳健：前端干净分层 + token 一致 + 响应端 zod 校验；后端凭据/密钥安全（`SecretString` + keyring + 序列化脱敏）、单飞刷新与 HEAD-pinned 乐观并发、blob 完整性校验、`pack.rs` 中 P0 级严格的不可信 zip 两阶段校验，以及 `plan.rs`/`apply.rs` 的 13 行真值表 + golden fingerprint、删除护栏与 namespace-root-health 防误删、journal + 不可变 snapshot 的 fail-closed 恢复裁决。

**发现一处 P1**：`pack.rs::parse_central_directory` 对畸形/恶意 central directory 的 name/extra/comment 切片缺少边界检查，可 panic；当前经 `spawn_blocking`（默认 `panic="unwind"`）降级为一次下载失败，但依赖隐式前提，且在 `panic="abort"` 下会升级为硬崩溃 DoS——建议按 `le_u16`/`le_u32` 同样的模式补边界校验返回 `AppError::Vault`。其余为 P2：前端（冗余 re-export、透传 barrel、重复 request 端 zod、`getAppState` 布局、`namespace` 重复、`SettingsPage` 去抖、`SyncPreviewPage` 状态卫生）与后端（`check_vault` 分支未分页、`selection_all` 死分支、写路径 401 重放三重复制、no-op RateLimited 重构造、404 分类简化；本轮 `apply.rs`/`plan.rs` 深读新增：`LocalReplaceFailed` 恢复只有读侧无写侧、`RemoteChanged` 后本地副作用已落盘而 state 滞后、`updated_by` 硬编码 `"device"`、`batch_apply` 二次 prepare、`apply_plan`/`merge_plan` 可提取校验/收集）。无 P0。建议优先处理：P1 边界校验、后端 `LocalReplaceFailed` 恢复补写 journal、`check_vault` 分页，以及前端两处状态一致性（`SettingsPage` 去抖、`SyncPreviewPage` `defaultNextPlan`）。

**已执行**：前端相关文件静态阅读 + `grep` 验证（导入方向/深导入/`export *`、颜色 token、任意尺寸、中文字面量）；后端逐文件深读 `remote_store.rs`、`errors.rs`、`github/{auth,app_config,credentials,store,repository}.rs`、`pack.rs`，并核对 `Cargo.toml` 的 panic 策略以定级 P1；本轮补齐 `sync_engine/{model,plan,apply}.rs`、`sync_state.rs`、`vault_manifest.rs`、`local_apply.rs`、`commands/sync.rs` 的深读（三方合并真值表、fingerprint、delete guard、decision 白名单、apply 事务顺序、journal 写入点、`resume_sync_recovery` 恢复重放裁决），并用 `grep` 确认 `local_replace_failed` 无写入点、`upload_skills`/`download_skills` 的注册与调用面、`updated_by` 字面量。**未执行**：`npm run typecheck`/`lint`/`build` 与 `cargo build`/`clippy`/`test`（纯审查、未改动代码）。**剩余审查风险**：`detect.rs`（扫描/碰撞检测）、`skill.rs`（SKILL.md 解析与 skill_id）、`portable_path.rs`（可移植路径/碰撞 key）、`config.rs`、`vault_binding.rs`、`ignore.rs` 等支撑模块本轮未深读，其内部规则（尤其 `portable_path` 的碰撞折叠与 `detect` 的碰撞产出）是 plan/pack 校验正确性的上游依赖，值得后续单独一轮核对。
