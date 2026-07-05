# Skill Sync Svelte UI Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development if subagents are available, or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 React 前端 UI 重构为 Svelte 5 / SvelteKit SPA，同时保留现有 Tauri 2 后端、模块边界、TypeScript 严格检查、ESLint 架构规则、自定义 npm 检查脚本和视觉规范。

**Architecture:** 保持现有 `app -> modules -> shared` 的产品分层，新增 SvelteKit 必需的 `src/routes` 作为最外层装配层。迁移顺序优先保护质量边界：先改工具链与 lint 覆盖，再迁共享 UI 和状态，再逐页替换 React 页面，最后移除 React 依赖。

**Tech Stack:** Tauri 2, Svelte 5, SvelteKit static SPA, TypeScript strict, Vite, Tailwind CSS v4, i18next, Zod, @tanstack/svelte-query, lucide-svelte.

---

## 1. 结论

对 `D:\project\skill-sync` 这种规格的 Tauri 桌面工具，推荐迁移到：

```txt
Tauri 2
+ Svelte 5
+ SvelteKit SPA mode
+ TypeScript strict
+ Tailwind CSS v4
+ @tanstack/svelte-query
+ lucide-svelte
+ i18next
+ Zod
```

选择 SvelteKit SPA，而不是纯 Svelte + 手写路由，原因是当前项目已经有清晰的多页面结构：

```txt
/app/dashboard
/app/skills
/app/sync
/app/conflicts
/app/backups
/app/settings
/app/onboarding
```

SvelteKit 可以自然承接这些页面、嵌套路由和统一 layout。由于 Tauri 不需要 SSR，必须配置为静态 SPA：

```ts
// src/routes/+layout.ts
export const ssr = false
export const prerender = false
```

```js
// svelte.config.js
import adapter from '@sveltejs/adapter-static'

const config = {
  kit: {
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
    }),
  },
}

export default config
```

如果以后做的是更小的新项目，只有 1-3 个视图，可以用 `Svelte 5 + Vite`。但对当前这个多页面工具，SvelteKit SPA 更划算。

---

## 2. 必须保留的边界

这次重构不是换框架时顺手“清空规则”。以下边界必须保留，迁移后命令名和语义都要继续存在。

### 2.1 npm 脚本边界

必须保留这些脚本：

```json
{
  "dev": "vite",
  "build": "npm run typecheck && vite build",
  "format": "prettier --ignore-unknown --write .",
  "format:check": "prettier --ignore-unknown --check .",
  "lint": "npm run typecheck && eslint . && npm run lint:responsive && npm run lint:colors && npm run lint:i18n",
  "lint:colors": "node scripts/check-color-tokens.mjs",
  "lint:i18n": "node scripts/check-i18n-literals.mjs",
  "lint:responsive": "node scripts/check-responsive-layout.mjs",
  "precommit": "lint-staged && npm run lint",
  "preview": "vite preview",
  "tauri": "tauri"
}
```

迁移后 `typecheck` 需要从 `tsc -b` 改为能检查 `.svelte` 文件的命令：

```json
{
  "typecheck": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json && tsc -p tsconfig.node.json --noEmit"
}
```

如果最终选择纯 Svelte + Vite，不用 SvelteKit，则改为：

```json
{
  "typecheck": "svelte-check --tsconfig ./tsconfig.app.json && tsc -p tsconfig.node.json --noEmit"
}
```

### 2.2 ESLint 架构边界

现有 `eslint.config.js` 的核心价值必须保留：

```txt
app/routes -> modules -> shared
禁止 lower layer import higher layer
禁止跨 module 深导入
禁止 shared 深导入
禁止 import cycle
禁止 self import
禁止 export *
```

迁移后需要把规则覆盖范围从：

```js
files: ['**/*.{ts,tsx}']
```

扩展为：

```js
files: ['**/*.{ts,svelte}']
```

并移除 React 专属规则：

```txt
eslint-plugin-react-hooks
eslint-plugin-react-refresh
```

新增 Svelte 规则：

```txt
eslint-plugin-svelte
svelte-eslint-parser
```

### 2.3 自定义检查脚本边界

这三个脚本必须继续作为质量门禁：

```txt
scripts/check-responsive-layout.mjs
scripts/check-color-tokens.mjs
scripts/check-i18n-literals.mjs
```

迁移后必须让它们扫描 `.svelte`：

```js
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.tsx', '.svelte'])
```

响应式脚本当前只对 `.tsx` 检查 `<table>`，需要改成：

```js
if (filePath.endsWith('.tsx') || filePath.endsWith('.svelte')) {
  checkTables(filePath, content, issues)
}
```

迁移完成后项目里不应再有 `.tsx`，但这个兼容写法方便分阶段迁移。

SvelteKit 的 `adapter-static` 默认输出目录通常不是当前 Tauri 配置使用的 `dist`。为了保留 `src-tauri/tauri.conf.json` 中的 `frontendDist: "../dist"`，本方案要求在 `svelte.config.js` 里显式设置：

```js
adapter({
  pages: 'dist',
  assets: 'dist',
  fallback: 'index.html',
})
```

### 2.4 UI 视觉边界

继续遵守：

```txt
颜色只能用 src/index.css 中的语义 token
组件里不能直接写 bg-slate-100 / text-red-500 / #fff / rgb(...)
表格必须有 overflow-x-auto + local min-w-[...]
布局必须 mobile-first
中文 UI 文案必须进 src/shared/i18n/locales
不要新增营销式 landing page
不要把页面 section 都做成浮动 card
```

---

## 3. 现有项目盘点

### 3.1 当前 React 依赖

当前前端关键依赖：

```txt
react
react-dom
react-router-dom
@tanstack/react-query
react-i18next
lucide-react
zustand
zod
tailwindcss
@tailwindcss/vite
@tauri-apps/api
@tauri-apps/plugin-dialog
```

迁移目标：

```txt
react                  -> remove
react-dom              -> remove
react-router-dom       -> SvelteKit file routing
@tanstack/react-query  -> @tanstack/svelte-query
react-i18next          -> remove; keep i18next core
lucide-react           -> lucide-svelte
zustand                -> Svelte 5 runes / .svelte.ts state
```

保留：

```txt
@tauri-apps/api
@tauri-apps/plugin-dialog
i18next
zod
clsx
tailwind-merge
tailwindcss
@tailwindcss/vite
typescript
vite
```

### 3.2 当前页面

页面迁移映射：

```txt
src/modules/dashboard/pages/DashboardPage.tsx
  -> src/modules/dashboard/pages/DashboardPage.svelte

src/modules/skills/pages/SkillsPage.tsx
  -> src/modules/skills/pages/SkillsPage.svelte

src/modules/sync/pages/SyncPreviewPage.tsx
  -> src/modules/sync/pages/SyncPreviewPage.svelte

src/modules/conflicts/pages/ConflictsPage.tsx
  -> src/modules/conflicts/pages/ConflictsPage.svelte

src/modules/backups/pages/BackupsPage.tsx
  -> src/modules/backups/pages/BackupsPage.svelte

src/modules/settings/pages/SettingsPage.tsx
  -> src/modules/settings/pages/SettingsPage.svelte

src/modules/onboarding/pages/OnboardingPage.tsx
  -> src/modules/onboarding/pages/OnboardingPage.svelte
```

### 3.3 当前共享 UI

共享 UI 迁移映射：

```txt
src/shared/ui/button.tsx       -> src/shared/ui/Button.svelte
src/shared/ui/card.tsx         -> src/shared/ui/Card.svelte, CardHeader.svelte, CardBody.svelte
src/shared/ui/input.tsx        -> src/shared/ui/Input.svelte
src/shared/ui/textarea.tsx     -> src/shared/ui/Textarea.svelte
src/shared/ui/checkbox.tsx     -> src/shared/ui/Checkbox.svelte
src/shared/ui/badge.tsx        -> src/shared/ui/Badge.svelte
src/shared/ui/status-badge.tsx -> src/shared/ui/StatusBadge.svelte
src/shared/ui/spinner.tsx      -> src/shared/ui/Spinner.svelte
src/shared/ui/empty-state.tsx  -> src/shared/ui/EmptyState.svelte
src/shared/ui/path-picker.tsx  -> src/shared/ui/PathPicker.svelte
src/shared/ui/breadcrumb.tsx   -> src/shared/ui/Breadcrumb*.svelte 或并入 AppLayout
```

建议先手工迁移当前薄 UI 原语，不要第一步引入整套 shadcn-svelte。当前组件足够简单，直接迁移更稳。后续如果需要 Dialog、Popover、Command、Select 等复杂交互，再引入 shadcn-svelte / Bits UI。

### 3.4 当前状态

当前 Zustand 状态：

```txt
src/shared/stores/themeStore.ts
src/shared/stores/uiStore.ts
src/shared/stores/languageStore.ts
src/modules/sync/stores/syncDecisionsStore.ts
```

迁移目标：

```txt
src/shared/state/theme.svelte.ts
src/shared/state/ui.svelte.ts
src/shared/state/language.svelte.ts
src/modules/sync/state/syncDecisions.svelte.ts
```

原则：

```txt
本地 UI 状态用 Svelte 5 runes
服务端/Tauri 命令数据继续用 @tanstack/svelte-query
schema 校验继续用 Zod
不要照搬 Zustand 式 thin wrapper
```

---

## 4. 目标目录结构

推荐结构：

```txt
src/
  app/
    layouts/
      AppLayout.svelte
    router/
      routeConfig.ts
    providers/
      QueryProvider.svelte
      ThemeSync.svelte

  routes/
    +layout.svelte
    +layout.ts
    +page.svelte
    app/
      +layout.svelte
      +page.svelte
      dashboard/
        +page.svelte
      skills/
        +page.svelte
      sync/
        +page.svelte
      conflicts/
        +page.svelte
      backups/
        +page.svelte
      settings/
        +page.svelte
      onboarding/
        +page.svelte

  shared/
    i18n/
      index.ts
      language.ts
      locales/
    lib/
      tauri.ts
      utils.ts
    schemas/
    state/
      theme.svelte.ts
      ui.svelte.ts
      language.svelte.ts
    theme/
      ThemeSync.svelte
    ui/
      Button.svelte
      Card.svelte
      CardBody.svelte
      CardHeader.svelte
      ...

  modules/
    dashboard/
      index.ts
      pages/
        DashboardPage.svelte
    skills/
      index.ts
      api/
      schemas/
      pages/
        SkillsPage.svelte
    sync/
      index.ts
      api/
      schemas/
      state/
        syncDecisions.svelte.ts
      pages/
        SyncPreviewPage.svelte
```

`src/routes` 只做路由装配，不承载业务逻辑。每个 `+page.svelte` 应该非常薄，例如：

```svelte
<script lang="ts">
  import { DashboardPage } from '@/modules/dashboard'
</script>

<DashboardPage />
```

这样可以保留现有模块边界，并继续禁止跨模块深导入。

---

## 5. 关键代码设计

### 5.1 routeConfig

React 版 `routeConfig.tsx` 里存了 `element`。Svelte 版不应该存组件实例，只保留导航元数据：

```ts
import {
  FolderSync,
  LayoutDashboard,
  ListChecks,
  Package,
  RotateCcw,
  Settings as SettingsIcon,
  Sparkles,
} from 'lucide-svelte'
import type { Component } from 'svelte'

export type RouteTitleKey =
  | 'routes.backups'
  | 'routes.conflicts'
  | 'routes.dashboard'
  | 'routes.onboarding'
  | 'routes.settings'
  | 'routes.skills'
  | 'routes.sync'

export type RouteGroupKey = 'routeGroups.main'

export interface AppRouteConfig {
  path: string
  title: RouteTitleKey
  group: RouteGroupKey
  icon: Component<{ class?: string }>
}

export const appRoutes: AppRouteConfig[] = [
  {
    path: '/app/dashboard',
    title: 'routes.dashboard',
    group: 'routeGroups.main',
    icon: LayoutDashboard,
  },
  // 其余路由同理
]

export const DEFAULT_ROUTE_PATH = '/app/dashboard'
```

### 5.2 AppLayout

`AppLayout.svelte` 替代 React Router 的 `Outlet`、`NavLink`、`useLocation`：

```svelte
<script lang="ts">
  import { page } from '$app/state'
  import { PanelLeftClose, PanelLeftOpen } from 'lucide-svelte'
  import { appRoutes } from '@/app/router/routeConfig'
  import { t } from '@/shared/i18n'
  import { uiState } from '@/shared/state/ui.svelte'
  import { cn } from '@/shared/lib'

  let { children } = $props()

  const currentRoute = $derived(
    appRoutes.find((route) => route.path === page.url.pathname) ?? appRoutes[0],
  )
</script>

<!-- 保留当前 grid/sidebar/header/main 结构和 Tailwind token -->
```

导航链接使用普通 `<a href={route.path}>` 即可。激活状态通过 `page.url.pathname === route.path` 计算，不需要引入额外路由库。

### 5.3 QueryProvider

保持当前 TanStack Query 默认策略：

```svelte
<script lang="ts">
  import {
    QueryClient,
    QueryClientProvider,
  } from '@tanstack/svelte-query'

  let { children } = $props()

  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        retry: 1,
        staleTime: 30_000,
      },
    },
  })
</script>

<QueryClientProvider client={queryClient}>
  {@render children?.()}
</QueryClientProvider>
```

页面内 React Query：

```ts
const state = useQuery({ queryKey: ['app-state'], queryFn: getAppState })
```

迁移为 Svelte Query：

```svelte
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query'
  import { getAppState } from '@/modules/settings'

  const state = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
</script>

{#if state.isPending}
  ...
{:else if state.isError}
  ...
{:else}
  ...
{/if}
```

注意：Svelte Query 的 `createQuery` 参数需要包在函数里，以保持响应式。

### 5.4 主题状态

用 runes 替代 Zustand：

```ts
export type ThemeMode = 'light' | 'dark' | 'system'

const STORAGE_KEY = 'skill-sync.theme'

const readSystemTheme = (): 'light' | 'dark' =>
  window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

const readStoredTheme = (): ThemeMode => {
  const stored = window.localStorage.getItem(STORAGE_KEY)
  return stored === 'light' || stored === 'dark' || stored === 'system'
    ? stored
    : 'system'
}

export const applyTheme = (mode: ThemeMode): void => {
  const effective = mode === 'system' ? readSystemTheme() : mode
  document.documentElement.classList.toggle('dark', effective === 'dark')
}

export const initTheme = (): void => {
  applyTheme(readStoredTheme())
}

class ThemeState {
  theme = $state<ThemeMode>(readStoredTheme())

  setTheme(theme: ThemeMode): void {
    window.localStorage.setItem(STORAGE_KEY, theme)
    applyTheme(theme)
    this.theme = theme
  }
}

export const themeState = new ThemeState()
```

### 5.5 i18n

保留 `i18next`，移除 `react-i18next`。关键是语言变化后 Svelte 组件必须重新渲染。

推荐用 `.svelte.ts` 包一层响应式语言状态：

```ts
import i18next from 'i18next'
import enUS from './locales/en-US.json'
import zhCN from './locales/zh-CN.json'
import {
  DEFAULT_LANGUAGE,
  LANGUAGE_STORAGE_KEY,
  readStoredLanguage,
  type Language,
} from './language'

void i18next.init({
  lng: readStoredLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
  resources: {
    'zh-CN': { translation: zhCN },
    'en-US': { translation: enUS },
  },
})

class LanguageState {
  language = $state<Language>(readStoredLanguage())

  async setLanguage(language: Language): Promise<void> {
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, language)
    await i18next.changeLanguage(language)
    this.language = language
  }
}

export const languageState = new LanguageState()

export const t = (key: string, options?: Record<string, unknown>): string => {
  languageState.language
  return i18next.t(key, options)
}
```

`languageState.language` 这行看起来像无用读取，但它让使用 `t(...)` 的 Svelte 组件跟随语言变化重新计算。不要删。

### 5.6 Tauri IPC

`src/shared/lib/tauri.ts` 基本可以原样保留：

```txt
invokeCmd<T>
SkillSyncError
errorMessage
chooseDirectory
openPath
```

所有 module API 和 Zod schema 可以继续保留 `.ts`，例如：

```txt
modules/settings/api/configApi.ts
modules/sync/api/syncApi.ts
modules/skills/api/scanSkills.ts
modules/backups/api/backupsApi.ts
modules/onboarding/api/onboardingApi.ts
```

后续可以追加 `specta / tauri-specta` 生成 TS 类型，但这不是本次 UI 迁移的必要条件。

---

## 6. ESLint 迁移设计

### Task: 更新 ESLint flat config

**Files:**

- Modify: `eslint.config.js`

- [ ] 移除 React 插件导入：

```js
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
```

- [ ] 新增 Svelte 插件：

```js
import svelte from 'eslint-plugin-svelte'
import svelteParser from 'svelte-eslint-parser'
```

- [ ] 扩展层级：

```js
const LAYER_ORDER = {
  shared: 0,
  modules: 1,
  app: 2,
  routes: 3,
}
```

这样允许：

```txt
routes -> app -> modules -> shared
```

禁止：

```txt
shared -> modules
modules -> app
app -> routes
```

- [ ] 将 TypeScript 文件规则保留给 `.ts`：

```js
{
  files: ['**/*.ts'],
  // js recommended + typescript-eslint + import-x + local rules
}
```

- [ ] 新增 `.svelte` 文件规则：

```js
{
  files: ['**/*.svelte'],
  languageOptions: {
    parser: svelteParser,
    parserOptions: {
      parser: tseslint.parser,
      extraFileExtensions: ['.svelte'],
      sourceType: 'module',
    },
    globals: globals.browser,
  },
  plugins: {
    svelte,
    'import-x': importX,
    local: {
      rules: {
        'no-reverse-layer-imports': noReverseLayerImportsRule,
      },
    },
  },
  rules: {
    ...svelte.configs.recommended.rules,
    'import-x/no-cycle': [
      'error',
      {
        allowUnsafeDynamicCyclicDependency: false,
        ignoreExternal: true,
        maxDepth: Infinity,
      },
    ],
    'import-x/no-self-import': 'error',
    'local/no-reverse-layer-imports': 'error',
    'no-restricted-imports': [
      'error',
      {
        patterns: [
          {
            regex: '^@/modules/[^/]+/.+',
            message:
              'Cross-module imports must use the module root entry, for example "@/modules/demo-dashboard". Use relative imports inside a module.',
          },
          {
            regex: '^@/shared/[^/]+/.+',
            message:
              'Shared imports must use stable subpath entries, for example "@/shared/ui". Do not deep import shared internals.',
          },
        ],
      },
    ],
  },
}
```

- [ ] 保留 shared 例外：

```js
{
  files: ['src/shared/**/*.{ts,svelte}'],
  rules: {
    'no-restricted-imports': 'off',
  },
}
```

### 验收

```bash
npm run lint
```

预期：迁移中可能会先失败，但失败必须来自真实 Svelte 源码问题，不应该是 ESLint 无法解析 `.svelte`。

---

## 7. 自定义检查脚本迁移设计

### Task: 响应式检查覆盖 Svelte

**Files:**

- Modify: `scripts/check-responsive-layout.mjs`

- [ ] 修改扩展名：

```js
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.tsx', '.svelte'])
```

- [ ] 修改 table 检查：

```js
if (filePath.endsWith('.tsx') || filePath.endsWith('.svelte')) {
  checkTables(filePath, content, issues)
}
```

- [ ] 全量迁移完成后，可移除 `.tsx`：

```js
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.svelte'])
```

### Task: 颜色 token 检查覆盖 Svelte

**Files:**

- Modify: `scripts/check-color-tokens.mjs`

- [ ] 修改扩展名：

```js
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.tsx', '.svelte'])
```

全量迁移完成后移除 `.tsx`。

### Task: i18n 中文硬编码检查覆盖 Svelte

**Files:**

- Modify: `scripts/check-i18n-literals.mjs`

- [ ] 修改扩展名：

```js
const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx', '.svelte'])
```

全量迁移完成后移除 `.tsx`。

注意：这个脚本会扫描源码中的所有中文字符，包括 Svelte 注释。如果确实需要中文注释，应先增强脚本忽略注释；否则迁移期间不要在 `src/**/*.svelte` 中新增中文注释。

### 验收

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：脚本能扫描 `.svelte`，并能报告 Svelte 文件里的违规 class / 颜色 / 中文硬编码。

---

## 8. 迁移阶段

## Chunk 1: 工具链和质量门禁

### Task 1: 替换前端框架依赖

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`

- [ ] 安装运行时依赖：

```bash
npm install @tanstack/svelte-query lucide-svelte
```

- [ ] 安装 Svelte/SvelteKit 与开发检查依赖：

```bash
npm install -D svelte @sveltejs/kit @sveltejs/adapter-static @sveltejs/vite-plugin-svelte svelte-check eslint-plugin-svelte svelte-eslint-parser
```

- [ ] 暂时保留 React 依赖，直到页面迁移完成。

- [ ] 迁移完成后再删除：

```bash
npm uninstall react react-dom react-router-dom react-i18next @tanstack/react-query lucide-react zustand @vitejs/plugin-react @types/react @types/react-dom eslint-plugin-react-hooks eslint-plugin-react-refresh
```

### Task 2: 更新 Vite 配置

**Files:**

- Modify: `vite.config.ts`

- [ ] 替换 React 插件：

```ts
import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [sveltekit(), tailwindcss()],
  server: {
    port: 5173,
    strictPort: true,
  },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    target: 'esnext',
  },
})
```

### Task 3: 新增 SvelteKit 配置

**Files:**

- Create: `svelte.config.js`

```js
import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
    }),
  },
}

export default config
```

### Task 4: 更新 TypeScript 配置

**Files:**

- Modify: `tsconfig.json`
- Modify: `tsconfig.app.json`
- Modify: `tsconfig.node.json`

- [ ] `tsconfig.json` 需要兼容 SvelteKit 生成的 `.svelte-kit/tsconfig.json`。

推荐：

```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler",
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
```

- [ ] 保留当前严格项：

```json
{
  "noUnusedLocals": true,
  "noUnusedParameters": true,
  "noFallthroughCasesInSwitch": true
}
```

- [ ] 移除 React JSX 配置：

```json
{
  "jsx": "react-jsx"
}
```

### Task 5: 更新 npm 脚本

**Files:**

- Modify: `package.json`

- [ ] 保持脚本名称不变。

```json
{
  "typecheck": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json && tsc -p tsconfig.node.json --noEmit",
  "lint": "npm run typecheck && eslint . && npm run lint:responsive && npm run lint:colors && npm run lint:i18n",
  "build": "npm run typecheck && vite build"
}
```

### Task 6: 更新 ESLint 与自定义脚本

**Files:**

- Modify: `eslint.config.js`
- Modify: `scripts/check-responsive-layout.mjs`
- Modify: `scripts/check-color-tokens.mjs`
- Modify: `scripts/check-i18n-literals.mjs`

- [ ] 按第 6、7 节修改。

### Chunk 1 验收

```bash
npm run typecheck
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：在尚未迁移页面时，可能会因为 React/Svelte 混合产生 lint 报错。此阶段至少要确认：

```txt
svelte-kit sync 能运行
svelte-check 能运行
ESLint 能解析 .svelte
三个自定义脚本能扫描 .svelte
vite config 不再依赖 @vitejs/plugin-react
```

建议提交：

```bash
git add package.json package-lock.json vite.config.ts svelte.config.js tsconfig.json tsconfig.app.json eslint.config.js scripts
git commit -m "build: add svelte ui toolchain"
```

---

## Chunk 2: Svelte 应用壳

### Task 7: 建立 SvelteKit 根入口

**Files:**

- Create: `src/routes/+layout.ts`
- Create: `src/routes/+layout.svelte`
- Create: `src/routes/+page.svelte`
- Create: `src/routes/app/+layout.svelte`
- Create: `src/routes/app/+page.svelte`

- [ ] `src/routes/+layout.ts`：

```ts
export const ssr = false
export const prerender = false
```

- [ ] `src/routes/+layout.svelte`：

```svelte
<script lang="ts">
  import '@/index.css'
  import QueryProvider from '@/app/providers/QueryProvider.svelte'
  import ThemeSync from '@/shared/theme/ThemeSync.svelte'
  import { initTheme } from '@/shared/state/theme.svelte'

  let { children } = $props()

  initTheme()
</script>

<QueryProvider>
  <ThemeSync />
  {@render children?.()}
</QueryProvider>
```

- [ ] `src/routes/+page.svelte` 使用 `onMount` + `goto` 跳转到默认页：

```svelte
<script lang="ts">
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { DEFAULT_ROUTE_PATH } from '@/app/router/routeConfig'

  onMount(() => {
    void goto(DEFAULT_ROUTE_PATH, { replaceState: true })
  })
</script>
```

- [ ] `src/routes/app/+layout.svelte`：

```svelte
<script lang="ts">
  import AppLayout from '@/app/layouts/AppLayout.svelte'

  let { children } = $props()
</script>

<AppLayout>
  {@render children?.()}
</AppLayout>
```

- [ ] `src/routes/app/+page.svelte` 同样跳转到默认页。

### Task 8: 迁移 routeConfig

**Files:**

- Modify: `src/app/router/routeConfig.tsx`
- Rename to: `src/app/router/routeConfig.ts`

- [ ] 移除 `ReactElement`、`ComponentType`、`element`。
- [ ] 替换 `lucide-react` 为 `lucide-svelte`。
- [ ] 保留 `RouteTitleKey`、`RouteGroupKey`、`DEFAULT_ROUTE_PATH`。

### Task 9: 迁移 AppLayout

**Files:**

- Modify/Rename: `src/app/layouts/AppLayout.tsx`
- Target: `src/app/layouts/AppLayout.svelte`

- [ ] 保留当前 sidebar/header/main 的视觉结构。
- [ ] 用 `page.url.pathname` 替代 `useLocation()`。
- [ ] 用 `<a href={route.path}>` 替代 `NavLink`。
- [ ] 用 `uiState.sidebarCollapsed` 替代 `useUiStore(...)`。
- [ ] 用 `t(...)` 保留 i18n key。
- [ ] 继续使用语义颜色 token。

### Chunk 2 验收

```bash
npm run typecheck
npm run lint
npm run build
npm run tauri dev
```

预期：

```txt
Tauri 窗口能打开
默认跳转到 /app/dashboard
侧边栏和 header 能渲染
亮暗主题不会首帧闪烁
dist 输出目录仍为 src-tauri/tauri.conf.json 中的 ../dist
```

建议提交：

```bash
git add src/routes src/app src/shared/theme src/shared/state
git commit -m "feat: add svelte app shell"
```

---

## Chunk 3: 共享 UI 与状态

### Task 10: 迁移共享工具

**Files:**

- Keep: `src/shared/lib/tauri.ts`
- Keep: `src/shared/lib/utils.ts`
- Keep: `src/shared/schemas/*`

- [ ] `cn` 可以原样保留。
- [ ] `invokeCmd` 可以原样保留。
- [ ] `errorMessage` 可以原样保留。

### Task 11: 迁移共享 UI

**Files:**

- Create: `src/shared/ui/Button.svelte`
- Create: `src/shared/ui/Card.svelte`
- Create: `src/shared/ui/CardHeader.svelte`
- Create: `src/shared/ui/CardBody.svelte`
- Create: `src/shared/ui/Input.svelte`
- Create: `src/shared/ui/Textarea.svelte`
- Create: `src/shared/ui/Checkbox.svelte`
- Create: `src/shared/ui/Badge.svelte`
- Create: `src/shared/ui/StatusBadge.svelte`
- Create: `src/shared/ui/Spinner.svelte`
- Create: `src/shared/ui/EmptyState.svelte`
- Create: `src/shared/ui/PathPicker.svelte`
- Modify: `src/shared/ui/index.ts`

- [ ] 保留同等 props 语义。
- [ ] 图标插槽使用 Svelte snippet 或 slot。
- [ ] Button 保留：

```txt
variant: primary | secondary | ghost | destructive
size: sm | md
loading
disabled
icon
```

- [ ] Card 拆成独立组件，不要在 Svelte 里模拟 React namespace。
- [ ] `index.ts` 显式导出，继续禁止 `export *`：

```ts
export { default as Button } from './Button.svelte'
export { default as Card } from './Card.svelte'
export { default as CardBody } from './CardBody.svelte'
export { default as CardHeader } from './CardHeader.svelte'
```

### Task 12: 迁移状态

**Files:**

- Create: `src/shared/state/theme.svelte.ts`
- Create: `src/shared/state/ui.svelte.ts`
- Create: `src/shared/state/language.svelte.ts`
- Create: `src/modules/sync/state/syncDecisions.svelte.ts`
- Modify: `src/shared/stores/index.ts` or remove after migration

- [ ] `themeState` 保留 localStorage key：`skill-sync.theme`。
- [ ] `uiState` 保留 localStorage key：`skill-sync.sidebar-collapsed`。
- [ ] `languageState` 保留 localStorage key：`skill-sync.language`。
- [ ] `syncDecisions` 保持：

```txt
decisions: Record<string, string>
setDecision(id, choice)
clear()
```

### Chunk 3 验收

```bash
npm run typecheck
npm run lint
```

预期：

```txt
共享 UI 无直接 palette color
共享 UI 无中文硬编码
共享 UI 无大屏固定尺寸违规
状态不再依赖 zustand
```

建议提交：

```bash
git add src/shared src/modules/sync/state
git commit -m "refactor: migrate shared ui to svelte"
```

---

## Chunk 4: 页面逐个迁移

### 推荐顺序

从依赖最少到依赖最多：

```txt
1. BackupsPage
2. SkillsPage
3. ConflictsPage
4. DashboardPage
5. OnboardingPage
6. SettingsPage
7. SyncPreviewPage
```

这个顺序的理由：

```txt
Backups/Skills: 查询 + 列表，适合验证 createQuery
Conflicts: 验证共享 syncDecisions state
Dashboard: 验证多个查询和导航
Onboarding: 验证表单 + 手写 async 状态
Settings: 验证复杂表单 + theme/language
SyncPreview: 验证 mutation + invalidate + decisions
```

### Task 13: 迁移 BackupsPage

**Files:**

- Rename: `src/modules/backups/pages/BackupsPage.tsx`
- Target: `src/modules/backups/pages/BackupsPage.svelte`
- Modify: `src/modules/backups/index.ts`
- Create: `src/routes/app/backups/+page.svelte`

- [ ] 替换 `useQuery` 为 `createQuery`。
- [ ] 替换 `useMutation` 为 `createMutation`。
- [ ] 替换 `useQueryClient` 为 `useQueryClient` from `@tanstack/svelte-query`。
- [ ] 替换 `lucide-react` 为 `lucide-svelte`。
- [ ] 所有中文文案继续用 `t(...)`。

验收：

```bash
npm run lint
npm run build
```

### Task 14: 迁移 SkillsPage

**Files:**

- Rename: `src/modules/skills/pages/SkillsPage.tsx`
- Target: `src/modules/skills/pages/SkillsPage.svelte`
- Modify: `src/modules/skills/index.ts`
- Create: `src/routes/app/skills/+page.svelte`

- [ ] 保留 `scanSkills` API 和 Zod 校验。
- [ ] 保留 warnings 显示。
- [ ] `openPath(skill.source_path)` 继续通过 Tauri command。

验收：

```bash
npm run lint
npm run build
```

### Task 15: 迁移 ConflictsPage

**Files:**

- Rename: `src/modules/conflicts/pages/ConflictsPage.tsx`
- Target: `src/modules/conflicts/pages/ConflictsPage.svelte`
- Modify: `src/modules/conflicts/index.ts`
- Create: `src/routes/app/conflicts/+page.svelte`

- [ ] 使用 `syncDecisions.decisions` 和 `syncDecisions.setDecision(...)`。
- [ ] 保留冲突选择：

```txt
local
remote
skip
```

验收：

```bash
npm run lint
npm run build
```

### Task 16: 迁移 DashboardPage

**Files:**

- Rename: `src/modules/dashboard/pages/DashboardPage.tsx`
- Target: `src/modules/dashboard/pages/DashboardPage.svelte`
- Modify: `src/modules/dashboard/index.ts`
- Create: `src/routes/app/dashboard/+page.svelte`

- [ ] 替换 `useNavigate` 为 `goto`。
- [ ] 保留 `app-state`、`scan-skills`、`sync-plan` query keys。
- [ ] `enabled: configured` 用 Svelte Query 响应式函数表达：

```ts
const scan = createQuery(() => ({
  queryKey: ['scan-skills'],
  queryFn: scanSkills,
  enabled: configured,
}))
```

注意 `configured` 需要是 `$derived`。

验收：

```bash
npm run lint
npm run build
```

### Task 17: 迁移 OnboardingPage

**Files:**

- Rename: `src/modules/onboarding/pages/OnboardingPage.tsx`
- Target: `src/modules/onboarding/pages/OnboardingPage.svelte`
- Modify: `src/modules/onboarding/index.ts`
- Create: `src/routes/app/onboarding/+page.svelte`

- [ ] 用 `$state` 替代 `useState`：

```ts
let remote = $state('')
let branch = $state('main')
let msg = $state('')
let saving = $state(false)
```

- [ ] 用 `$effect` 或显式 guard 替代当前 render-time prefill。
- [ ] `goto('/app/dashboard')` 替代 `navigate('/app/dashboard')`。

验收：

```bash
npm run lint
npm run build
```

### Task 18: 迁移 SettingsPage

**Files:**

- Rename: `src/modules/settings/pages/SettingsPage.tsx`
- Target: `src/modules/settings/pages/SettingsPage.svelte`
- Modify: `src/modules/settings/index.ts`
- Create: `src/routes/app/settings/+page.svelte`

- [ ] `$state` 管理 `config`、`codexPaths`、`claudePaths`、`ignore`、`msg`。
- [ ] `themeState.theme` / `themeState.setTheme(...)` 替代 Zustand。
- [ ] `languageState.language` / `languageState.setLanguage(...)` 替代 Zustand。
- [ ] 保留保存后 invalidation：

```ts
queryClient.invalidateQueries({ queryKey: ['app-state'] })
```

验收：

```bash
npm run lint
npm run build
```

### Task 19: 迁移 SyncPreviewPage

**Files:**

- Rename: `src/modules/sync/pages/SyncPreviewPage.tsx`
- Target: `src/modules/sync/pages/SyncPreviewPage.svelte`
- Modify: `src/modules/sync/index.ts`
- Create: `src/routes/app/sync/+page.svelte`

- [ ] 保留分组显示：

```txt
uploads
downloads
updates
deletes
conflicts
```

- [ ] 保留 apply mutation。
- [ ] 保留 apply 成功后：

```txt
result message
clear decisions
invalidate sync-plan
```

验收：

```bash
npm run lint
npm run build
```

### Chunk 4 最终验收

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
npm run tauri dev
```

手动检查：

```txt
Dashboard 能加载配置状态
Skills 能扫描并打开目录
Sync Preview 能预览和应用同步计划
Conflicts 能选择 local/remote/skip
Backups 能列出和恢复
Settings 能保存配置、切换主题、切换语言
Onboarding 能检查 Git/remote 并保存
窄屏布局不出现不可控横向挤压
暗色/亮色模式颜色来自 token
中文文案切换能立即反映到 UI
```

建议提交：

```bash
git add src/routes src/modules
git commit -m "refactor: migrate pages to svelte"
```

---

## Chunk 5: 移除 React 遗留

### Task 20: 删除 React 入口

**Files:**

- Delete: `src/main.tsx`
- Delete: `src/App.tsx`
- Delete: `index.html` 中 React root 相关入口，或按 SvelteKit 需要调整

SvelteKit 管理入口后，不再需要手写 React mount。

### Task 21: 删除 React 依赖

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`

执行：

```bash
npm uninstall react react-dom react-router-dom react-i18next @tanstack/react-query lucide-react zustand @vitejs/plugin-react @types/react @types/react-dom eslint-plugin-react-hooks eslint-plugin-react-refresh
```

如果 `radix-ui` 当前没有实际使用，也一起移除：

```bash
npm uninstall radix-ui
```

`class-variance-authority` 如果没有在 Svelte UI 中使用，也移除。

### Task 22: 删除 `.tsx`

执行：

```bash
rg --files src | rg '\.tsx$'
```

预期：无输出。

然后把脚本中的 `.tsx` 兼容项删除：

```txt
scripts/check-responsive-layout.mjs
scripts/check-color-tokens.mjs
scripts/check-i18n-literals.mjs
```

### Task 23: 更新文档

**Files:**

- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `components.json` or remove if no longer using shadcn React config

需要把 `React Architecture` 改成 `Svelte Architecture`，并保留原有思想：

```txt
按产品能力组织
app/routes 只做装配
modules 承载业务页面和 API
shared 放稳定跨模块基础
禁止空架构
禁止 thin wrapper
```

### Chunk 5 验收

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
npm run tauri dev
```

最终提交：

```bash
git add .
git commit -m "chore: remove react frontend"
```

---

## 9. 风险与处理

### 风险 1: ESLint 对 `.svelte` import 解析不完整

处理：

```txt
先保证 svelte-eslint-parser 正常解析 <script lang="ts">
如果 import-x/no-cycle 对 .svelte 支持不足，可以把 cycle 检查暂时限定到 .ts，再用 no-restricted-imports 和 local/no-reverse-layer-imports 覆盖 .svelte
但不能关闭层级检查和深导入限制
```

### 风险 2: SvelteKit static fallback 与 Tauri 路由

处理：

```txt
使用 adapter-static fallback: 'index.html'
根路由和 /app 路由用 client goto 跳转
确认生产 build 后 npm run tauri dev / tauri build 能加载 /app/dashboard
如果深链接无法直接打开，桌面应用默认只从根入口进入即可接受
```

### 风险 3: i18n 不重新渲染

处理：

```txt
t(...) 内部读取 languageState.language
语言切换必须更新 languageState.language
SettingsPage 切语言后手动确认 layout/nav/header/page 文案同步更新
```

### 风险 4: 迁移时 UI 细节漂移

处理：

```txt
先迁移现有 handmade UI，不立即换成新设计系统
保留 Tailwind token
逐页截图对照
每迁一页跑 lint + build
```

### 风险 5: runes 共享状态被滥用

处理：

```txt
只有主题、语言、侧栏折叠、冲突决策使用共享 runes state
Tauri 命令数据继续用 Svelte Query
表单临时输入留在页面本地
不要把所有东西塞进 shared state
```

---

## 10. 最终验收清单

必须全部通过：

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
npm run tauri dev
```

检查依赖：

```bash
npm ls react react-dom react-router-dom react-i18next @tanstack/react-query lucide-react zustand
```

预期：这些包不再作为项目直接依赖出现。

检查文件：

```bash
rg --files src | rg '\.tsx$'
```

预期：无输出。

检查深导入：

```bash
npm run lint
```

预期：跨模块深导入、反向层级导入、`export *` 都会被拦截。

检查自定义边界：

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：

```txt
Responsive layout lint passed.
Color token lint passed.
i18n literal lint passed.
```

---

## 11. 参考文档

- Tauri frontend docs: https://v2.tauri.app/start/frontend/
- SvelteKit single-page apps: https://svelte.dev/docs/kit/single-page-apps
- Svelte TypeScript docs: https://svelte.dev/docs/typescript
- Svelte runes docs: https://svelte.dev/docs/svelte/what-are-runes
- TanStack Query Svelte docs: https://tanstack.com/query/latest/docs/framework/svelte/overview
- eslint-plugin-svelte: https://github.com/sveltejs/eslint-plugin-svelte
