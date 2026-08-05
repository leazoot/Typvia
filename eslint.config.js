import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      '**/node_modules/',
      '**/dist/',
      '**/target/',
      '**/src-tauri/gen/',
      'native/**/build/',
      'espanso/',
      'design/',
      'docs/',
    ],
  },
  ...tseslint.configs.recommended,
  {
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
    },
  },
  {
    // The typed IPC layer in packages/shared is the frontend's only data
    // entry point; nothing else may talk to the Tauri bridge directly.
    files: ['apps/**/src/**', 'packages/ui/src/**'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@tauri-apps/api/core',
              message: 'Use the typed IPC layer from @typvia/shared instead of invoke().',
            },
          ],
        },
      ],
    },
  },
);
