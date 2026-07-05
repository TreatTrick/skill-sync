import js from '@eslint/js'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createTypeScriptImportResolver } from 'eslint-import-resolver-typescript'
import importX from 'eslint-plugin-import-x'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

const PROJECT_ROOT = path.dirname(fileURLToPath(import.meta.url))
const SRC_ROOT = path.join(PROJECT_ROOT, 'src')

const LAYER_ORDER = {
  shared: 0,
  modules: 1,
  app: 2,
}

const getLayerFromPath = (filePath) => {
  const relativePath = path.relative(SRC_ROOT, path.resolve(filePath))

  if (relativePath.startsWith('..') || path.isAbsolute(relativePath)) {
    return null
  }

  const [layer] = relativePath.split(path.sep)

  return Object.hasOwn(LAYER_ORDER, layer) ? layer : null
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
        'Reverse layer import is not allowed: {{fromLayer}} cannot import {{toLayer}}. Allowed direction is app -> modules -> shared.',
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

export default defineConfig([
  // src-tauri 是 Rust/Tauri 后端，包含构建产物（target/ 下的生成代码），
  // 不属于前端 ESLint 检查范围，需整体排除
  globalIgnores(['dist', 'src-tauri']),
  {
    files: ['**/*.{ts,tsx}'],
    plugins: {
      'import-x': importX,
      local: {
        rules: {
          'no-reverse-layer-imports': noReverseLayerImportsRule,
        },
      },
    },
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      importX.flatConfigs.typescript,
    ],
    languageOptions: {
      globals: globals.browser,
    },
    settings: {
      'import-x/resolver-next': [
        createTypeScriptImportResolver({
          project: './tsconfig.app.json',
        }),
        importX.createNodeResolver(),
      ],
    },
    rules: {
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
      'no-restricted-syntax': [
        'error',
        {
          selector: 'ExportAllDeclaration',
          message:
            'Do not use export *; explicitly name every exported member.',
        },
        {
          selector: 'ExportNamedDeclaration > ExportNamespaceSpecifier',
          message:
            'Do not use export * as namespace; explicitly name every exported member.',
        },
      ],
    },
  },
  {
    files: ['src/shared/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
])
