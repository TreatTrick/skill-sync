# UI / UX 评审与重构方案（ui-refactor.md）

> 评审范围：仅视觉设计（Visual）与交互体验（UX），不涉及业务逻辑、数据流、Tauri 后端。
> 评审对象：`src/routes`、`src/app`、`src/modules`、`src/shared/ui` 下的全部 Svelte UI（Onboarding / Sync / Settings 三大页面 + 布局 + 设计系统原语）。
> 目标：按本文方案落地后，UI 美学 + 交互体验在下述评分体系中从 **60/100 提升到 ≥90/100**。

**v2 复审更新（2026-07-12）** — 对 v1 做了一轮对照代码的复核，改动如下：

- **订正 v1 中三处技术性错误的方案代码**：F1 radius token 原方案在 `@theme inline` 里自引用（无效写法），改为"沿用 Tailwind 内置 scale + 使用约定"；F4 等宽字栈原列了未打包的 JetBrains Mono，改为系统字体优先；F20 原自造 `@utility focus-ring` 写法不成立，改为钦定现有 `focus-visible:ring-2 ring-ring/40` 组合。
- **新增 6 条遗漏发现**：F0（success/info 缺 `-border` token，Callout 的前置依赖）、F23（原生 number 旋钮）、F24（Dialog 硬编码英文 Close）、F25（对比度实测 9 组全 AA——由"含糊扣分"改为"有硬数据的守成项"）、F26（`<html lang>` 不随语言切换 + 缺 `color-scheme`）、F27（空态语义不分 + 指标卡不可点）；F17 补充 Onboarding 独立壳无纵向留白的问题。
- **补齐落地配套**：新增 i18n key 清单（`sync.selectedCount` 等方案引用的 key 现在并不存在，先补 key 再写 UI）；修复 v1 路线图被格式化压行的问题并纳入新条目。
- 总分维持 **60/100** 不变：新发现是对既有维度判分的进一步佐证（D2/D4/D6 简评已更新），而非新扣分；目标分与预测（92/100）同样维持。

---

## 一、评分体系（我的评价标准）

按 6 个维度加权打分，总分 100。每个维度既看"绝对水准"（是否达到成熟桌面应用/优秀 SaaS 的观感），也看"内部一致性"（同一系统内是否自洽）。

> 打分是专业主观评估，但可证伪的项尽量给硬证据：颜色对比用 WCAG 相对亮度公式实测（见 D6），控件/圆角/字重用代码计数（见各 Finding 的"现状"）。分数用于排优先级，不是精密刻度。

| #   | 维度               | 权重 | 关注点                                            |
| --- | ------------------ | ---- | ------------------------------------------------- |
| D1  | 视觉基础与一致性   | 22   | 颜色语义、字体阶梯、圆角语言、阴影层级、间距节奏  |
| D2  | 设计系统与组件复用 | 18   | 原语是否统一、有没有裸原生控件、token 是否贯彻    |
| D3  | 交互反馈           | 20   | 加载态、成功/失败反馈、瞬时 vs 持久、危险操作确认 |
| D4  | 布局与信息架构     | 15   | 层级清晰度、冗余、主操作可达性、密度              |
| D5  | 动效与微交互       | 12   | 过渡、进入/退出、状态切换的连续感                 |
| D6  | 可访问性与边界状态 | 13   | 焦点、对比、键盘、空态/错误态的打磨               |

### 当前得分：**60 / 100**

| 维度             | 得分    | 简评                                                                                                                                          |
| ---------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| D1 视觉基础      | 16 / 22 | token 体系与暗色模式是亮点；但圆角混乱、扁平面板、字重无阶梯、技术字符串无等宽字体拉低观感                                                    |
| D2 设计系统      | 11 / 18 | 存在裸 `<select>`、`<progress>`、原生 `<input type=checkbox>`、number 旋钮、`window.confirm`；筛选下拉渲染未翻译 key；Dialog 硬编码英文 Close |
| D3 交互反馈      | 11 / 20 | 全靠内联 banner/Card，瞬时反馈（复制成功）被渲染成持久布局；无 Toast、无骨架屏                                                                |
| D4 布局与 IA     | 10 / 15 | Sync 页出现重复标题；主操作 Apply 随长列表滚走；Settings 信息卡密集无分隔；空态不区分"全部同步"与"筛选无结果"                                 |
| D5 动效          | 3 / 12  | 除 Dialog 自带动画外，全站几乎零过渡；页面/卡片/banner 硬切                                                                                   |
| D6 可访问性/状态 | 9 / 13  | 对比度实测全 AA（加分）；但焦点环有遗漏、`<html lang>` 不随语言切换、无 `color-scheme`、无 `prefers-reduced-motion` 兜底                      |

---

## 二、总体印象

**做得好的地方（值得保留）：**

- **语义 token 体系完善。** [src/index.css](src/index.css) 定义了成对的 `surface / border / foreground / primary / warning / destructive / success / info` 及各自的 `-muted`，并通过 `@theme inline` 映射为 Tailwind 颜色类，暗色模式独立提亮（teal `#0f766e → #2dd4bf`）。这是整套 UI 最扎实的地基。
- **颜色对比合规（已实测）。** 对 9 组高频前景/背景 token 用 WCAG 公式核算，全部 ≥4.5:1（正文 AA），无失败项（详见 F25 表）。这点常被忽视，但当前配色是"能直接过无障碍审计"的，属加分项。
- **布局骨架专业。** [AppLayout.svelte](src/app/layouts/AppLayout.svelte) 的可折叠侧栏 + sticky 顶栏 + 面包屑 + `max-w-screen-2xl` 内容区，结构清晰、响应式合理。
- **i18n / 主题基础设施到位**，`t(...)` 贯彻，主题在首帧前同步应用避免闪烁（[+layout.svelte](src/routes/+layout.svelte#L10)）。

**核心问题（一句话概括）：**

> 地基是成熟的，但"精装修"缺位——**圆角/字重/面板样式各写各的、多处裸原生控件破坏视觉语言、反馈机制原始（无 Toast/骨架屏/优雅确认框）、几乎没有动效**。这些正是"能用"与"精致"之间的差距。

下面按维度给出**逐条问题（现状 → 问题 → 方案）**，每条都能直接落地。

---

## 三、问题清单与改进方案

### D1 · 视觉基础与一致性

#### F0｜token 体系不对称：`success` / `info` 缺 `-border` 变体（Callout 的前置依赖）

**现状：** `destructive` / `warning` / `primary` 都有成对的 `--*-border`，但 **`--success` 和 `--info` 没有 `-border`**（[src/index.css](src/index.css)）。后果直接可见：成功横幅只能写 `border-success-muted bg-success-muted`（边框色 = 填充色 ≈ 没有边框，[SyncPreviewPage.svelte:453](src/modules/sync/pages/SyncPreviewPage.svelte#L453)），info 横幅被迫借用 `border-primary-muted`（[SyncPreviewPage.svelte:459](src/modules/sync/pages/SyncPreviewPage.svelte#L459)）。
**问题：** 这是所有 tone 化提示条（F2 的 Callout）无法自洽的根因——5 个 tone 里有 2 个描不出边。不先补齐 token，Callout 就是残缺的。
**方案：** 先补两个 border token（浅/暗各一份），再做 F2。

```css
/* :root */
--success-border: #bbf7d0;
--info-border: #bfdbfe;
/* .dark */
--success-border: #1f4d33;
--info-border: #1e3a5f;
/* @theme inline 追加 */
--color-success-border: var(--success-border);
--color-info-border: var(--info-border);
```

> 注：F2 的 Callout `info` tone 用的正是这里补的 `--info-border`；两条须一并落地，顺序为 **F0 → F2**。

#### F1｜圆角语言不统一（缺使用约定）

**现状：** 全站圆角散落 `rounded-md ×11 / rounded-lg ×6 / rounded-xl ×2 / rounded-sm ×1 / rounded-full ×1`，且有 **7 处内联面板完全没有圆角**。Card 是 `rounded-xl`（[card.svelte:14](src/shared/ui/card/card.svelte#L14)），Button/Input/Badge 是 `rounded-md`，StatusBadge 是 `rounded-full`，而 Sync 页的指标卡、Onboarding 的设备码框都是**直角**。
**问题：** 直角内联面板嵌在圆角卡片里，观感割裂、显廉价。
**方案：** Tailwind v4 本就内置 `--radius-*` 阶梯（md=6px / lg=8px / xl=12px），**问题不在缺 token，而在缺使用约定**。无需重定义 scale（重定义会全局改变 `rounded-md` 等既有类的含义，牵连按钮/输入框），只需三档收敛：

| 层级      | 类                   | 适用                                                    |
| --------- | -------------------- | ------------------------------------------------------- |
| 容器      | `rounded-xl`（12px） | Card、Dialog（现状已是，保持）                          |
| 控件/面板 | `rounded-md`（6px）  | 按钮、输入框、**所有内嵌面板/横幅**（Callout 统一携带） |
| 药丸      | `rounded-full`       | StatusBadge、Stepper 节点                               |

若想要"一个旋钮调全局"，可选 shadcn 模式（`:root { --radius: 0.625rem }` + `@theme inline` 里用 `calc(var(--radius) ± n)` 派生），但对当前体量**不建议**——先把 7 处无圆角面板收口到 `rounded-md`（随 F2 的 Callout 落地自动完成），`rounded-lg`/`rounded-sm`/`rounded-none` 的散点用法替换归位即可。

#### F2｜扁平内联面板泛滥（应抽象为 Callout / Surface）

**现状：** 以下均是手写的 `border ... bg-* p-*` 直角块：

- Sync 指标卡 `border border-border bg-surface p-4`（[SyncPreviewPage.svelte:266](src/modules/sync/pages/SyncPreviewPage.svelte#L266)）
- planNotice / deleteGuard / resultMsg / resultStateMsg 四个横幅（[SyncPreviewPage.svelte:368-462](src/modules/sync/pages/SyncPreviewPage.svelte#L368)）
- Onboarding 设备码框、rebind 确认框（[OnboardingPage.svelte:446](src/modules/onboarding/pages/OnboardingPage.svelte#L446)、[:544](src/modules/onboarding/pages/OnboardingPage.svelte#L544)）

**问题：** 同一种"提示条"在 4 个文件里有 4 种写法，颜色/内边距/图标位置全靠记忆对齐，极易漂移；且都缺圆角。
**方案：** 新增 `Callout` 原语（`src/shared/ui/callout/`），统一 tone × 图标 × 圆角 × 间距。

```svelte
<!-- src/shared/ui/callout/callout.svelte -->
<script module lang="ts">
  import { tv } from 'tailwind-variants'
  export const calloutVariants = tv({
    base: 'flex items-start gap-2.5 rounded-md border p-3 text-sm',
    variants: {
      tone: {
        info: 'border-info-border bg-info-muted text-info-muted-foreground',
        warning: 'border-warning-border bg-warning-muted text-warning',
        danger: 'border-destructive-border bg-destructive-muted text-destructive',
        success: 'border-success-border bg-success-muted text-success',
        brand: 'border-primary-border bg-primary-muted text-primary-muted-foreground',
      },
    },
    defaultVariants: { tone: 'info' },
  })
</script>
<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { VariantProps } from 'tailwind-variants'
  import { cn } from '@/shared/lib/utils'
  type Tone = VariantProps<typeof calloutVariants>['tone']
  let { tone = 'info', icon, class: className, children }:
    { tone?: Tone; icon?: Snippet; class?: string; children: Snippet } = $props()
</script>
<div class={cn(calloutVariants({ tone }), className)}>
  {#if icon}<span class="mt-0.5 shrink-0">{@render icon()}</span>{/if}
  <div class="min-w-0 flex-1">{@render children()}</div>
</div>
```

落地后：所有横幅替换为 `<Callout tone="warning">…</Callout>`，删除内联样式。指标卡直接换用 `Card`（自带 `rounded-xl shadow-sm`），或保留轻量 div 但补 `rounded-md`。

#### F3｜字重/标题阶梯混乱

**现状：** 全站 `font-bold ×19 / font-medium ×12 / font-extrabold ×1`，几乎无 `font-semibold`（仅 CardTitle 原语用了）。页面 h1 用 `font-bold text-2xl`，Sync 页内又出现 `text-xl font-bold` 的二级 h1，Settings 用 `text-lg font-bold` 的 h2，品牌名 `font-extrabold`。
**问题：** 权重与字号没有成体系的阶梯，"到处都很粗"导致层级扁平、缺乏呼吸感。
**方案：** 固定一套 type scale，落到文档约定并逐处替换：

| 角色          | 类                                                                          |
| ------------- | --------------------------------------------------------------------------- |
| 页面主标题 H1 | `text-2xl font-semibold tracking-tight text-strong-foreground`              |
| 区块标题 H2   | `text-base font-semibold text-strong-foreground`                            |
| 卡片标题      | `text-sm font-semibold`（CardTitle 已接近，去掉 `tracking-tight` 之外保持） |
| 正文          | `text-sm text-foreground`                                                   |
| 次要说明      | `text-sm text-muted-foreground`                                             |
| 元信息/标签   | `text-xs font-medium text-muted-foreground`                                 |

核心动作：**把页面级 `font-bold` 主标题统一改为 `font-semibold`**，`font-bold` 仅保留给需要强调的数字（指标值）与品牌名。视觉立刻更克制、更"设计过"。

#### F4｜字体栈两问题：技术字符串无等宽、正文栈 CJK 打头拖累 Latin

**现状 A（等宽缺位）：** hash（`shortHash`）、`remote_commit`、`installation_id`、设备码 `user_code` 全用默认无衬线字体渲染（[SyncSkillCard.svelte:77-87](src/modules/sync/components/SyncSkillCard.svelte#L77)、[SettingsPage.svelte:242](src/modules/settings/pages/SettingsPage.svelte#L242)、[OnboardingPage.svelte:448](src/modules/onboarding/pages/OnboardingPage.svelte#L448)）。`<code>` 标签也没绑定等宽字族。
**现状 B（栈顺序）：** 正文栈是 `'Microsoft YaHei', 'PingFang SC', Inter, Arial`（[index.css:58](src/index.css#L58)）——CJK 字体打头意味着**英文与数字也落在雅黑上**，其 Latin 字形与数字观感明显弱于系统 UI 字体；且雅黑只有 Regular/Bold 两档，配合已设置的 `font-synthesis: none`，`font-semibold(600)` 会被就近解析到 Bold——**中文的 semibold/bold 区分会坍缩**。项目未打包任何字体（无 `@fontsource`/`@font-face`），列 `Inter` 是无效期望。
**问题：** 哈希/ID 这类等宽敏感内容用比例字体，`0/O`、`1/l` 难辨认；Latin/数字质感被雅黑拖低。
**方案：** 全部用系统字体（桌面端零下载成本），Latin 优先、CJK 兜底：

```css
/* :root 内替换现有 font-family（Latin 优先，CJK 随后接管中文字形） */
font-family:
  'Segoe UI', system-ui, 'Microsoft YaHei', 'PingFang SC', sans-serif;
```

```css
/* @theme 内新增（不随明暗变化，直接定义在 @theme，无需 :root/inline 二段式） */
@theme {
  --font-mono:
    ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
}
```

用法：`<span class="font-mono text-xs">{shortHash(entry.local_hash)}</span>`；设备码 `<code class="font-mono text-2xl tracking-widest">`。
**对 F3 的影响：** 栈序修正后，标题里的英文/数字能获得真正的 600 字重；中文字形 semibold≈bold 是平台现实，型阶靠"字号 + 颜色（strong-foreground vs foreground）"双通道兜底，不单赌字重。

#### F5｜定义了 `shadow-md/lg` 却从不使用

**现状：** [index.css:168-176](src/index.css#L168) 定义了 `--shadow-xs/sm/md/lg` 四档，但全站只用到 `shadow-sm/xs`。
**问题：** 缺乏 z 轴层次——卡片、悬浮元素、对话框看起来"贴"在同一平面。
**方案：** 建立层级约定：静态卡片 `shadow-sm`；hover 可交互卡片 `hover:shadow-md transition-shadow`；对话框/弹层 `shadow-lg`。给 SyncSkillCard、Settings 的 skill root 卡片加 `transition-shadow hover:shadow-md`，形成可点击暗示。

---

### D2 · 设计系统与组件复用

#### F6｜裸原生 `<select>`（×2）

**现状：** Onboarding 分支选择（[OnboardingPage.svelte:489](src/modules/onboarding/pages/OnboardingPage.svelte#L489)）与 Sync 状态筛选（[SyncPreviewPage.svelte:390](src/modules/sync/pages/SyncPreviewPage.svelte#L390)）都是原生 `<select>`。前者甚至**没有焦点环样式**，两者下拉面板由操作系统渲染，暗色模式下与整体割裂。
**方案：** 用 `bits-ui` 补一个 `Select` 原语（项目已依赖 bits-ui，与 Checkbox/Dialog 同源）：

```bash
npx shadcn-svelte@latest add select
```

封装到 `src/shared/ui/select/`，导出后替换两处原生 select。收益：自定义下拉、暗色一致、焦点/键盘统一。

#### F7｜裸原生 `<progress>` 进度条

**现状：** Onboarding 顶部 `<progress class="h-2 w-full accent-primary" max="5">`（[OnboardingPage.svelte:407](src/modules/onboarding/pages/OnboardingPage.svelte#L407)）。
**问题：** 原生 `<progress>` 在 Windows WebView2 下渲染风格不可控，`accent-primary` 并不能完全接管轨道/填充配色，圆角也无法与设计系统对齐。而这是**用户进入产品看到的第一屏**。
**方案：** 换成设计系统化的进度组件；鉴于 Onboarding 本质是 5 步流程，建议直接升级为 **Stepper（步骤指示器）**，信息量更大：

```svelte
<!-- 轻量 Stepper：5 个节点 + 连接线，当前步高亮 -->
<ol class="flex items-center gap-2">
  {#each steps as step, i (step)}
    <li class="flex flex-1 items-center gap-2">
      <span class={cn(
        'flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold transition-colors',
        i < current ? 'bg-primary text-primary-foreground'
        : i === current ? 'bg-primary-muted text-primary-muted-foreground ring-2 ring-primary'
        : 'bg-surface-muted text-muted-foreground')}>
        {i + 1}
      </span>
      {#if i < steps.length - 1}
        <span class={cn('h-0.5 flex-1 rounded-full', i < current ? 'bg-primary' : 'bg-border')}></span>
      {/if}
    </li>
  {/each}
</ol>
```

若不想改结构，最小方案是替换为自绘轨道：`<div class="h-2 rounded-full bg-surface-muted"><div class="h-2 rounded-full bg-primary transition-[width]" style:width={`${step/5*100}%`}></div></div>`。

#### F8｜Onboarding 里混用原生 checkbox

**现状：** rebind 确认用 `<input type="checkbox" class="mt-0.5">`（[OnboardingPage.svelte:545](src/modules/onboarding/pages/OnboardingPage.svelte#L545)），而全站其他地方用的是已封装的 `Checkbox`（bits-ui）。
**问题：** 同一应用两种勾选框外观，暗色下原生框尤其突兀。
**方案：** 直接换成 `<Checkbox bind:checked={confirmRebind} />`，与 Sync 页 deleteGuard 一致。

#### F9｜危险操作用 `window.confirm`

**现状：** 断开 GitHub 用 `window.confirm(t('settings.disconnectConfirm'))`（[SettingsPage.svelte:138](src/modules/settings/pages/SettingsPage.svelte#L138)）。
**问题：** 系统级弹窗完全脱离应用视觉语言，且在 Tauri 桌面壳里观感尤其"网页化"、廉价，还无法承载"这将删除远端绑定"这类分级警示。
**方案：** 基于现有 Dialog 封装 `AlertDialog`（确认/取消 + 危险语气）：

```bash
npx shadcn-svelte@latest add alert-dialog
```

断开、以及未来任何破坏性操作（含 Sync 的删除确认）统一走 `AlertDialog`，危险按钮用 `variant="destructive"`。

#### F10｜状态筛选下拉渲染的是**未翻译的 i18n key**（显示 bug）

**现状：** [SyncPreviewPage.svelte:396](src/modules/sync/pages/SyncPreviewPage.svelte#L396) `<option>{statusFilterLabel(filter)}</option>`，而 `statusFilterLabel` 返回的是模板字符串 `` `sync.filters.${filter}` `` 本身（[SyncPreviewPage.svelte:119](src/modules/sync/pages/SyncPreviewPage.svelte#L119)），**没有过 `t()`**。
**问题：** 用户看到的是 `sync.filters.all`、`sync.filters.conflict` 这样的**原始 key**，而非"全部/冲突"。这是当前 UI 上最直接的观感事故。
**方案：** 包一层 `t()`：`{t(statusFilterLabel(filter))}`。并入 F6 的 `Select` 改造一并处理。

#### F23｜Settings 数值输入用原生 number 旋钮

**现状：** limits 区 5 个 `<Input type="number">`（[SettingsPage.svelte:262-281](src/modules/settings/pages/SettingsPage.svelte#L262)）走的是原生数字微调箭头（spinner）。原生旋钮在 WebView2 下宽度/配色不可控，暗色模式尤其突兀，且与 `Input` 组件的圆角/焦点环不统一。
**问题：** 5 个输入框右侧挂着一排风格外挂的系统箭头，是 Settings 页最扎眼的"网页毛边"。
**方案：** 二选一。① 若不需要步进交互，隐藏旋钮更干净：

```css
/* index.css 全局 */
input[type='number']::-webkit-inner-spin-button,
input[type='number']::-webkit-outer-spin-button {
  appearance: none;
  margin: 0;
}
```

② 若要保留步进，封装 `NumberInput`（`Input` + 自绘 ▲▼ 按钮，复用 Button/图标），彻底纳入设计系统。字节值这类场景推荐 ①（用户直接输数，旁边已有 `formatBytes` 实时回显）。

#### F24｜Dialog 原语里硬编码英文 "Close"

**现状：** [dialog-content.svelte:42](src/shared/ui/dialog/dialog-content.svelte#L42) `<span class="sr-only">Close</span>`、[dialog-footer.svelte:28](src/shared/ui/dialog/dialog-footer.svelte#L28) `<Button>Close</Button>`，均为 shadcn 生成后未本地化的英文字面量。
**问题：** 中文界面里读屏用户会听到英文 "Close"；footer 的默认 Close 按钮若被启用会露出英文。与 CLAUDE.md 第 5 条（Chinese copy 走 `t()`）冲突。
**方案：** 关闭按钮的 `sr-only` 文案与 footer 默认按钮接受 `t('common.actions.close')`（该 key 需在 locales 补齐——现有 `common.actions` 有 apply/cancel/… 但**无 close**）。或给 `DialogContent` 增加 `closeLabel` prop 由调用方传入，默认回退到 `common.actions.close`。

---

### D3 · 交互反馈

#### F11｜无 Toast：瞬时反馈被渲染成持久布局

**现状：** "路径已复制"`actionMessage`、保存成功/失败、Sync 应用结果，全部渲染成页面里常驻的 Card/横幅（[SettingsPage.svelte:197-203](src/modules/settings/pages/SettingsPage.svelte#L197)、[SyncPreviewPage.svelte:452-462](src/modules/sync/pages/SyncPreviewPage.svelte#L452)）。复制路径这种**一次性**反馈会把顶部的卡片挤下去、且不会自动消失。
**问题：** 瞬时事件用持久布局承载 → 布局跳动、信息噪音、用户得手动无视。
**方案：** 引入轻量 Toast 系统（`sonner-svelte` 或自建 store + 右下角容器）。

```bash
npx shadcn-svelte@latest add sonner
```

约定：**瞬时结果（复制成功、保存成功、应用完成 N 项）走 Toast**；**需要用户决策/持续存在的错误（加载失败、需要 recovery、限流）保留为页内 Callout**。仅此一条就能显著提升"这个产品很顺手"的感觉。

#### F12｜全是 Spinner，无骨架屏

**现状：** 所有加载态都是居中 `<Spinner class="size-6" />` + `py-12`（Sync/Settings/Onboarding 各处）。
**问题：** 内容区从"空转圈"直接跳到"完整卡片列表"，有明显布局位移（CLS），不够高级。
**方案：** 为 Sync 卡片列表、Settings 信息卡加 `Skeleton` 占位（形状贴合真实卡片），首屏更平滑。

```bash
npx shadcn-svelte@latest add skeleton
```

```svelte
{#if plan.isLoading}
  <div class="grid gap-3 lg:grid-cols-2">
    {#each Array(4) as _, i (i)}
      <div class="rounded-xl border border-border bg-card p-4">
        <Skeleton class="h-5 w-40" />
        <Skeleton class="mt-3 h-3 w-full" />
        <Skeleton class="mt-2 h-3 w-2/3" />
      </div>
    {/each}
  </div>
{/if}
```

#### F13｜主操作 Apply 与提交摘要割裂、随长列表滚走

**现状：** Sync 页顶部右上是 `Recheck / Apply`（[:344-357](src/modules/sync/pages/SyncPreviewPage.svelte#L344)），而"提交摘要"在页面**最底部**（[:432](src/modules/sync/pages/SyncPreviewPage.svelte#L432)）。技能卡多时，Apply 按钮滚出视口，用户看着底部摘要却够不到主按钮。
**问题：** 核心动作的可达性差；"我要提交的内容"与"提交按钮"在空间上分离。
**方案：** 底部增加 **sticky 操作条**（`sticky bottom-0`），聚合"提交摘要一行 + 选中计数 + Apply 主按钮"，随时可点：

```svelte
<div class="sticky bottom-0 z-10 -mx-4 border-t border-border bg-background/85 px-4 py-3 backdrop-blur sm:-mx-6 sm:px-6">
  <div class="mx-auto flex max-w-screen-2xl items-center justify-between gap-3">
    <span class="text-sm text-muted-foreground">
      <!-- sync.selectedCount 为新增 key，见第四节 i18n 清单 -->
      {t('sync.selectedCount', { count: selectedActionIds.length })} · {commitSummaryText}
    </span>
    <Button disabled={!canApply} loading={apply.isPending} onclick={handleApply}>
      {t('common.actions.apply')}
    </Button>
  </div>
</div>
```

顶部保留 Recheck，Apply 主入口下沉到 sticky 条（顶部可保留次级 Apply 或移除，避免双主按钮）。

#### F14｜设备码/路径缺一键复制

**现状：** Onboarding 设备码 `user_code` 需用户**肉眼抄写**到 GitHub（[OnboardingPage.svelte:448](src/modules/onboarding/pages/OnboardingPage.svelte#L448)）；Settings 有复制路径但反馈是持久 Card（见 F11）。
**方案：** 设备码旁加"复制"图标按钮（`navigator.clipboard` + Toast 反馈），并把 `verification_uri` 从下划线文本链接升级为 `Button variant="outline"`（明确可点、命中区更大）。

---

### D4 · 布局与信息架构

#### F15｜Sync 页出现**重复标题**

**现状：** [AppLayout.svelte:130](src/app/layouts/AppLayout.svelte#L130) 顶栏已渲染当前路由标题为 `<h1 text-2xl>`（"同步"），而 [SyncPreviewPage.svelte:340-341](src/modules/sync/pages/SyncPreviewPage.svelte#L340) 页面内又渲染了一个 `<h1 text-xl>{t('sync.title')}` + 描述。
**问题：** 同屏两个 h1、标题重复；语义与视觉双重冗余。
**方案：** 二选一——推荐**保留顶栏 h1，把页面内标题降级为一行副描述**（或直接删标题、只留 `description` 作为顶栏副标题）。理想做法是让 AppLayout 顶栏支持可选 `description`/`actions` 插槽，页面把 Recheck/Apply、说明文字交给顶栏，正文区只放内容。这样 Settings、Onboarding 也能共享统一页头。

#### F16｜Settings 的 GitHub 信息卡：密集 key-value、无分隔

**现状：** [SettingsPage.svelte:219-242](src/modules/settings/pages/SettingsPage.svelte#L219) 是一连串 `grid sm:grid-cols-2` 的 label/value 行，行与行之间无分隔、无节奏。
**问题：** 7~8 行等距灰字堆叠，扫读困难、单调。
**方案：** 用**定义列表 + 细分隔线**，label 定宽、value 右侧对齐；技术值加 `font-mono`（见 F4）：

```svelte
<dl class="divide-y divide-border-muted text-sm">
  <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
    <dt class="text-muted-foreground">{t('settings.repository')}</dt>
    <dd class="font-mono text-foreground">{owner}/{repo}:{branch}</dd>
  </div>
  <!-- … 其余行同结构 -->
</dl>
```

#### F17｜Onboarding 关键步骤缺"主视觉焦点"

**现状：** 授权页把"连接 GitHub"做成普通按钮，设备码框是朴素直角面板；install/branch 等步骤的行动点是下划线文本链接（[:471](src/modules/onboarding/pages/OnboardingPage.svelte#L471)、[:525](src/modules/onboarding/pages/OnboardingPage.svelte#L525)）。
**问题：** 首次配置是"第一印象"，当前每一步都平淡、行动点不突出。
**方案：** 每个 stage 卡片顶部给一个**图标徽章**（`size-10 rounded-full bg-primary-muted text-primary-muted-foreground` 包住 lucide 图标），标题—说明—主按钮形成清晰视觉动线；文本链接一律升级为 `Button variant="outline"` 并右接 `ExternalLink` 图标。设备码框套用 F2 的 `Callout`/圆角面板 + F14 复制按钮，做成该步的"英雄区"。
**附（页面壳层）：** Onboarding 路由绕过 AppLayout 直渲染（[+layout.svelte:43-44](src/routes/app/+layout.svelte#L43)），而 [OnboardingPage.svelte:397](src/modules/onboarding/pages/OnboardingPage.svelte#L397) 根节点只有 `mx-auto max-w-2xl`，**无任何垂直内边距/居中**——内容顶着窗口上边缘。给独立壳补 `min-h-screen px-4 py-10 sm:py-16`（内容纵向留白 + 小窗兜底），第一屏立刻"稳"下来。

#### F27｜Sync 空态语义不分、指标卡是"死"的

**现状 A：** `visibleEntries.length === 0` 时统一渲染同一个 EmptyState（[SyncPreviewPage.svelte:411-416](src/modules/sync/pages/SyncPreviewPage.svelte#L411)），文案固定为"没有匹配的同步条目/调整搜索或状态筛选后再试"。但**触发原因有两种**：真·全部同步完成（值得庆祝的状态）vs 搜索/筛选过滤掉了所有条目（需要引导清除条件）。当前把前者也说成"没匹配、去调筛选"，语义错位。
**现状 B：** 顶部四张指标卡（发现/待上传/待下载/冲突）纯展示。用户看到"冲突 2"的第一反应是点它——点不动。
**方案：**

- 空态分叉：`planData.entries.length === 0` → "全部已同步 🎉"（success 图标，无动作按钮）；`entries.length > 0 && visibleEntries.length === 0` → 现文案 + **"清除筛选"按钮**（一键重置 `search`/`statusFilter`）。
- 指标卡可点：点击"冲突"卡 → `statusFilter = 'conflict'`；点击"待上传/待下载"同理映射。卡片加 `cursor-pointer hover:border-border-strong transition-colors` + `aria-pressed` 表达当前激活的筛选，与下拉双向同步。信息即导航，密度不变、效率翻倍。

---

### D5 · 动效与微交互

#### F18｜全站几乎零过渡（最大短板）

**现状：** 除 Dialog 走 bits-ui 的 `data-open/closed:animate-*`（[dialog-content.svelte:31](src/shared/ui/dialog/dialog-content.svelte#L31)）外，**全站没有任何 Svelte 过渡**。Onboarding 的 stage 硬切、横幅瞬现瞬灭、卡片列表筛选时闪烁、指标数字突变。
**问题：** "硬切"是廉价感的最大来源之一；连续性缺失让状态变化显得突兀。
**方案：** 用 Svelte 内置过渡做**克制**的动效（不炫技，只补连续性）：

```svelte
<script>
  import { fade, fly } from 'svelte/transition'
</script>

<!-- 横幅/Callout 进出 -->
{#if planNotice}
  <div transition:fly={{ y: -6, duration: 150 }}><Callout tone="warning">{planNotice}</Callout></div>
{/if}

<!-- Onboarding stage 切换 -->
{#key stage}
  <div in:fade={{ duration: 120 }}>…当前 stage 卡片…</div>
{/key}

<!-- 列表项进入（配合 animate:flip 处理重排） -->
{#each visibleEntries as entry (entry.action_id)}
  <div in:fade={{ duration: 100 }} animate:flip={{ duration: 150 }}>
    <SyncSkillCard … />
  </div>
{/each}
```

统一时长语汇：**微交互 100–150ms、进出 150ms、hover `transition-colors/shadow` 已有则保留**。务必加 `prefers-reduced-motion` 兜底（见 F22）。

#### F19｜可交互元素缺"被点"反馈

**现状：** 按钮只有 `hover:bg-*`，无 `active:` 态；卡片可选/可展开但无 hover 抬升。
**方案：** 按钮加 `active:scale-[0.98] transition-transform`（或 `active:brightness-95`）；可点卡片加 `hover:border-border-strong hover:shadow-md`（呼应 F5），选中态用 `ring-2 ring-primary` 明确化——目前 SyncSkillCard 选中仅靠 Checkbox，卡片本身无选中视觉，建议选中时 `border-primary bg-primary-muted/30`。

---

### D6 · 可访问性与边界状态

#### F20｜焦点环不一致 / 部分控件无焦点态

**现状：** Button/Input/Checkbox 用 `focus-visible:ring-ring/40`，但 Onboarding 分支 `<select>`（[:489](src/modules/onboarding/pages/OnboardingPage.svelte#L489)）**完全没有焦点样式**；Badge 用的是 `focus:ring`（非 `focus-visible`）。
**方案：** 不必新造工具类——项目已有事实标准 `focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40`（Button/Input/Textarea 均如此）。把它**钦定为唯一焦点写法**并修掉偏差点：

- Onboarding `<select>`：随 F6 换 `Select` 组件后自然消除（过渡期先补上这组类）；
- Badge：`focus:ring` → `focus-visible:ring-2 focus-visible:ring-ring/40`；
- Checkbox 当前是 `focus-visible:ring-1 ring-ring`（无透明度），统一到 `ring-2 ring-ring/40`。

一致的焦点环宽度/颜色是键盘用户感知"这是同一个产品"的最直接信号。

#### F21｜EmptyState 过于朴素

**现状：** [empty-state.svelte:16-27](src/shared/ui/empty-state/empty-state.svelte#L16) 只有一个 `text-muted-foreground` 的裸图标 + 标题 + 描述。
**方案：** 图标套一个柔和容器，提升"空态也是被设计过的"观感：

```svelte
<div class="mx-auto flex size-14 items-center justify-center rounded-full bg-surface-muted text-muted-foreground">
  {@render icon()}
</div>
```

并把外层 `p-8` 提到 `p-10`、标题 `text-base font-semibold`，与新 type scale 对齐。

#### F22｜杂项打磨

- **品牌 logo `alt=""`**（[AppLayout.svelte:43](src/app/layouts/AppLayout.svelte#L43)）：装饰性可接受，但更稳妥是 `alt={t('layout.brandTitle')}`。
- **语言/主题切换**（SettingsPage 分段按钮）视觉不错，但可抽成通用 `SegmentedControl`，供主题/语言/（未来）筛选复用，减少三处重复的手写高亮逻辑（[SettingsPage.svelte:156-193](src/modules/settings/pages/SettingsPage.svelte#L156)）。
- **`prefers-reduced-motion` 兜底**：在 index.css 加全局降级，尊重系统偏好（配合 F18）：
  ```css
  @media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
      animation-duration: 0.01ms !important;
      transition-duration: 0.01ms !important;
    }
  }
  ```

#### F25｜对比度实测（结论：达标，列为基线保护项）

**做法：** 对高频前景/背景 token 对用 WCAG 2.1 相对亮度公式核算对比度（`(L1+0.05)/(L2+0.05)`）。这是本评审唯一"无需主观"的维度，故给硬数据。

| 前景 → 背景                                          | 比值 | 等级 |
| ---------------------------------------------------- | ---- | ---- |
| muted-fg `#5b6471` → surface `#fff`（浅）            | 5.99 | AA   |
| muted-fg → background `#f6f7f9`（浅）                | 5.59 | AA   |
| muted-fg → surface-muted `#f4f5f7`（浅）             | 5.49 | AA   |
| warning `#b45309` → warning-muted（浅）              | 4.71 | AA   |
| success `#15803d` → success-muted（浅）              | 4.79 | AA   |
| primary link `#0f766e` → 白                          | 5.47 | AA   |
| primary-muted-fg `#115e59` → primary-muted（浅徽标） | 7.21 | AAA  |
| muted-fg `#8b949e` → surface `#161b22`（暗）         | 5.62 | AA   |
| warning `#f59e0b` → warning-muted（暗）              | 7.53 | AAA  |

**结论：** 9 组全部 ≥4.5:1（正文 AA），无失败。**方案是"守成"而非"整改"**：把这些配对写进 `lint:colors` 的期望基线或一条注释约定，避免后续调色时把某个 `-muted` 调亮导致回归。改任何 muted/foreground 值后按上表复算。

#### F26｜切换语言不同步 `<html lang>`、缺 `color-scheme`

**现状 A：** [app.html](src/app.html) 硬编码 `<html lang="zh-CN">`，而 `languageState.setLanguage` 只切 i18next、不改 DOM 的 `lang`（[language.svelte.ts](src/shared/state/language.svelte.ts)）。切到英文后，`<html>` 仍报告 `zh-CN`。
**现状 B：** 全局未设置 CSS `color-scheme`。暗色仅靠 `.dark` 类切换 token，但**原生控件**（`<select>` 下拉面板、`<input type=number>` 旋钮、滚动条、日期/文件选择器）不会跟随，暗色下会闪出浅色系统 UI。
**问题：** A 影响读屏发音与 `:lang()` 规则；B 是暗色模式"边角发白"的根因，与 F6/F23 的原生控件问题叠加。
**方案：**

```ts
// setLanguage 内追加（DOM 与 i18next 同步）
document.documentElement.lang = language
```

```css
/* index.css：让原生控件跟随明暗 */
:root {
  color-scheme: light;
}
.dark {
  color-scheme: dark;
}
```

> B 落地后，即便 F6/F23 尚未替换原生控件，暗色下的系统 UI 也会先变协调——性价比极高的一行。

---

## 四、需要新增/沉淀的设计系统资产

**Token（[src/index.css](src/index.css)）**

- `--success-border` / `--info-border`（F0，补齐 tone 对称，Callout 前置）
- 圆角三档使用约定：容器 `xl` / 控件与面板 `md` / 药丸 `full`（F1，用 Tailwind 内置 scale，不新增 token）
- `--font-mono` + 修正正文字体栈顺序（F4）
- `color-scheme: light/dark`（F26，原生控件跟随明暗）
- `prefers-reduced-motion` 全局降级、`focus-ring` 工具类（F20/F22）

**新增组件（`src/shared/ui/*`，优先 `npx shadcn-svelte@latest add`）**

| 组件                              | 用途               | 解决      |
| --------------------------------- | ------------------ | --------- |
| `Select`                          | 分支选择、状态筛选 | F6、F10   |
| `Progress` 或 `Stepper`           | Onboarding 进度    | F7        |
| `Toast`（sonner）                 | 瞬时反馈           | F11、F14  |
| `Skeleton`                        | 加载占位           | F12       |
| `AlertDialog`                     | 危险操作确认       | F9        |
| `Callout`（手搓，无 shadcn 等价） | 统一提示条         | F2        |
| `SegmentedControl`（手搓）        | 主题/语言/筛选分段 | F22       |
| `NumberInput`（手搓，可选）       | limits 数值步进    | F23 方案② |

> 手搓组件（Callout / SegmentedControl / Stepper / NumberInput）符合 CLAUDE.md 第 11 条"无 shadcn 等价时才手建"的约束，与既有 Spinner/EmptyState/StatusBadge 同级别。

**需新增的 i18n key（`src/shared/i18n/locales/{zh-CN,en-US}.json`，实现时先补 key 再写 UI，否则 `lint:i18n` 会拦）**

| key                                                      | 用途                                                             | 来源 |
| -------------------------------------------------------- | ---------------------------------------------------------------- | ---- |
| `common.actions.close`                                   | Dialog 关闭按钮 / sr-only 文案（现有 `common.actions` 无 close） | F24  |
| `common.actions.copy`                                    | 设备码复制按钮                                                   | F14  |
| `sync.selectedCount`                                     | sticky 操作条"已选 N 项"（带 `{{count}}` 插值）                  | F13  |
| `sync.emptyAllSynced` / `sync.emptyAllSyncedDescription` | 真·全部同步的空态                                                | F27  |
| `sync.clearFilters`                                      | 空态"清除筛选"按钮                                               | F27  |

> 现有 `sync.filters.*`、`sync.empty*`、`settings.pathCopied` 等已具备，直接复用；上表之外若实现中出现新文案，一律同批补进两份 locale。

---

## 五、实施路线图（按性价比排序）

**P0 — 观感事故 & 一致性 & 零成本高收益（约半天，收益最大）**

1. F10 修复筛选下拉显示原始 key（一行 `t()`）
2. F26 加 `color-scheme` + 同步 `<html lang>`（两行，先让暗色原生控件不发白）
3. F0 补 `--success-border`/`--info-border` → F2 抽 `Callout` 替换 4+ 处横幅
4. F1 radius token + 全站补圆角；F23 隐藏 number 旋钮
5. F8 换掉原生 checkbox；F3 页面主标题 `font-bold → font-semibold`；F15 去除 Sync 页重复标题

→ 预计 D1/D2/D4 明显回升。

**P1 — 交互反馈与关键控件（约 1–1.5 天）**

6. F11 引入 Toast，迁移瞬时反馈；F14 设备码复制
7. F6 `Select` 组件替换两处原生 select；F9 `AlertDialog` 替换 `window.confirm`；F24 本地化 Dialog 的 Close
8. F7 Onboarding 进度升级 Stepper；F13 Sync sticky 操作条
9. F4 修正字体栈 + 技术字符串 `font-mono`

→ 预计 D2/D3 达标。

**P2 — 打磨与动效（约 1 天）**

10. F18/F19 全站过渡与 active/hover 微交互 + `prefers-reduced-motion`
11. F12 Skeleton；F16 Settings 定义列表；F17 Onboarding hero 化 + 独立壳纵向留白
12. F27 空态语义分叉 + 指标卡可点筛选；F21 EmptyState；F5 阴影层级；F20 焦点统一；F22 杂项
13. F25 把实测对比配对写入 `lint:colors` 基线注释，防回归

→ 预计 D5/D6 达标。

---

## 六、改造后评分预测

| 维度             | 现状   | 改造后       | 关键提升                                                   |
| ---------------- | ------ | ------------ | ---------------------------------------------------------- |
| D1 视觉基础      | 16/22  | **20/22**    | tone token 补齐、圆角/mono/字重阶梯统一、面板收口          |
| D2 设计系统      | 11/18  | **17/18**    | 消灭全部裸原生控件、Select/AlertDialog/Callout 沉淀        |
| D3 交互反馈      | 11/20  | **18/20**    | Toast + Skeleton + sticky 主操作 + 复制                    |
| D4 布局/IA       | 10/15  | **14/15**    | 标题去重、统一页头、空态分叉、指标卡即筛选                 |
| D5 动效          | 3/12   | **11/12**    | 克制过渡贯穿状态切换/列表/横幅                             |
| D6 可访问性/状态 | 9/13   | **12/13**    | 焦点统一、lang/color-scheme 同步、reduced-motion、对比基线 |
| **合计**         | **60** | **92 / 100** | ✅ 达标（≥90）                                             |

> 说明：满分未拉满是刻意的——`92` 对应"成熟、精致、自洽"的桌面应用水准；剩余 8 分属于品牌化插画、深色/浅色双主题的逐像素微调、以及真实用户可用性测试驱动的迭代，超出本轮重构范围。

---

## 七、验收清单

**每次 UI 改动后（CLAUDE.md 第 10 条）：**

```bash
npm run lint:responsive   # 无任意尺寸/溢出隐患
npm run lint:colors       # 只用语义 token（radius/mono token 也需在 index.css 定义）
npm run lint:i18n         # 新增文案进 locales，.ts/.svelte 无中文
npm run format:check
npm run lint && npm run build
```

**逐屏视觉 QA（浅色 + 深色各一遍）：**

- [ ] Onboarding：Stepper 高亮正确；每步有图标徽章；设备码等宽 + 可复制；链接为 outline 按钮；stage 切换有淡入；独立壳有纵向留白（内容不顶窗口边）
- [ ] Sync：筛选下拉显示中文标签（非 key）；加载走 Skeleton；卡片 hover 抬升 + 选中态清晰；sticky 操作条常驻可点；结果走 Toast；"全部同步"与"筛选无结果"空态文案不同，后者有清除筛选按钮；指标卡点击可切筛选
- [ ] Settings：断开走 AlertDialog（非系统弹窗）；GitHub 信息为定义列表 + 分隔线 + 等宽值；复制路径走 Toast；数字输入无原生旋钮（或旋钮已设计系统化）
- [ ] 全局：无重复 h1；无直角内联面板；所有可交互元素 `focus-visible` 一致（ring-2 ring-ring/40）；`prefers-reduced-motion` 下动效降级
- [ ] 无障碍抽查：切换语言后 `document.documentElement.lang` 同步变化；暗色下原生下拉/滚动条随 `color-scheme` 变深；Dialog 关闭按钮读屏文案为中文；success/info 横幅有可见边框（F0 token 生效）
- [ ] 圆角/字重/阴影三者在同屏内自洽（卡片/对话框 xl、控件与内嵌面板 md、徽标 full；标题 semibold、数字 bold）

---

### 附：一句话结论

> 当前 UI 的**地基（token 体系、布局、暗色模式、i18n）已达优秀水准（这是最难的部分）**，短板集中在"精装修"：**统一圆角/字重/等宽 → 消灭裸原生控件 → 补 Toast/Skeleton/优雅确认 → 加克制动效**。以上四步（P0→P2）落地即可把体验从"能用且工整"推到"精致且顺手"的 90+ 区间。
