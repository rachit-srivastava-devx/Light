import {createRequire} from 'node:module';
import {join} from 'node:path';

const transientModules = process.env['ORB_ESLINT_NODE_MODULES'];
const require = transientModules
  ? createRequire(join(transientModules, '__orb-eslint-loader.cjs'))
  : createRequire(import.meta.url);
const tseslint = require('typescript-eslint');

export default tseslint.config(
  {
    ignores: [
      '**/node_modules/**',
      '**/build/**',
      '**/dist/**',
      '**/coverage/**',
      '**/Pods/**',
      '**/target/**',
    ],
  },
  {
    files: [
      'apps/*/src/**/*.{ts,tsx}',
      'backend/*-sidecar/src/**/*.{ts,tsx}',
    ],
    extends: [tseslint.configs.recommendedTypeChecked],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/await-thenable': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-misused-promises': 'error',
      '@typescript-eslint/no-unnecessary-condition': 'error',
      '@typescript-eslint/switch-exhaustiveness-check': 'error',
      eqeqeq: ['error', 'always'],
      'no-fallthrough': 'error',
      'require-atomic-updates': 'error',
    },
  },
);
