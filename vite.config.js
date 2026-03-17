import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { viteSingleFile } from 'vite-plugin-singlefile';

export default defineConfig({
  plugins: [react(), viteSingleFile()],
  base: './',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    modulePreload: false,
    // singlefile needs everything in one chunk
    cssCodeSplit: false,
    assetsInlineLimit: 100000000,
    rollupOptions: {
      output: {
        manualChunks: undefined,
      },
    },
  },
});
