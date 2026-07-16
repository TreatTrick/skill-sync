import js from '@eslint/js'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createTypeScriptImportResolver } from 'eslint-import-resolver-typescript'
import importX from 'eslint-plugin-import-x'
import svelte from 'eslint-plugin-svelte'
import svelteParser from 'svelte-eslint-parser'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

const PROJECT_ROOT = path.dirname(fileURLToPath(import.meta.url))
const SRC_ROOT = path.join(PROJECT_ROOT, 'src')
const MODULES_ROOT = path.join(SRC_ROOT, 'modules')

const LAYER_ORDER = {
  shared: 0,
  modules: 1,
  app: 2,
  routes: 3,
}

const getLayerFromPath = (filePath) => {
  const relativePath = path.relative(SRC_ROOT, path.resolve(filePath))

  if (relativePath.startsWith('..') || path.isAbsolute(relativePath)) {
    return null
  }

  const [layer] = relativePath.split(path.sep)

  return Object.hasOwn(LAYER_ORDER, layer) ? layer : null
}

// Returns the module name when a file lives under src/modules/<name>/, else null.
const getModuleName = (filePath) => {
  const relativePath = path.relative(MODULES_ROOT, path.resolve(filePath))

  if (relativePath.startsWith('..') || path.isAbsolute(relativePath)) {
    return null
  }

  const [name] = relativePath.split(path.sep)

  return name || null
}

const getLayerFromImportSource = (source, importerPath) => {
  if (source.startsWith('@/')) {
    const [layer] = source.slice(2).split('/')

    return Object.hasOwn(LAYER_ORDER, layer) ? layer : null
  }

  if (source.startsWith('src/')) {
    const [layer] = source.slice(4).split('/')

    return Object.hasOwn(LAYER_ORDER, layer) ? layer : null
  }

  if (source.startsWith('.')) {
    return getLayerFromPath(path.resolve(path.dirname(importerPath), source))
  }

  return null
}

// Resolves the module name targeted by an import source, or null when the
// import does not point at src/modules/<name>. Relative imports are resolved
// against the importer so cross-module relative paths are caught too.
const getModuleFromImportSource = (source, importerPath) => {
  if (source.startsWith('@/modules/')) {
    const [name] = source.slice('@/modules/'.length).split('/')

    return name || null
  }

  if (source.startsWith('src/modules/')) {
    const [name] = source.slice('src/modules/'.length).split('/')

    return name || null
  }

  if (source.startsWith('.')) {
    return getModuleName(path.resolve(path.dirname(importerPath), source))
  }

  return null
}

const getRuleFilename = (context) =>
  context.physicalFilename ??
  context.filename ??
  context.getPhysicalFilename?.() ??
  context.getFilename?.()

const noReverseLayerImportsRule = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Disallow reverse imports between app, modules, and shared layers.',
    },
    schema: [],
    messages: {
      reverseLayerImport:
        'Reverse layer import is not allowed: {{fromLayer}} cannot import {{toLayer}}. Allowed direction is routes -> app -> modules -> shared.',
    },
  },
  create(context) {
    const importerPath = getRuleFilename(context)
    const fromLayer = importerPath ? getLayerFromPath(importerPath) : null

    const checkSource = (sourceNode, reportNode = sourceNode) => {
      if (!fromLayer || sourceNode?.type !== 'Literal') {
        return
      }

      const source = sourceNode.value

      if (typeof source !== 'string') {
        return
      }

      const toLayer = getLayerFromImportSource(source, importerPath)

      if (toLayer && LAYER_ORDER[toLayer] > LAYER_ORDER[fromLayer]) {
        context.report({
          node: reportNode,
          messageId: 'reverseLayerImport',
          data: {
            fromLayer,
            toLayer,
          },
        })
      }
    }

    return {
      ImportDeclaration(node) {
        checkSource(node.source)
      },
      ExportAllDeclaration(node) {
        checkSource(node.source)
      },
      ExportNamedDeclaration(node) {
        checkSource(node.source)
      },
      ImportExpression(node) {
        checkSource(node.source, node)
      },
      CallExpression(node) {
        if (node.callee.type === 'Import') {
          checkSource(node.arguments[0], node)
        }
      },
    }
  },
}

const noCrossModuleImportsRule = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Disallow imports between sibling modules under src/modules.',
    },
    schema: [],
    messages: {
      crossModuleImport:
        'Cross-module import is not allowed: module "{{importerModule}}" cannot import module "{{targetModule}}". Move the shared concern to src/shared.',
    },
  },
  create(context) {
    const importerPath = getRuleFilename(context)
    const importerModule = importerPath ? getModuleName(importerPath) : null

    const checkSource = (sourceNode, reportNode = sourceNode) => {
      if (!importerModule || sourceNode?.type !== 'Literal') {
        return
      }

      const source = sourceNode.value

      if (typeof source !== 'string') {
        return
      }

      const targetModule = getModuleFromImportSource(source, importerPath)

      if (targetModule && targetModule !== importerModule) {
        context.report({
          node: reportNode,
          messageId: 'crossModuleImport',
          data: {
            importerModule,
            targetModule,
          },
        })
      }
    }

    return {
      ImportDeclaration(node) {
        checkSource(node.source)
      },
      ExportAllDeclaration(node) {
        checkSource(node.source)
      },
      ExportNamedDeclaration(node) {
        checkSource(node.source)
      },
      ImportExpression(node) {
        checkSource(node.source, node)
      },
      CallExpression(node) {
        if (node.callee.type === 'Import') {
          checkSource(node.arguments[0], node)
        }
      },
    }
  },
}

// Shared rules reused across .ts/.tsx and .svelte blocks.
const IMPORT_RULES = {
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
  'local/no-cross-module-imports': 'error',
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
  'no-restricted-syntax': [
    'error',
    {
      selector: 'ExportAllDeclaration',
      message: 'Do not use export *; explicitly name every exported member.',
    },
    {
      selector: 'ExportNamedDeclaration > ExportNamespaceSpecifier',
      message:
        'Do not use export * as namespace; explicitly name every exported member.',
    },
  ],
}

const IMPORT_SETTINGS = {
  'import-x/resolver-next': [
    createTypeScriptImportResolver({
      project: './tsconfig.json',
    }),
    importX.createNodeResolver(),
  ],
}

const LOCAL_PLUGIN = {
  local: {
    rules: {
      'no-reverse-layer-imports': noReverseLayerImportsRule,
      'no-cross-module-imports': noCrossModuleImportsRule,
    },
  },
}

export default defineConfig([
  // src-tauri 是 Rust/Tauri 后端，包含构建产物（target/ 下的生成代码），
  // 不属于前端 ESLint 检查范围，需整体排除
  globalIgnores(['dist', 'src-tauri', '.svelte-kit']),
  // Svelte 基础规则与 svelte-eslint-parser 注册
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.ts'],
    plugins: {
      'import-x': importX,
      ...LOCAL_PLUGIN,
    },
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      importX.flatConfigs.typescript,
    ],
    languageOptions: {
      globals: globals.browser,
    },
    settings: IMPORT_SETTINGS,
    rules: IMPORT_RULES,
  },
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
      'import-x': importX,
      ...LOCAL_PLUGIN,
    },
    settings: IMPORT_SETTINGS,
    rules: {
      ...IMPORT_RULES,
      // Tauri serves the SPA from root; there is no base path to resolve
      // against, so requiring resolve() adds noise without value here.
      'svelte/no-navigation-without-resolve': 'off',
    },
  },
  {
    files: ['src/shared/**/*.{ts,svelte}'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
])
