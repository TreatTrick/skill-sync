# Onboarding 绑定时建立本地基线 base

## 背景

删除本地 skill 后，系统判为"从云端重新下载"而非"删除云端"。根因：`sync_state.json` 的 `skills` 为空（base 无记录）。base 只在 apply 同步后写入；onboarding（`bind_github_vault`）只写空 base；用户未 apply 则 base 一直空。

真值表中 `base=None, local=None, remote=Some` → `RemoteUpdate`（下载）；要得到 `LocalDeleted`（删云端）需 `base=Some, local=None, remote=Some` 且 `remote.hash == base.base_hash`，即 base 必须有记录。

经确认这不是 fb10c92 引入（fb10c92 对 `derive_entry` 真值表零改动，仅注释）。用户选择方案：**onboarding 绑定成功后自动建立基线**。

## 目标

onboarding 绑定成功后，自动把"本地与云端 hash 一致"的 skill 写入 base，使后续"删除本地 => 删云端"判定在首次绑定后即生效，无需用户手动 apply。

## 核心复用

- `prepare_plan`（apply.rs，private）已经算出 `plan.base_adoptions` —— 正是"local==remote、应采纳为 base"的 skill（base 空时全部命中）。
- `apply.rs:786-817` 已有把 adoptions/removals 写入 `working.skills` 的逻辑。提取为共享函数，`apply_plan` 与新 `establish_baseline` 共用。

## 改动清单

### 后端

1. **`src-tauri/src/sync_engine/apply.rs`** — 提取状态转移函数
   - 把 786-817 的 adoptions/removals 写入循环提取为：
     ```rust
     fn apply_state_transfers(working: &mut SyncState, plan: &SyncPlan) -> Vec<String>
     ```
     返回发生变更的 skill_id 列表（无 IO，纯内存）。
   - `apply_plan` 原位替换为 `state_updated.extend(apply_state_transfers(&mut working, plan));`，行为不变（测试 `result.state_updated` 仍通过）。

2. **`src-tauri/src/sync_engine/apply.rs`** — 新增 `establish_baseline`

   ```rust
   pub(crate) async fn establish_baseline<S: RemoteStore>(
       config: &AppConfig,
       state: &mut SyncState,
       store: &S,
       home: &Path,
       config_dir: &Path,
   ) -> Result<BaselineResult>
   ```
   - `prepare_plan(config, state, store, home)` （只读：fetch + scan + pack + merge）
   - `working = state.clone()`；`apply_state_transfers(&mut working, plan)`
   - 若无变更：直接返回 `BaselineResult { adoptions: 0, removals: 0, commit_sha: prepared.expected_commit }`，不写盘
   - 若有变更：`working.remote.commit_sha = prepared.expected_commit`；`save_to(config_dir)`（durable 原子写，`spawn_blocking`）。回写 `*state = working`。
   - **无需 journal**：纯 state 转移，无远端提交、无本地文件副作用，`save_to` 失败前不持久化，不会产生 pending recovery。

3. **`src-tauri/src/sync_engine/model.rs`** — 新增类型

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
   pub(crate) struct BaselineResult {
       pub adoptions: u32,
       pub removals: u32,
       pub commit_sha: String,
   }
   ```

4. **`src-tauri/src/commands/sync.rs`** — 新增命令

   ```rust
   #[tauri::command]
   pub(crate) async fn establish_baseline(state: State<'_, AppRuntime>) -> Result<BaselineResult>

   pub(crate) async fn establish_baseline_impl(runtime: &AppRuntime) -> Result<BaselineResult>
   ```
   - 写锁 `runtime.gate.inner.write().await`
   - `ensure_no_pending_recovery()`
   - `load_config` + `load_store_and_state` + 取 `home` + `config_dir`
   - 调 `crate::sync_engine::apply::establish_baseline`

5. **`src-tauri/src/lib.rs`** — `generate_handler!` 注册 `commands::sync::establish_baseline`

### 前端

6. **`src/shared/schemas`** — 加 `baselineResultSchema`（`adoptions`/`removals` 非负 int + `commit_sha` string）并导出 type。放现有 sync 相关 schema 处，遵循 `routes -> app -> modules -> shared` 导向。

7. **`src/modules/onboarding/api/onboardingApi.ts`** — 加：

   ```ts
   export const establishBaseline = async (): Promise<BaselineResult> =>
     baselineResultSchema.parse(await invokeCmd<unknown>('establish_baseline'))
   ```

8. **`src/modules/onboarding/pages/OnboardingPage.svelte`** — `confirmBind`（~401-426）里 `bindGithubVault` 成功后、`goto` 前，best-effort 调用：
   ```ts
   try {
     const result = await establishBaseline()
     if (result.adoptions > 0)
       toast.success(
         t('onboarding.baselineEstablished', { count: result.adoptions }),
       )
   } catch (e) {
     console.warn('establish_baseline failed', e)
   }
   ```
   - 失败不阻塞：仍执行 `goto('/app/sync')`。
   - 在已有 `busyDepth++ / finally busyDepth--` 范围内，用户看到 busy。

### i18n

9. **`src/shared/i18n/locales/zh-CN.json` / `en-US.json`** — 加 `onboarding.baselineEstablished`（带 `{count}`）。不设 `baselineFailed`（失败 silent warn，不打扰）。

### 测试

10. **`src-tauri/src/sync_engine/apply.rs` tests** — 复用现有 `temp_home` / mock store helper：
    - base 空 + local==remote → `establish_baseline` 后 `state.skills` 含该 skill，`adoptions == 1`
    - base 空 + local≠remote（conflict）→ 不写入，`adoptions == 0`
    - base 已有且 local==remote hash 同 → 无变更，`adoptions == 0`，不写盘

## 设计决策

- **best-effort**：bind 成功是核心，baseline 失败不阻塞 onboarding；用户进入 sync 页后仍可手动 apply。
- **幂等**：已有 base 时只推进需要更新的（base 过时但 local==remote 的 adoption），不破坏既有非一致状态。same_binding rebind 时调用也安全，但本次只在 `confirmBind` 调用。
- **不破坏 preview 只读**：`get_sync_plan` 保持只读，基线写入只在 onboarding 的写操作里发生。
- **不触发 recovery**：纯 state 写 + 原子 `save_to`，失败前不持久化。

## 验证

- `cargo test -p skill-sync`（重点 apply/plan 模块）
- `npm run rust:clippy`
- `npm run lint`（含 lint:i18n / lint:responsive / lint:colors）
- `npm run build`
- 手动：onboarding 绑定后检查 `sync_state.json` 的 `skills` 已填充；删除某本地 skill，sync 预览应显示"删除云端"而非"下载"。
