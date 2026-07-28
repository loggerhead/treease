import path from 'node:path';
import { copyFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const coreWasmDir = path.resolve(appDir, '../../packages/core/wasm');
const webSrcDir = path.resolve(appDir, '../web/src');

export default defineConfig({
  resolve: {
    alias: [
      { find: /^@core-wasm(\/.*)?$/, replacement: `${coreWasmDir}$1` },
      // The extension consumes the same graph-stream protocol and delta reducer as
      // the Web viewer. Keep this as an explicit boundary until these modules move
      // to a workspace package; do not fork their Core projection semantics here.
      { find: /^@treease-web(\/.*)?$/, replacement: `${webSrcDir}$1` },
    ],
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
    rollupOptions: {
      input: {
        sidepanel: path.resolve(appDir, 'sidepanel.html'),
        content: path.resolve(appDir, 'src/content/index.ts'),
        background: path.resolve(appDir, 'src/background/service-worker.ts'),
      },
      output: {
        entryFileNames: '[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
      },
    },
  },
  plugins: [{
    name: 'copy-treease-license',
    closeBundle: async () => {
      await copyFile(path.resolve(appDir, '../../LICENSE'), path.resolve(appDir, 'dist/LICENSE'));
    },
  }],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts', 'test/**/*.test.ts'],
    exclude: ['test/**/*.integration.test.ts'],
  },
});
