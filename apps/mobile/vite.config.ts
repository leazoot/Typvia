// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Tauri mobile dev: the CLI injects TAURI_DEV_HOST when the WebView must
// reach the dev server through a forwarded/remote address (Android emulator
// uses adb reverse, physical devices use the LAN IP).
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: host ?? false,
    port: 5183,
    strictPort: true,
    ...(host ? { hmr: { protocol: 'ws', host, port: 5184 } } : {}),
  },
});
