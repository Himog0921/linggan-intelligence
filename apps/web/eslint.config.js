import js from '@eslint/js';
import { defineConfig, globalIgnores } from 'eslint/config';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default defineConfig([
  globalIgnores(['dist', 'node_modules']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat['recommended-latest'],
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
  },
  {
    files: ['vite.config.ts', 'eslint.config.js'],
    extends: [js.configs.recommended],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    // 浏览器证明脚本在 Node 里运行，传给 page.evaluate 的函数在浏览器里运行。
    files: ['test/**/*.mjs'],
    extends: [js.configs.recommended],
    languageOptions: {
      globals: { ...globals.node, ...globals.browser },
    },
  },
]);
