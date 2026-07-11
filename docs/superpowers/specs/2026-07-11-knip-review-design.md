# Knip 审查接入设计

## 目标

为当前 npm 项目增加 Knip 静态审查，用于发现未使用的文件、导出和依赖。审查既可以独立运行，也纳入现有全量 `npm run lint`，使本地检查和 CI 调用同一入口。

## 范围

- 使用 `npm install --save-dev knip@6` 添加开发依赖，并由 `package-lock.json` 固定实际安装版本。
- 新增 `npm run lint:knip`，只负责运行 Knip。
- 在现有 `npm run lint` 末尾调用 `npm run lint:knip`。
- 新增精简的 `knip.json`，描述 SvelteKit 路由、Vite/Svelte 配置、项目 TypeScript/Svelte 源文件和仓库脚本等真实入口与扫描范围。
- 运行首次审查，并处理配置误报；真实未使用项只记录，不在未确认时删除。

以下内容不在本次范围内：Rust 死代码检查、自动删除未使用代码、把全仓库 Knip 加入 `lint-staged`、新增 CI 工作流或宽泛忽略目录。

## 配置与命令

`package.json` 暴露两个层次的接口：

- `lint:knip`: 开发者排查 Knip 结果时使用的独立命令。
- `lint`: 保留现有类型、ESLint、响应式、颜色与国际化检查，并在这些检查之后运行 `lint:knip`。

`knip.json` 使用 Knip 6 的 JSON Schema，并保持静态、可审查的配置：

```json
{
  "$schema": "https://unpkg.com/knip@6/schema.json",
  "entry": [
    "src/routes/**/+*.{js,ts,svelte}",
    "svelte.config.js",
    "vite.config.ts",
    "eslint.config.js",
    "scripts/*.mjs"
  ],
  "project": ["src/**/*.{js,ts,svelte}", "scripts/**/*.mjs"]
}
```

`entry` 只包含框架、工具或 npm scripts 能够直接加载的文件。SvelteKit 的 `+page`、`+layout`、`+server` 和 `+error` 等约定文件由 `+*` 统一覆盖；根配置和现有 `scripts/*.mjs` 是工具入口。`project` 中的 `src/**/*.ts` 同时覆盖普通 TypeScript、`.svelte.ts` runes 模块和 `src/app.d.ts` 声明文件。项目当前没有测试文件；未来新增独立测试入口时应显式扩展配置。

Knip 根据已安装的 `svelte`、`@sveltejs/kit` 和 Vite 依赖启用内置插件，用于编译 `.svelte` 文件、解析 SvelteKit 特殊引用和识别配置依赖；显式入口不依赖插件替开发者猜测路由。`.svelte-kit`、`dist`、`public` 和 `src-tauri` 不在 JavaScript 项目 glob 中：前两者是生成产物，`public` 是静态资源，`src-tauri` 属于 Rust 检查范围。

Knip 不加入 `lint-staged`。其分析依赖完整项目图，只检查暂存文件会产生不完整结果，也会不必要地拖慢每次提交。

## 告警处理

首次 `npm run lint:knip` 的结果按以下顺序处理：

1. 确认报告项是否由框架约定、配置加载或脚本入口隐式引用。
2. 动态 `import()` 由 Knip 的 AST 结果判断；SvelteKit 路由和 npm 脚本已由入口配置覆盖。应用不是发布库，因此仅在应用内部引用的 barrel 导出不视为外部公共契约。
3. 对工具无法推导、但能够指出实际调用方的入口，在 `knip.json` 中精确声明，并在实现结果中写明理由。
4. 对确认未使用的文件、导出或依赖，只记录文件位置和类别并暂停实现，请用户决定是否扩大范围。本次设计不授权删除既有应用代码或依赖。
5. 不为真实告警添加 `ignore`、`ignoreDependencies`、`ignoreExportsUsedInFile` 或覆盖目录的通配忽略。只有框架或外部工具确实使用、且无法通过入口表达的项目才允许精确例外；例外必须逐项记录理由并重新接受规格确认。

若 Knip 无法执行或发现问题，命令保持非零退出码，使 `npm run lint` 失败并输出原始诊断。

## 验证

实现完成后依次运行：

```bash
npm run lint:knip
npm run typecheck
npm run format:check
npm run lint
npm run build
```

验收标准如下：

- `npm run lint:knip` 退出码为零，且没有未处置的 Knip finding。
- `package.json` 的 `lint` 脚本仅调用一次 `npm run lint:knip`，并通过 `&&` 放在现有检查之后；npm 的退出码传播保证 Knip 失败会使全量 lint 失败。
- `npm run lint` 输出一次 `lint:knip` 子命令和 Knip 结果，并以零退出码结束。
- 配置不包含任何 `ignore*` 项；若首次扫描发现真实未使用项，当前实现不满足验收条件，应在用户决定处置方式前暂停。
- 其余预完成命令保持通过；既有、与本次变更无关的基线失败需要如实报告，不得顺手修改。
