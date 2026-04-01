import { defineConfig } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy';
import * as path from 'path';

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    fs: {
      allow: ['.', 'extensions'],
    },
  },
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: 'extensions/*',
          dest: 'extensions',
        },
        {
          src: 'extensions-meta.json',
          dest: '.',
        },
      ],
    }),
  ],
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      'vs': path.resolve(__dirname, 'src/vs'),
    },
  },
  build: {
    target: ['es2022', 'chrome100', 'safari15'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    chunkSizeWarningLimit: 10000,
    rollupOptions: {
      output: {
        manualChunks: {
          'monaco-editor': ['monaco-editor'],
        },
      },
    },
  },
  optimizeDeps: {
    exclude: ['@tauri-apps/api', '@tauri-apps/plugin-dialog', '@tauri-apps/plugin-fs',
              '@tauri-apps/plugin-clipboard-manager', '@tauri-apps/plugin-shell',
              '@tauri-apps/plugin-notification', '@tauri-apps/plugin-opener'],
  },
  worker: {
    format: 'es',
  },
});
