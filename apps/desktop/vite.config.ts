// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Tauri drives the dev cycle and expects the dev server on a fixed port;
// a random port would break `tauri dev` (devUrl in tauri.conf.json).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'es2022',
  },
});
