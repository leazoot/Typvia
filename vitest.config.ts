// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['apps/**/src/**/*.test.{ts,tsx}', 'packages/**/src/**/*.test.{ts,tsx}'],
    // The workspace starts with no frontend tests; the suite must still be runnable
    // so the unified `test` entry works from the beginning.
    passWithNoTests: true,
  },
});
