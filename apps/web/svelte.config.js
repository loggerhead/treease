import path from 'node:path'
import { fileURLToPath } from 'node:url'
import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

const here = path.dirname(fileURLToPath(import.meta.url))
const coreWasmDir = path.resolve(here, '..', '..', 'packages', 'core', 'wasm')


const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: 'index.html' }),
    alias: {
      '@core-wasm': coreWasmDir,
      '@core-wasm/index': path.resolve(coreWasmDir, 'index.ts'),
    },
  }
}

export default config
