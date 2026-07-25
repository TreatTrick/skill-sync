import js from '@eslint/js'
import svelte from 'eslint-plugin-svelte'
import svelteParser from 'svelte-eslint-parser'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['build', '.svelte-kit']),
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.ts'],
    extends: [js.configs.recommended, tseslint.configs.recommended],
    languageOptions: {
      globals: globals.browser,
    },
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
    // External links (GitHub, releases) and a base-aware home link do not need
    // resolve(); the site is a base-path SPA like the Tauri shell.
    rules: {
      'svelte/no-navigation-without-resolve': 'off',
    },
  },
])
