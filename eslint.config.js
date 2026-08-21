// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

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
