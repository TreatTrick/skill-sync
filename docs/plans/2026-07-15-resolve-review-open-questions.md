# 解决 code-review 的四个 Open Questions

基线 `HEAD c3aefc1`。本计划只处理用户点名的四个 open question，不触碰 review 中其余 P2 项。

---

## 1. `getAppState` 提升 shared + 新增「禁止跨 module 引用」ESLint 规则

### 1a. `getAppState` -> `shared/lib`

`getAppState` 是通用 app-state 查询（config + 凭据状态 + recovery），被 3 个 module + 3 个 route 消费，不应属于 settings。

- 新建 `src/shared/lib/appState.ts`：把 `getAppState` 从 `modules/settings/api/configApi.ts` 整体迁出（依赖 `appStateSchema` from `@/shared/schemas` + `invokeCmd` from `@/shared/lib`）。
- `src/shared/lib/index.ts` 增加 `export { getAppState } from './appState'`。
- `modules/settings/api/configApi.ts` 删除 `getAppState`（保留 `saveConfig` / `disconnectGithub`，二者是 settings 专有）。
- `modules/settings/index.ts` 删除 `export { getAppState }`，仅保留 `SettingsPage`。
- 更新所有消费方改从 `@/shared/lib` 导入：
  - `modules/sync/pages/SyncPreviewPage.svelte:22`
  - `modules/onboarding/pages/OnboardingPage.svelte:7`
  - `modules/settings/pages/SettingsPage.svelte:25`（拆分：`disconnectGithub, saveConfig` 仍来自 `../api/configApi`，`getAppState` 来自 `@/shared/lib`）
  - `routes/+page.svelte:6`、`routes/app/+page.svelte:6`、`routes/app/+layout.svelte:8`

### 1b. `scanSkills` 一并提升 shared（严格规则的必要后果）

用户要的「modules 之间禁止互相引用」是严格规则。现状 `scanSkills`（skills 模块）被 sync + settings 消费，是仅有的另一处跨 module 依赖。skills 模块只有 `api/scanSkills.ts` + `schemas/skill.ts` + `index.ts`，**无 page/component/state**，本质是共享数据访问层，不是产品 module。要满足严格规则，必须把 `scanSkills` 也提升到 shared，skills 模块随之解散。这与 AGENTS.md「stable cross-module foundations such as request clients 属于 shared」一致。

- 新建 `src/shared/lib/skills.ts`：迁入 `scanSkills`（依赖 `scanResultSchema` from `@/shared/schemas` + `invokeCmd`）。
- `src/shared/lib/index.ts` 增加 `export { scanSkills } from './skills'`。
- 新建 `src/shared/schemas/skill.ts`：迁入 `namespaceSchema`、`skillSchema`、`scanRootStatusSchema`、`scanCollisionSchema`、`scanResultSchema`、`ScanResult`（见 1c，`namespaceSchema` 在此定义为唯一源）。
- `src/shared/schemas/index.ts` 增加 `scanResultSchema` / `ScanResult` / `namespaceSchema` 导出。
- 更新消费方改从 shared 导入：
  - `modules/sync/pages/SyncPreviewPage.svelte:23`
  - `modules/settings/pages/SettingsPage.svelte:23`
  - `modules/settings/components/SkillRootsSection.svelte:5`（`import type { scanSkills }`）
- 删除 `src/modules/skills/` 整个目录。

### 1c. `namespace` 枚举提升 `shared/schemas`（open question 2，并入此步）

- `namespaceSchema = z.enum(['agents','codex','claude-code'])` 唯一定义在 `src/shared/schemas/skill.ts`，经 `shared/schemas/index.ts` 导出。
- `modules/sync/schemas/syncPlan.ts:5` 删除本地 `namespaceSchema`，改 `import { namespaceSchema } from '@/shared/schemas'`。

### 1d. ESLint 规则：禁止跨 module 引用

`eslint.config.js` 现有 `no-restricted-imports` 模式 `^@/modules/[^/]+/.+` 只禁深层跨 module，仍允许经 module 根入口跨 module。新增自定义规则补上「同层 module 之间不得互相引用」：

- 新增 `noCrossModuleImportsRule`（仿 `noReverseLayerImportsRule`）：
  - 计算 importer 所在 module 名（`src/modules/<X>` 的 `<X>`）。
  - 对每个 import source（`@/modules/<Y>` / `src/modules/<Y>` / 相对路径解析后落在 `modules/<Y>`），若 `Y != X` 则报错。
  - 报错信息引导「把共享能力提升到 @/shared」。
- 在 `LOCAL_PLUGIN` 注册 `local/no-cross-module-imports`，加入 `IMPORT_RULES`。
- 保留现有 `^@/modules/[^/]+/.+` 模式（仍禁止 app/routes 等任意层对 module 的**深层**导入；module 间根入口导入由新规则接管）。更新该模式 message 避免与新规则矛盾。
- `AGENTS.md`「Module Boundaries」第 1 条由「不得 deep-import 另一 module」收紧为「不得引用另一 module（含根入口），需提升到 shared」。

完成后全树无 `modules/<X>` -> `modules/<Y>` 导入，新规则通过。

---

## 2. `namespace` 枚举统一

已在 1c 完成（唯一源 `shared/schemas`，`syncPlan.ts` 与 `skill.ts` 共用）。

---

## 3. `SelectionAll` 检测：真正实现并强制 selected（后端）

`repository.rs::discover_single_repository` 现有 `selection_all` 是死变量（`let _ = &mut selection_all` 从不读 `repository_selection`），`SelectionAll` 永不产出，前端 `selection_all -> repository_scope_blocked` 契约悬空。

- 在 installations 循环内，对每个 `inst` 读 `inst.get("repository_selection")`；为 `"all"` 时立即 `return Ok(SelectionAll)`。`repository_selection` 字段在 `/user/installations` 的 installation 对象上（已核对 GitHub REST 文档）。
- 删除死变量 `selection_all` 与 `if total == 0 { let _ = &mut selection_all; }` 块。
- 前端已对 `selection_all` 映射到 `repository_scope_blocked`（`OnboardingPage.svelte:203`，展示「调整安装范围」文案），无需改前端。
- 新增测试：installation 带 `repository_selection: "all"` -> 返回 `SelectionAll`。现有测试 mock 未带该字段，`get(...)` 返回 None，行为不变，不会破坏。

---

## 4. 移除 trash，本地删除直接删除（后端）

open question 4：`.skill-sync-trash` 无 GC 无限增长。用户决定不要 trash 安全网，直接删。

- `local_apply.rs`：删除 `trash_dir` 与 `move_to_trash`；更新文件头注释与 `recover_pending` 注释中 `TrashMoveFailed` 字样。
- `apply.rs` 删本地循环（613-644）：`move_to_trash(&target, &trash)` 改为 `fs::remove_dir_all(&target)`（保留 `if target.exists()` 守卫）；失败时 `return Err(AppError::Vault(format!("local delete failed for {}: {e}", dl.skill_id)))`。
  - 失败处理取舍：原 `trash_move_failed` 走 RecoveryRequired，但其 resume 只裁决远端、不重试本地删除，本身已不连贯；直接删除失败极少见（Windows 文件占用），返回 Err 更简单且自愈——下次 sync 仍会判定该 skill 为 `RemoteDeleted` 并重试删除。删除 `save_recovery_journal(..., "trash_move_failed", ...)` 与对应 `RecoveryRequired` 返回。
  - 移除 import 中的 `trash_dir` / `move_to_trash`（13-14 行）。
- `model.rs`：`RecoveryPhase` 删除 `TrashMoveFailed`。
- `commands.rs:317`：删除 `"trash_move_failed" => RecoveryPhase::TrashMoveFailed` 映射（旧 journal 命中走 `_ => RemoteOutcomeUnknown` 兜底，可接受）。
- `src/shared/schemas/apiResponse.ts:21`：`recoveryInfoSchema.phase` 枚举删除 `'trash_move_failed'`。
- `detect.rs`：skip 列表（76、215、386、400）删除 `.skill-sync-trash`（feature 已移除，引用变死）。
- `apply.rs` 测试 `remote_deleted_moves_local_skill_to_trash_with_zero_commit`（1390）：重命名为 `remote_deleted_directly_removes_local_skill_with_zero_commit`，断言改为「target 不存在、且不存在 `.skill-sync-trash` 目录」。
- `RecoveryCard.svelte` 无需改（通用渲染 `recovery.phase`，无 per-phase 分支）。

---

## 验证

- 前端：`npm run lint`、`npm run typecheck`、`npm run build`；schema 枚举变更顺跑 `npm run lint:i18n`。
- 后端：`cargo test`（repository.rs discovery + apply.rs 删除路径）、`cargo build`；按记忆 `cargo clippy` 手动跑（不在自动链）。
- 报告实际命令输出，不臆测成功。
