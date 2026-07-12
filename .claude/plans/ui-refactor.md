# UI 重构执行计划（落地 `docs/ui-refactor.md`）

## 范围与方式

- 全量 P0 + P1 + P2（F0–F27 共 28 项），目标 60 → 92/100。
- 新增 shadcn 组件用 `npx shadcn-svelte@latest add <name>` 联网拉取（用户已确认）；若联网/版本失败，回退为手搓 bits-ui 封装等价组件，并记录原因。
- 手搓组件（无 shadcn 等价或文档钦定手建）：`Callout`、`SegmentedControl`、`OnboardingStepper`。
- 严格遵守 CLAUDE.md / AGENTS.md：中文文案进 locales、只用语义色 token、无任意 text/rounded 尺寸、import 流向 routes→app→modules→shared、Prettier 管格式。

## 依赖顺序

- F0（补 success/info border token）→ F2（Callout 依赖 info-border/success-border）。
- F6（Select 组件）→ F10（筛选标签 t() 包裹，随 Select 一并替换）。
- F26 color-scheme / lang 同步独立先行（零成本高收益）。
- 其余按 P0 → P1 → P2 顺序，批内可并行。

---

## 批次 P0：观感事故 & 一致性（零成本高收益）

### P0-1 全局地基

- **F0** `src/index.css`：`:root`/`.dark` 补 `--success-border`、`--info-border`；`@theme inline` 补 `--color-success-border`、`--color-info-border`。
- **F4** `src/index.css`：正文字体栈改为 `'Segoe UI', system-ui, 'Microsoft YaHei', 'PingFang SC', sans-serif`；`@theme` 新增 `--font-mono: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace`。
- **F23** `src/index.css`：加 `input[type='number']::-webkit-inner/outer-spin-button { appearance: none; margin: 0 }`。
- **F26-B** `src/index.css`：`:root { color-scheme: light }`、`.dark { color-scheme: dark }`。
- **F22** `src/index.css`：加 `@media (prefers-reduced-motion: reduce)` 全局降级。
- **F26-A** `src/shared/state/language.svelte.ts`：`setLanguage` 内追加 `document.documentElement.lang = language`。

### P0-2 i18n key 补齐（先补 key 再写 UI）

`src/shared/i18n/locales/{zh-CN,en-US}.json` 两份同步新增：

- `common.actions.close`（F24）、`common.actions.copy`（F14）
- `sync.selectedCount`（带 `{{count}}`，F13）
- `sync.emptyAllSynced`、`sync.emptyAllSyncedDescription`、`sync.clearFilters`（F27）
- `github.deviceCodeCopied`（F14 设备码复制反馈）
- `settings.disconnectTitle`（F9 AlertDialog 标题，复用已有 `disconnectConfirm` 作描述、`disconnectGithub` 作确认按钮）

### P0-3 Callout 原语 + 替换横幅（F0→F2）

- 新建 `src/shared/ui/callout/`（`callout.svelte` + `index.ts`），按文档 tone×图标×`rounded-md` 方案；在 `src/shared/ui/index.ts` 导出。
- **F2** 替换 4+ 处手写横幅：
  - `SyncPreviewPage.svelte`：planNotice、deleteGuard、resultMsg、resultStateMsg → `<Callout tone=...>`
  - `OnboardingPage.svelte`：message 错误卡、rebind 确认框、设备码框 → Callout/圆角面板
- **F1** 指标卡 `metric` snippet 补 `rounded-md`；散点 `rounded-lg`/`rounded-sm`/`rounded-none` 归位到 `rounded-md`/`rounded-xl`/`rounded-full` 三档。

### P0-4 字重阶梯（F3）

- 页面级主标题 `font-bold` → `font-semibold`：`AppLayout.svelte` h1、`SyncPreviewPage`（随 F15 删除）、`ConflictDetailDialog`、`SyncSkillCard` h3、`SettingsPage` skillRoots h2、`OnboardingPage` 各 h2、`empty-state` 标题。
- `font-bold` 仅保留：品牌名（AppLayout `font-extrabold` 保留）、侧栏 active 项、指标数值、commitSummary 数字行。

### P0-5 去重与原生控件速修（F15 / F8 / F10）

- **F15** `SyncPreviewPage.svelte`：删除页面内重复 `<h1>` 块，`sync.description` 降为页面顶部一行 `text-sm text-muted-foreground` 引言（顶栏 h1 已有标题，不重复）。
- **F8** `OnboardingPage.svelte`：rebind 原生 `<input type=checkbox>` → `<Checkbox bind:checked>`。
- **F10** `SyncPreviewPage.svelte`：`statusFilterLabel` 包 `t()`（随 P1 F6 Select 一并替换，过渡期先包 t）。

### P0-6 验证

`npm run lint:responsive && npm run lint:colors && npm run lint:i18n && npm run format:check && npm run lint && npm run build`

---

## 批次 P1：交互反馈与关键控件

### P1-1 新增 shadcn 组件（npx 联网拉取）

依次执行（失败则回退手搓 bits-ui 封装）：

- `npx shadcn-svelte@latest add select`（F6）
- `npx shadcn-svelte@latest add alert-dialog`（F9）
- `npx shadcn-svelte@latest add sonner`（F11，会装 `sonner-svelte` 依赖）
- `npx shadcn-svelte@latest add skeleton`（F12）
  拉取后核对生成文件 import 路径、在 `src/shared/ui/index.ts` 补导出；`sonner` 的 `Toaster` 需在根 `src/routes/+layout.svelte` 渲染，并 re-export `toast`。

### P1-2 Toast 迁移瞬时反馈（F11 / F14）

- `SettingsPage.svelte`：复制路径成功/失败、保存成功/失败 → `toast.success/error`，移除顶部 `saveError || actionMessage` 持久 Card。
- `OnboardingPage.svelte`：设备码复制 → `toast.success(t('github.deviceCodeCopied'))`。
- 约定：瞬时结果走 Toast；需用户决策/持续错误（加载失败、recovery、限流）保留页内 Callout。

### P1-3 Select 替换原生 select（F6 / F10）

- `OnboardingPage.svelte` 分支选择、`SyncPreviewPage.svelte` 状态筛选 → `<Select>`；option 文案过 `t()`（修 F10）。

### P1-4 AlertDialog 替换 window.confirm（F9）

- `SettingsPage.svelte` 断开 GitHub → 受控 `<AlertDialog>`，确认按钮 `variant="destructive"`。

### P1-5 Dialog 本地化 Close（F24）

- `dialog-content.svelte` sr-only `Close` → `t('common.actions.close')`；`dialog-footer.svelte` 默认 Close 按钮 → `t('common.actions.close')`（加 `closeLabel` prop 可选，默认回退该 key）。

### P1-6 Onboarding Stepper（F7）

- 新建 `src/modules/onboarding/components/OnboardingStepper.svelte`（模块私有，仅 Onboarding 用，遵循 AGENTS.md 不轻易进 shared）：5 节点 + 连接线 + 当前步高亮，替换原生 `<progress>`。

### P1-7 Sync sticky 操作条（F13）

- `SyncPreviewPage.svelte` 底部 `sticky bottom-0` 条：提交摘要一行 + `sync.selectedCount` + Apply 主按钮；顶部保留 Recheck，移除顶部 Apply（避免双主按钮）。

### P1-8 等宽字体应用（F4）

- `SyncSkillCard`、`ConflictDetailDialog`、`SettingsPage`、`OnboardingPage` 的 hash/ID/设备码/remote_commit → `class="font-mono ..."`。

### P1-9 验证

同 P0-6。

---

## 批次 P2：打磨与动效

### P2-1 动效与微交互（F18 / F19）

- **F18**：`SyncPreviewPage` 横幅 `transition:fly`、列表项 `in:fade + animate:flip`；`OnboardingPage` `{#key stage} in:fade`。统一时长 100–150ms。
- **F19**：`buttonVariants` base 加 `active:scale-[0.98]`（配合 `transition`）；`SyncSkillCard` 选中态 `border-primary bg-primary-muted/30` + hover `hover:border-border-strong hover:shadow-md`；指标卡 hover（随 F27 可点）。

### P2-2 骨架屏（F12）

- `SyncPreviewPage` `plan.isLoading` → 4 张 Skeleton 卡占位；`SettingsPage` `scan.isLoading` → Skeleton 卡。

### P2-3 Settings 定义列表（F16）

- `SettingsPage` GitHub 信息卡：`grid sm:grid-cols-2` 行 → `<dl class="divide-y divide-border-muted">` + `grid-cols-[140px_1fr]`（140px<240 不触发响应式 lint），技术值 `font-mono`。

### P2-4 Onboarding hero 化 + 独立壳留白（F17）

- `OnboardingPage` 根节点加 `min-h-screen px-4 py-10 sm:py-16`；每步加图标徽章 `size-10 rounded-full bg-primary-muted`；文本链接 → `Button variant="outline"` + `ExternalLink`。

### P2-5 空态语义分叉 + 指标卡可点（F27）

- `SyncPreviewPage`：`planData.entries.length === 0` → "全部已同步 🎉"（success，无动作）；`entries>0 && visible.length===0` → 现文案 + "清除筛选"按钮。
- 指标卡可点：点"冲突"→ `statusFilter='conflict'`，待上传/下载同理；`cursor-pointer hover:border-border-strong` + `aria-pressed`。

### P2-6 EmptyState / 阴影 / 焦点 / 杂项（F21 / F5 / F20 / F22）

- **F21** `empty-state.svelte`：图标套 `size-14 rounded-full bg-surface-muted`；`p-8`→`p-10`；标题 `text-base font-semibold`。
- **F5** 可交互卡片 `transition-shadow hover:shadow-md`（SyncSkillCard、Settings skill root 卡、指标卡）。
- **F20** 焦点统一为 `focus-visible:ring-2 focus-visible:ring-ring/40`：`badge.svelte`（`focus:ring`→`focus-visible:ring-2 ring-ring/40`）、`checkbox.svelte`（`ring-1`→`ring-2 ring-ring/40`）；Onboarding select 随 F6 解决。
- **F22** 抽 `src/shared/ui/segmented-control/`（手搓），替换 `SettingsPage` 主题/语言两处手写分段按钮；AppLayout logo `alt={t('layout.brandTitle')}`。

### P2-7 对比度基线（F25）

- `src/index.css` 末尾加注释，记录 F25 实测 9 组 AA 配对作为调色回归基线（守成项，不改色值）。

### P2-8 验证

同 P0-6，并执行逐屏视觉 QA（浅/深色）：Onboarding / Sync / Settings / 全局，对照文档第七节验收清单。

---

## 风险与回退

- npx 联网失败 / 版本不兼容 → 该组件回退手搓 bits-ui 封装（与现有 Dialog/Checkbox 同源），token 走已有 `--popover`/`--muted` alias，不影响功能。
- sonner 若装包失败 → 回退自建 Toast store + 右下角容器（零依赖）。
- 任何批次 lint 失败 → 当批修复后再进下一批，不累积技术债。

## 交付

- 改动文件集中在：`src/index.css`、`src/app.html`(可能不改)、`src/routes/+layout.svelte`、`src/app/layouts/AppLayout.svelte`、`src/shared/state/language.svelte.ts`、`src/shared/i18n/locales/*`、`src/shared/ui/{callout,select,alert-dialog,sonner,skeleton,segmented-control,dialog,empty-state,badge,checkbox,button,index.ts}`、`src/modules/{sync,onboarding,settings}/**`、`src/modules/onboarding/components/OnboardingStepper.svelte`。
- 最终跑 `npm run lint && npm run build` 通过即交付。
