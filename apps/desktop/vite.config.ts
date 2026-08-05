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
