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
