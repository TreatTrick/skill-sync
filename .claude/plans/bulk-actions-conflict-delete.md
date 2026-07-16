# 冲突与待删除的批量操作

## 目标

1. **冲突选项卡**：在卡片列表最上方加两个批量按钮 —— 紫色「全部使用云端」、绿色「全部保留本地」，免去逐张卡片点开「查看冲突详情」。
2. **待删除选项卡**：把「删除」拆成两个二级选项卡 —— 「待删除云端」（红色主题）与「待删除本地」（淡红色主题），各自卡片列表上方再给两个批量按钮（红 = 全部删除该侧 / 绿 = 全部恢复或保留该侧）。

## 现状关键事实（已核对）

- 过滤器是 `SyncFilterBar` 里的一个 `Select` 下拉，`SyncStatusFilter` 取值 `all | synced | local_update | remote_update | deleted | conflict`（`src/modules/sync/lib/syncStatus.ts`）。`deleted` 过滤只匹配 `status === 'local_deleted' | 'remote_deleted'`（`matchesStatusFilter`）。
- 决策存在 `syncDecisions` 单例（`state/syncDecisions.svelte.ts`，key = `skill_id`）。
- 后端 `plan.rs:96` 的真值表确认：
  - `status === 'conflict'` 的条目有 4 种 `conflict_reason`：`both_changed`、`same_name_first_seen`（选项 `keep_local/use_remote/skip`）、`local_deleted_remote_changed`（`delete_remote/restore_remote/skip`，且 `delete_direction='delete_remote'`）、`remote_deleted_local_changed`（`keep_local/accept_delete/skip`，且 `delete_direction='delete_local'`）。
  - 即「删除类冲突」status 是 `conflict`，只出现在**冲突**选项卡，不在「删除」选项卡。
  - 「删除」选项卡只剩纯删除：`local_deleted`（`delete_direction='delete_remote'`，恢复 = `restore_remote` / 删除 = 选中 action_id）与 `remote_deleted`（`delete_direction='delete_local'`，保留 = `keep_local` / 删除 = 选中 action_id）。
- 冲突条目 `isSelectable` 不含 `conflict`，不可勾选，只能通过 `decisions` 处理；删除条目通过「勾选 action_id = 执行删除」或「设 recovery decision = 恢复/保留」二选一（`handleDeleteDecision` 已保证互斥）。
- 颜色 lint (`scripts/check-color-tokens.mjs`) 禁止 Tailwind 调色板色名（red/green/purple…）与字面色值，只能用语义 token。可用：`remote*`(紫)、`success*`(绿)、`destructive*`(红，含 `-foreground/-muted/-border`)。实色按钮仅 `primary`/`destructive` 有 foreground token。
- i18n lint 禁止 `.ts/.svelte` 任何中文字符（含注释）→ 代码注释用英文，中文文案写进 `zh-CN.json` + `en-US.json`。

## 设计决策

### A. 冲突批量按钮的语义映射（重要）

「全部使用云端 / 全部保留本地」按 reason 做语义映射，覆盖**全部**冲突（含删除类冲突），这样真正一键完成、不留漏网：

| reason                                               | 使用云端(紫)                             | 保留本地(绿)                             |
| ---------------------------------------------------- | ---------------------------------------- | ---------------------------------------- |
| `both_changed` / `same_name_first_seen`              | `use_remote`                             | `keep_local`                             |
| `local_deleted_remote_changed`（本地已删、云端改了） | `restore_remote`（把云端拉回本地）       | `delete_remote`（保持本地已删 → 删云端） |
| `remote_deleted_local_changed`（云端已删、本地改了） | `accept_delete`（接受云端删除 → 删本地） | `keep_local`（保留本地 → 重传）          |

语义统一为「采纳云端状态 / 采纳本地状态」。映射逻辑放进 `syncStatus.ts` 的新函数 `bulkConflictDecision(reason, bias)`，便于测试与复用。

### B. 「删除」用二级选项卡，不拆主下拉

保留主 `Select` 的「删除」项（不破坏 `SyncMetric`「待删除」点击联动）。当 `statusFilter === 'deleted'` 时，在批量操作栏内渲染一组带 tone 的二级 tab：待删除云端（实色红选中态）/ 待删除本地（淡红选中态）。新增状态 `deleteSubFilter: 'delete_remote' | 'delete_local'`，`visibleEntries` 在 `deleted` 下再按 `delete_direction` 过滤。

### C. 颜色（全部语义 token，过 lint）

- 紫按钮（使用云端）：`bg-remote-muted text-remote border border-remote-border`
- 绿按钮（保留/恢复）：`bg-success-muted text-success border border-success-border`
- 红按钮（删除）：`variant="destructive"`（实色 `bg-destructive text-destructive-foreground`，符合现有设计语言里「危险实色」）
- 二级 tab 选中态：待删除云端 = `border-destructive bg-destructive text-destructive-foreground`（深红）；待删除本地 = `border-destructive-border bg-destructive-muted text-destructive`（淡红）。未选中 = `border-border bg-surface text-foreground`。

不新增 CSS token，不改 `index.css`。

### D. 组件拆分

新建 `src/modules/sync/components/SyncBulkActionBar.svelte`，承载「判断当前过滤器下显示哪些批量按钮 / 二级 tab」的渲染逻辑（非纯转发，符合规则 4）。`SyncPreviewPage` 持有状态与处理函数，把 `statusFilter`、`deleteSubFilter`(bindable)、计数、回调传给它。

## 改动清单

### 1. `src/modules/sync/lib/syncStatus.ts`

- 新增 `bulkConflictDecision(reason: ConflictReason, bias: 'remote' | 'local'): SyncDecision`（表 A 的映射）。
- 新增 `matchesDeleteDirection(entry, sub: 'delete_remote' | 'delete_local'): boolean`（按 `entry.delete_direction` 判断）。
- 新增 `countDeleteDirections(entries): { delete_remote: number; delete_local: number }`。

### 2. `src/modules/sync/components/SyncBulkActionBar.svelte`（新建）

- Props：`statusFilter`、`deleteSubFilter`(bindable)、`deleteCounts`、`conflictCount`、`onBulkConflict(bias)`、`onBulkDelete(action)`。
- `statusFilter === 'conflict'` 且 `conflictCount > 0`：渲染紫/绿两按钮。
- `statusFilter === 'deleted'`：渲染二级 tab（红/淡红，带计数）+ 当前子 tab 对应的红/绿两按钮；计数为 0 时按钮 `disabled`。
- 按钮文案走 `t(...)`，尺寸 `sm`，`flex flex-wrap gap-2`。

### 3. `src/modules/sync/pages/SyncPreviewPage.svelte`

- 新增 `let deleteSubFilter = $state<'delete_remote' | 'delete_local'>('delete_remote')`。
- `visibleEntries`：`matchesEntry(...) && (statusFilter !== 'deleted' || matchesDeleteDirection(entry, deleteSubFilter))`。
- 新增 `deleteCounts`、`visibleConflictCount` 派生。
- `$effect`：`statusFilter === 'deleted'` 时若当前子 tab 计数为 0 而另一侧 > 0，自动切到有数据的一侧（首次进入体验）。
- 新增处理函数：
  - `applyBulkConflictDecision(bias)`：遍历 `visibleEntries` 里的冲突条目，查 `planData.conflicts` 取 reason，调 `bulkConflictDecision` 得 decision，`syncDecisions.setDecision`。
  - `applyBulkDeleteDecision(action: 'delete' | 'recover')`：对当前可见删除条目 —— `delete` = 清除其 recovery decision + 把 action_id 并入 `selectedActionIds`；`recover` = 设 `restore_remote`(delete_remote 子 tab) 或 `keep_local`(delete_local 子 tab) + 从 `selectedActionIds` 移除。批量用集合运算，O(n)。
- 在 `SyncFilterBar` 之后、卡片 grid 之前插入 `<SyncBulkActionBar ... />`。
- 批量只设选择/决策，不绕过 `delete_guard_ack`（用户仍需勾「我确认这些删除操作」，`canApply` 已反映）。

### 4. i18n（`zh-CN.json` + `en-US.json`，`sync` 下新增）

- `sync.bulk.useAllRemote`：全部使用云端 / Use all remote
- `sync.bulk.keepAllLocal`：全部保留本地 / Keep all local
- `sync.bulk.deleteAllRemote`：全部删除云端 / Delete all remote
- `sync.bulk.restoreAllRemote`：全部恢复云端 / Restore all remote
- `sync.bulk.deleteAllLocal`：全部删除本地 / Delete all local
- `sync.deleteTabs.deleteRemote`：待删除云端 / Pending remote delete
- `sync.deleteTabs.deleteLocal`：待删除本地 / Pending local delete

## 不改动

- `SyncFilterBar`（全选/全不选基于 `visibleEntries`，已自动跟随子 tab，无需改）。
- `SyncSkillCard`、`ConflictDetailDialog`（单卡决策路径不变，批量与单卡可叠加/覆盖）。
- 后端、`index.css`、schema。

## 验证

1. `npm run lint:responsive && npm run lint:colors && npm run lint:i18n`
2. `npm run lint`（typecheck + eslint + 上述三项）
3. `npm run build`
4. 手动：冲突选项卡点「全部使用云端/保留本地」→ 卡片 decision badge 全部更新；删除选项卡切两个子 tab → 批量删除/恢复 → 勾选确认 → 应用同步，结果与逐张操作一致。
