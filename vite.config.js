import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Strip crossorigin attributes — breaks Tauri WebView2 on some Windows systems
const removeCrossorigin = () => ({
  name: 'remove-crossorigin',
  transformIndexHtml(html) {
    return html.replace(/ crossorigin/g, '');
  },
});

export default defineConfig({
  plugins: [react(), removeCrossorigin()],
  // Tauri: use relative paths so assets resolve via custom protocol
  base: './',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Tauri uses Chromium on Windows, WebKit on macOS/Linux
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    // Disable module preload — Tauri serves locally, no benefit
    modulePreload: false,
    rollupOptions: {
      output: {
        manualChunks: {
          'motion': ['framer-motion'],
          'dock': ['rc-dock'],
          'charts': ['lightweight-charts'],
        },
      },
    },
  },
});
