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
      // react-hooks v7's `recommended-latest` is the React Compiler rule family
      // (set-state-in-effect, purity, immutability, set-state-in-render, …) plus the
      // classic rules-of-hooks/exhaustive-deps. The two rules below are kept explicitly
      // because that config doesn't provide them (they're not react-hooks rules).
      ...reactHooks.configs['recommended-latest'].rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
)
