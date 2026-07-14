# 删除后悔药:重新下载 / 重新上传

## 背景与目标

当前同步预览里,"删本地 / 删云端"是系统根据三方比较推断出来的建议(base 有、某一侧没了)。
用户若不想删,只能这次不勾选,但下次同步还会再提示,没有"恢复"路径。

用户诉求:删除提示本质是"一边没了、另一边有",后悔药就应是对称地把还有的那侧复制到没了的那侧:

- 云端删了、本地有 → **重新上传到云端**(把本地推回去,云端恢复)
- 本地删了、云端有 → **重新下载到本地**(从云端拉回来,本地恢复)

两个动作代码里已有名字(`KeepLocal` / `RestoreRemote`),也已实现路由,只是只对"删除侧冲突"(row 10/12)开放,没对"纯删除"(row 9 `LocalDeleted` / row 11 `RemoteDeleted`)开放。本方案把它们对称地补上,**完全复用现成的上传/下载路径,不引入持久忽略标记,不留长期分叉**。

## 当前机制(改动前提)

- `derive_entry`([plan.rs:96-174](src-tauri/src/sync_engine/plan.rs#L96-L174))按真值表推断 status。row 9 = `LocalDeleted`、row 11 = `RemoteDeleted`,二者都不是 `Conflict`,故不在 `plan.conflicts` 里。
- apply 校验([apply.rs:434-448](src-tauri/src/sync_engine/apply.rs#L434-L448))要求 `request.decisions` 的每个 skill_id 必须能在 `plan.conflicts` 找到,否则 `Blocked("decision for non-conflict skill")`。**这是 row 9/11 走不了 decision 的直接原因。**
- decision 路由([apply.rs:533-580](src-tauri/src/sync_engine/apply.rs#L533-L580))已实现:`KeepLocal`→`uploads`、`UseRemote|RestoreRemote`→`download_items`、`DeleteRemote`→`delete_remote_ids`、`AcceptDelete`→`delete_local_items`。但路由从 `conflict` 取 `namespace/folder_name/relative_dir`,对 row 9/11 同样会因找不到 conflict 而失败。
- `allowed_decisions(reason: ConflictReason)`([apply.rs:174-192](src-tauri/src/sync_engine/apply.rs#L174-L192))按冲突原因分发白名单,row 9/11 没有 `conflict_reason`。
- `has_delete` / delete_guard([apply.rs:451-468](src-tauri/src/sync_engine/apply.rs#L451-L468))只把 `DeleteRemote`/`AcceptDelete` 及 selected 的 `LocalDeleted`/`RemoteDeleted` 算作删除。恢复类 decision(`RestoreRemote`/`KeepLocal`)不计入 → 选恢复天然不触发 delete_guard,**无需改**。
- 前端 `syncDecisions` state、`summarizeSyncSelection`([syncStatus.ts:57-81](src/modules/sync/lib/syncStatus.ts#L57-L81))已把 decision 计入 uploads/downloads。
- apply 命令纯透传([commands/sync.rs:120-149](src-tauri/src/commands/sync.rs#L120-L149)),**无需改**。
- plan fingerprint 不含 decision、不改 row 9/11 的 status → **fingerprint 不受影响**。

## 方案

### 后端 — `src-tauri/src/sync_engine/apply.rs`

#### 1. 新增按 status 的 decision 白名单

```rust
fn allowed_decisions_for_status(status: SyncStatus) -> &'static [SyncDecision] {
    match status {
        SyncStatus::LocalDeleted => &[SyncDecision::RestoreRemote, SyncDecision::DeleteRemote, SyncDecision::Skip],
        SyncStatus::RemoteDeleted => &[SyncDecision::KeepLocal, SyncDecision::AcceptDelete, SyncDecision::Skip],
        _ => &[],
    }
}
```

与现有 conflict 删除侧白名单一致(`local_deleted_remote_changed` → `[delete_remote, restore_remote, skip]`、`remote_deleted_local_changed` → `[keep_local, accept_delete, skip]`),只是按 status 而非 reason 分发。

#### 2. 校验循环放宽(改 [apply.rs:434-448](src-tauri/src/sync_engine/apply.rs#L434-L448))

对每个 `(skill_id, decision)`:

1. 先查 `plan.conflicts` → 命中则走原 `allowed_decisions(conflict.conflict_reason)`(conflict 行为不变);
2. 否则查 `plan.entries` → 命中且 status ∈ {`LocalDeleted`,`RemoteDeleted`} 则走 `allowed_decisions_for_status(entry.status)`;
3. 否则 `Blocked("decision for unknown/non-deletable skill")`。

#### 3. 互斥防御(新增,放在 selected_set 校验之后)

同一 skill 不能既走 selected delete 又走恢复 decision。对每个有 decision 的 skill:

- 若 decision ∈ {`RestoreRemote`, `KeepLocal`} 且其 delete action_id(`delete_remote:{id}` / `delete_local:{id}`)在 `request.selected_action_ids` → `Blocked("conflicting delete + recovery decision")`。

(纯删除类 decision `DeleteRemote`/`AcceptDelete` 与 selected delete 语义等价,允许并存即可,apply 幂等。)

#### 4. decision 路由改用 entry(改 [apply.rs:533-580](src-tauri/src/sync_engine/apply.rs#L533-L580))

把路由循环开头的「找 conflict」改为「找 entry」,字段 `namespace/folder_name/relative_dir` 改从 `entry` 取。

理由:conflict 与其对应 entry 同源,字段等价;改用 entry 后 row 9/11 也能路由,且不改变现有 conflict 行为(已有 conflict 测试需全过)。

- `KeepLocal`:用 `entry.*` + `prepared.packed.get(skill_id)`。row 11 本地有 skill,prepare_plan 会 pack,`packed` 命中 ✓。
- `RestoreRemote`/`UseRemote`:用 `entry.*` + `next_manifest.skills.get(skill_id)`。row 9 云端有 skill,manifest 命中 ✓。
- `DeleteRemote`/`AcceptDelete`/`Skip`:不依赖 conflict 字段,无需改。

#### 5. 测试(`apply.rs` tests,沿用现有 mock 框架)

新增:

- `local_deleted_with_restore_remote_downloads_and_aligns_state`:row 9 + `RestoreRemote` → 1 download、0 delete、0 commit、`state.skills[id].base_hash == remote_hash`、本地目录恢复。
- `remote_deleted_with_keep_local_uploads_and_aligns_state`:row 11 + `KeepLocal` → 1 upload、1 commit、`state.skills[id].base_hash == local_hash`、manifest 恢复该 skill。
- `local_deleted_with_delete_remote_via_decision_equivalent_to_selected`:row 9 + `DeleteRemote` decision → 删云端,等价于勾选。
- `recovery_decision_conflicting_with_selected_delete_blocks`:row 9 同时 selected `delete_remote:{id}` + `RestoreRemote` → `Blocked`、0 commit、state 不变。
- 回归:现有 conflict decision 测试全过(验证路由改 entry 不破坏 conflict)。

### 前端

#### 6. `src/modules/sync/lib/syncStatus.ts`

新增:

```ts
export const deleteDecisionOptions = (
  entry: SyncSkillEntry,
): readonly SyncDecision[] => {
  if (entry.status === 'local_deleted')
    return ['restore_remote', 'delete_remote', 'skip']
  if (entry.status === 'remote_deleted')
    return ['keep_local', 'accept_delete', 'skip']
  return []
}
```

`summarizeSyncSelection` 不改:互斥(见下)保证 selectedEntries 与 decisions 不重叠,不会双重计数。

#### 7. `src/modules/sync/components/SyncSkillCard.svelte`

对 `isDeleteEntry` 的卡片(row 9/11),在 delete_direction badge 下方加恢复按钮区:

- row 9(`delete_direction === 'delete_remote'`):主按钮「重新下载到本地」→ `onDecision('restore_remote')`
- row 11(`delete_direction === 'delete_local'`):主按钮「重新上传到云端」→ `onDecision('keep_local')`

交互:

- 点恢复按钮 → 调 `onDecision(choice)` 设置 decision;卡片用现有 decision badge(`decisionLabelKey`/`decisionTone`)显示已选状态。
- 已选恢复时:checkbox 禁用(避免再勾删除),并提供「撤销」(再点恢复按钮 = 清除该 decision)。
- Props 新增 `onDecision?: (choice: SyncDecision) => void`(仅删除卡片传入)。

#### 8. `src/modules/sync/pages/SyncPreviewPage.svelte`

- 给删除类 `SyncSkillCard` 传 `onDecision`。
- 互斥:点恢复前若 `selectedActionIds` 含该 `action_id`,先 `toggleAction(action_id, false)` 移除勾选。
- 复用现有 `handleDecision`/`syncDecisions.setDecision`(已用于 conflict)。
- `handleApply` 已透传 `decisions: { ...syncDecisions.decisions }`,无需改。
- `canApply` 现有逻辑(`selectionSummary.selected > 0`)已覆盖恢复 decision 计入 selected,无需改。

### i18n — `src/shared/i18n/locales/{zh-CN,en-US}.json`

新增 `sync.deleteRecovery` 段(不复用 `sync.decisions.*`,因 `restore_remote`="恢复云端"在删除场景有歧义):

| key                            | zh-CN          | en-US                |
| ------------------------------ | -------------- | -------------------- |
| `sync.deleteRecovery.download` | 重新下载到本地 | Re-download to local |
| `sync.deleteRecovery.upload`   | 重新上传到云端 | Re-upload to remote  |
| `sync.deleteRecovery.undo`     | 撤销恢复       | Undo recovery        |

decision badge 文案沿用现有 `sync.decisions.*`(冲突页已用,不改)。

## 不做什么

- **不做持久忽略标记**:用户已确认两个场景都用"重新对齐"(恢复),不留长期分叉。
- **不做 trash / git 历史的 UI 恢复入口**:独立改进,与本方案不冲突,以后另议。
- **不改 row 9/11 的 status / fingerprint**:保持纯删除推断语义,fingerprint 稳定。
- **不动 `batch_apply` / `upload_skills` / `download_skills`**:它们不走 decision。

## 风险

1. **路由改 entry 是最大改动点**:必须保证 conflict 行为不变,靠现有 conflict 测试 + 新增测试覆盖。
2. **互斥前后端都要做**:前端 UX 互斥 + 后端防御性 block,任一缺失都会导致同一 skill 既删又恢复的矛盾执行。
3. **decision badge 文案歧义**:`sync.decisions.restore_remote`="恢复云端"在删除卡片上字面像"恢复云端",但实际动作是"从云端下载到本地"。本方案用按钮新文案(`重新下载到本地`)兜底,badge 文案留待后续统一优化。

## 实现顺序

1. 后端:白名单 + 校验放宽 + 互斥防御 + 路由改 entry + 测试 → `npm run rust:clippy`、`cargo test`
2. 前端:`syncStatus.deleteDecisionOptions` + `SyncSkillCard` 按钮 + `SyncPreviewPage` 互斥/回调
3. i18n:新增文案
4. 收尾:`npm run lint:responsive`、`npm run lint:colors`、`npm run lint:i18n`、`npm run lint`、`npm run build`
