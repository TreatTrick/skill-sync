import { existsSync, mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// site/ 构建时 oxc（rolldown-vite）会从被编译的 TS 文件向上加载祖先层 tsconfig，
// 包括仓库根的 tsconfig.json（Tauri 主应用）。它 extends "./.svelte-kit/tsconfig.json"，
// 但 Tauri 主应用的 .svelte-kit 在 fresh clone / CI（只装 site 依赖）下不存在，
// 会导致 site 构建报 "Tsconfig not found <repo>/.svelte-kit/tsconfig.json"。
// 缺失时这里建一个最小桩让 extends 能解析；Tauri 主应用自己的 svelte-kit sync 会随后覆盖它。
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const rootTsconfig = resolve(repoRoot, '.svelte-kit', 'tsconfig.json')

if (!existsSync(rootTsconfig)) {
  mkdirSync(dirname(rootTsconfig), { recursive: true })
  writeFileSync(rootTsconfig, '{\n  "compilerOptions": {}\n}\n')
  console.log(`[ensure-root-tsconfig] created stub ${rootTsconfig}`)
}
