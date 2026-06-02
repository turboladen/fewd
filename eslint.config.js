import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['dist', 'eslint.config.js'] },
  {
    files: ['src/**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2020,
      // Mirror the old eslintrc `env: { browser: true, es2020: true }`: browser
      // DOM globals + the ES2020 built-ins (Promise, Map, Set, globalThis, …).
      globals: { ...globals.browser, ...globals.es2020 },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      // Mirror the pre-migration ruleset (eslint-plugin-react-hooks v4 `recommended`).
      // react-hooks v7's `recommended-latest` adds the React Compiler rule family
      // (set-state-in-effect, purity, immutability, …) — adopting those + fixing the
      // sites they surface is tracked in fewd-6h2 so this stays a pure tooling migration.
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
)
