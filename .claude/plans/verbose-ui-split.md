# 冗长 UI 子组件拆分计划

## 目标与约束

将三个最大的页面文件的冗长 UI 标记拆分为展示型子组件。**不改变 UI 外观、不改变逻辑**：

- 所有 state、query、mutation、`$effect`、事件处理函数保留在页面内。
- 子组件只接收 props（数据 + 回调），仅拥有与自身标记强耦合的展示型常量/格式化函数。
- 严格沿用现有组件范式（`interface Props` + `$props()`，从 `@/shared/ui`、`@/shared/i18n`、`@/shared/lib` 及模块内相对路径导入）。
- 不新增 i18n key、不新增色 token、不改变 class；仅做标记搬运。
- 新增注释用中文；保留既有注释。

---

## 1. OnboardingPage（622 -> 约 520）

`<script>`（编排逻辑，416 行）原样保留；`{#key stage}` + `in:fade` 外壳保留。把 4 个最冗长的阶段卡抽到 `src/modules/onboarding/components/`，其余 ≤16 行的小阶段（loading、app_not_configured、checking_vault、vault_unavailable、confirm_initialize、invalid_manifest、rate_limited）保持内联。

| 新组件                            | 覆盖阶段                                     | Props                                                                                       |
| --------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `DeviceAuthorizationStage.svelte` | `authorize` / `device_pending`（设备码面板） | `stage`, `busy`, `deviceFlow`, `deviceExpiresAt`, `onStart`, `onCopyCode`, `onOpenExternal` |
| `InstallAppStage.svelte`          | `install_app` / `repository_scope_blocked`   | `stage`, `installUrl`, `busy`, `onOpenExternal`, `onCheckInstallation`                      |
| `SelectBranchStage.svelte`        | `select_branch`                              | `remote`, `branchNames`, `selectedBranch`(bindable), `busy`, `onChooseBranch`               |
| `VaultReadyStage.svelte`          | `ready`（含 rebind 确认 Callout）            | `remote`, `bindingChanged`, `confirmRebind`(bindable), `busy`, `onBindVault`                |

页面内回调直接传入：`startDeviceAuthorization` / `copyDeviceCode` / `openExternal` / `discoverRepository` / `chooseBranch` / `bindVault`。

---

## 2. SyncPreviewPage（549 -> 约 330）

`<script>` 全部保留（含 `metric` snippet 删除后改用组件）。抽到 `src/modules/sync/components/`：

| 新组件                     | 覆盖区块                               | Props                                                                                                        |
| -------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `SyncMetric.svelte`        | `metric` snippet（4 处复用）提升为组件 | `label`, `value`, `icon`, `tone?`, `filter?`, `activeFilter`, `onFilter?`                                    |
| `RecoveryCard.svelte`      | recovery-required 卡                   | `recovery`, `resumePending`, `onResume`                                                                      |
| `SyncFilterBar.svelte`     | 搜索 + 状态筛选 + 全选/清空            | `search`(bindable), `statusFilter`(bindable), `canSelectAll`, `canSelectNone`, `onSelectAll`, `onSelectNone` |
| `SyncCommitSummary.svelte` | 提交摘要块                             | `summary`, `localStateUpdates`, `hasLocalStateUpdates`                                                       |
| `SyncApplyBar.svelte`      | 底部 sticky Apply 条                   | `selectedCount`, `willCreateCommit`, `showCommitHint`, `canApply`, `applyPending`, `onApply`                 |

`transition:fly`（planNotice）、`in:fade`+`animate:flip`（列表项）、`ConflictDetailDialog`、空态/骨架/错误态保持页面内。

---

## 3. SettingsPage（369 -> 约 190）

`<script>` 逻辑保留；仅把仅被对应区块使用的展示型常量/格式化函数随标记迁入子组件。抽到 `src/modules/settings/components/`：

| 新组件                          | 覆盖区块                        | 迁入的展示辅助              | Props                                                                       |
| ------------------------------- | ------------------------------- | --------------------------- | --------------------------------------------------------------------------- |
| `GithubVaultCard.svelte`        | 只读 vault 信息 `dl` + 操作按钮 | `credentialStatusLabelKeys` | `config`, `appState`, `onReconfigure`, `onDisconnect`, `disconnectDisabled` |
| `LimitsCard.svelte`             | limits 表单                     | `formatBytes`               | `limits`(bindable), `limitsInvalid`                                         |
| `SkillRootsSection.svelte`      | skill roots 网格                | `namespaceLabelKeys`        | `isLoading`, `roots`, `onOpenPath`, `onCopyPath`                            |
| `DisconnectGithubDialog.svelte` | 断开确认 Dialog                 | —                           | `open`(bindable), `onConfirm`                                               |

`roots` 类型用 `Awaited<ReturnType<typeof scanSkills>>['roots']` 推导，避免跨模块深导入 schema。外观/语言两张小卡（直接用全局 `themeState`/`languageState`）保持内联。

---

## 遵守的规则

- import 流向 `modules -> shared`，无跨模块深导入、无循环/自导入、无 `export *`。
- 无薄包装：每个组件都拥有真实标记 + 自身展示辅助，非纯转发。
- `bind` 字段（`selectedBranch`、`limits`、`search`、`statusFilter`、`open`、`confirmRebind`）用 `$bindable` props，保持双向绑定语义不变。

## 完成前验证

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
npm run format:check
npm run lint
npm run build
```
