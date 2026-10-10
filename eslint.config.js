import js from '@eslint/js';
import solid from 'eslint-plugin-solid';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    plugins: {
      solid,
    },
    rules: {
      ...solid.configs['flat/typescript'].rules,
      complexity: ['warn', { max: 15, variant: 'classic' }],
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
        },
      ],
    },
  },
  {
    files: ['**/*.test.ts', '**/*.test.tsx', '**/*.spec.ts', '**/*.spec.tsx'],
    rules: {
      complexity: 'off',
    },
  },
  {
    ignores: ['dist', 'src-tauri'],
  },
);
