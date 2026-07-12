# Svelte UI 架构说明

当前前端使用 Svelte 5、SvelteKit、TypeScript strict、Tailwind CSS v4、shadcn-svelte 和 `@tanstack/svelte-query`。

## 目录边界

```text
src/
  routes/              # SvelteKit 路由薄包装
  app/                 # 应用组装、布局、路由门禁
  modules/
    onboarding/       # Device Flow、installation、Vault bind
    settings/         # 只读 identity、limits、ignore、固定 roots
    skills/           # scan API 和 schema
    sync/              # plan、决策、预览和 recovery
  shared/             # UI、schemas、i18n、请求封装、稳定 client state
```

依赖方向固定为 `routes -> app -> modules -> shared`。模块之间不能深度导入私有实现；跨模块能力通过模块入口、shared capability 或后端 command 复用。

## 路由和门禁

认证工作区只包含：

- `/app/sync`
- `/app/settings`

`/app/onboarding` 是无 sidebar 的引导路由。根路由和 app layout 先等待 `get_app_state()`，只有完整 Ready Vault binding 且 credential 状态为 `valid` 或 `refreshing` 时才渲染工作区布局。Device Flow authorized、GitHub App installed 或 initialize ready 都不会提前解锁工作区。

## API 和状态

- 每个 module 的 API 函数负责调用 Tauri、zod response validation 和错误归一化。
- `@tanstack/svelte-query` 管理 AppState、scan、sync plan 等服务端数据。
- `.svelte.ts` runes 只保存小型客户端状态，例如主题、语言和临时同步决策。
- 表单状态保存在页面内；稳定身份、root、credential 和 Vault 状态来自后端，不允许在 Settings 伪造或编辑。

## UI 约束

- 用户文案只放在 `src/shared/i18n/locales`，组件通过 `t()` 读取。
- 颜色只使用 `src/index.css` 暴露的语义 token。
- 新页面保持工具型、密集、可扫描；不要恢复已删除的旧引导、可编辑 root、独立备份或独立冲突路由。
- 布局、颜色和文案改动后运行 `npm run lint`。
