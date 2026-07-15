# 首次打开引导弹窗（介绍 GitHub Vault 概念）

## 背景

首次打开应用时，`/` 根页面根据 `isWorkspaceReady` 把用户直接重定向到 `/app/onboarding`。OnboardingPage 一上来就是 GitHub Device Flow 授权 → 安装 GitHub App → 选仓库/分支 → 初始化 Vault → 绑定的多步流程，没有任何前置说明"本软件通过新建一个 GitHub 仓库作为 skill 云端存储"。用户一进来就被要求装 GitHub App，产生困惑。

目标：首次打开时弹出一个引导弹窗，用 GitHub 图标作为视觉元素，讲清楚"用 GitHub 仓库作为 skill 云端 Vault"这一核心概念，之后再进入既有 onboarding 流程。仅首次打开出现一次。

## 关键事实（已核实）

- 首次打开必经 `/app/onboarding`（workspace 未就绪时根页面重定向到此）。引导弹窗挂在这里即可覆盖所有"首次打开"场景；reconfigure 用户此前已打开过应用，会被持久化标记跳过。
- 持久化模式：`window.localStorage` + `skill-sync.*` 键（见 `theme.svelte.ts`、`ui.svelte.ts`、`language.svelte.ts`）。新增 `skill-sync.intro-seen`。
- Dialog 组件已存在于 `@/shared/ui`：`<Dialog bind:open><DialogContent>...</DialogContent></Dialog>`，默认 `sm:max-w-sm`，可传 `class` 覆盖。`DialogContent` 默认带关闭按钮。
- `@lucide/svelte@1.24.0` **没有** github 品牌图标（仅 git-branch/fork/commit 等通用图标）。按规则"无 shadcn 等价物时手写"，内联官方 GitHub Octocat mark SVG，用 `fill="currentColor"` + 语义色 token，不硬编码颜色。
- 颜色 token：`--foreground`/`--background`/`--primary`/`--primary-foreground`/`--primary-muted`/`--primary-muted-foreground`/`--strong-foreground`/`--muted-foreground` 均可用。
- `i18next.d.ts` 用 `zh-CN.json` 作为 `t()` 类型来源，键严格校验 → 三步说明必须用**字面量** `t('...')` key，不能用模板字符串动态拼接。zh-CN.json 与 en-US.json 必须结构一致（lint:i18n 校验对等）。
- `.svelte`/`.ts` 文件禁止中文字符（含注释）——i18n lint 强制，优先级高于 CLAUDE.md 规则1。组件内注释用英文；中文文案只进 locale JSON。
- en-US.json / zh-CN.json 当前有未提交改动，编辑时只做定点插入，不动既有内容。

## 改动清单

### 1. 新建 `src/modules/onboarding/components/FirstRunIntroDialog.svelte`

自包含组件：自己管理 `open` 状态 + localStorage 标记，父组件只需 `<FirstRunIntroDialog />`。

逻辑：

- `let open = $state(false)`、`let shown = $state(false)`
- `onMount`：若 `localStorage.getItem('skill-sync.intro-seen') !== 'true'`，则 `open = true`
- `$effect`：`open` 为 true 时记 `shown = true`；当 `open` 变 false 且 `shown` 为 true 时写入 `localStorage.setItem('skill-sync.intro-seen', 'true')`。`shown` 守卫确保初始关闭态不会提前写标记。
- "开始配置"按钮 + X/ESC/遮罩关闭都走 `bind:open`，任一关闭路径都会触发上面的 `$effect` 写标记。

结构（用语义色 token，标准尺寸类，无任意值）：

- `DialogContent class="sm:max-w-md"`（覆盖默认 `sm:max-w-sm`，给说明文字更多宽度）
- `DialogHeader` 内一个 `flex items-center gap-3` 行：
  - 左侧徽标 `size-11 rounded-full bg-foreground text-background`，内含 GitHub Octocat SVG（`size-6`，`fill="currentColor"`），`aria-hidden="true"` + `sr-only` 文案 `t('onboarding.intro.githubMark')`。`bg-foreground/text-background` 在亮/暗主题下自适应反转，呈 GitHub 风格徽标。
  - 右侧 `grid gap-1`：`DialogTitle`（标题）+ `DialogDescription`（一句话总述）
- 正文 `grid gap-4`：
  - Vault 概念块：`font-medium text-strong-foreground` 小标题 + `text-muted-foreground` 说明（点明"新建一个 GitHub 仓库作为私有 skill 云端 Vault"）
  - 私有仓库提示：`text-muted-foreground`
  - "接下来会做什么"：`font-medium text-strong-foreground` 小标题 + `<ol>` 三条，每条用 `size-5 rounded-full bg-primary-muted text-primary-muted-foreground` 数字徽标（与 OnboardingStepper 当前步骤样式一致），文案用**字面量** `t('onboarding.intro.nextSteps.authorize' | 'selectRepository' | 'bind')`
- `DialogFooter`：主按钮 `class="w-full sm:w-auto"`，文案 `t('onboarding.intro.continue')`，点击置 `open = false`

GitHub Octocat 官方 16×16 path（公有领域/官方 mark）：

```
M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z
```

### 2. `src/modules/onboarding/pages/OnboardingPage.svelte`

- 顶部 `<script>` 加 `import FirstRunIntroDialog from '../components/FirstRunIntroDialog.svelte'`（与既有相对路径导入风格一致，不导出 index）
- 在根 `<div class="mx-auto grid ...">` 内第一个子元素位置渲染 `<FirstRunIntroDialog />`（Dialog 走 portal，视觉位置不受影响）

### 3. i18n：在 `onboarding` 对象下新增 `intro` 子对象

**`src/shared/i18n/locales/en-US.json`**（在 `onboarding` 内、`title` 之前插入）：

```json
"intro": {
  "title": "Welcome to Skill Sync",
  "description": "Skill Sync keeps your agent skills consistent across every machine by syncing them through a GitHub repository.",
  "vaultTitle": "Your skill cloud vault",
  "vaultDescription": "You will create a GitHub repository to act as your private skill vault. Skill Sync uploads, downloads, and reconciles skills through it.",
  "privateHint": "A private repository is recommended so your skills stay visible only to you.",
  "nextStepsTitle": "What happens next",
  "nextSteps": {
    "authorize": "Authorize Skill Sync with GitHub",
    "selectRepository": "Create or choose a vault repository",
    "bind": "Bind the vault and start syncing"
  },
  "continue": "Get started",
  "githubMark": "GitHub"
},
```

**`src/shared/i18n/locales/zh-CN.json`**（同结构，`onboarding` 内、`title` 之前插入）：

```json
"intro": {
  "title": "欢迎使用 Skill Sync",
  "description": "Skill Sync 通过一个 GitHub 仓库，让你每台机器上的 agent skills 保持一致。",
  "vaultTitle": "你的 skill 云端仓库",
  "vaultDescription": "你需要新建一个 GitHub 仓库作为私有的 skill 云端仓库，Skill Sync 会通过它上传、下载并同步 skills。",
  "privateHint": "推荐使用私有仓库，确保 skills 仅你自己可见。",
  "nextStepsTitle": "接下来会做什么",
  "nextSteps": {
    "authorize": "使用 GitHub 授权 Skill Sync",
    "selectRepository": "新建或选择一个 vault 仓库",
    "bind": "绑定 vault 并开始同步"
  },
  "continue": "开始配置",
  "githubMark": "GitHub"
},
```

## 验证

- `npm run lint:i18n`（新增文案 + 键对等）
- `npm run lint:colors`（仅用语义色 token）
- `npm run lint:responsive`（标准尺寸类，无任意值）
- `npm run typecheck`（`t()` 字面量 key 通过类型校验）
- `npm run format:check`（必要时 `npm run format`）
- `npm run lint` + `npm run build`（完成前必跑）

## 不做的事

- 不改 onboarding 既有流程/阶段，不动 AppLayout/app shell 路由分支。
- 不把 FirstRunIntroDialog 提升到 shared（仅 onboarding 内使用，未达 3+ 处复用门槛）。
- 不做"再次查看"入口、不做复选框——首次一次即持久化，简单为主。Onboarding 卡片已有的 `description` 文案作为关闭后的兜底上下文。
- 不硬编码任何颜色值；GitHub mark 用 `currentColor` + token。
