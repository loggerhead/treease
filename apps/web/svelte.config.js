import path from 'node:path'
import { fileURLToPath } from 'node:url'
import adapterCloudflare from '@sveltejs/adapter-cloudflare'
import adapterStatic from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

const here = path.dirname(fileURLToPath(import.meta.url))
const coreWasmDir = path.resolve(here, '..', '..', 'packages', 'core', 'wasm')
const workspaceSurface = process.env.TREEASE_WORKSPACE_SURFACE ?? 'web'
const buildDirectory = workspaceSurface === 'desktop' ? '../desktop/dist' : 'build'
const e2eBuild = process.env.TREEASE_E2E === 'true'
const adapter = workspaceSurface === 'desktop' || e2eBuild
  ? adapterStatic({ pages: e2eBuild ? 'build' : buildDirectory, assets: e2eBuild ? 'build' : buildDirectory, fallback: '200.html' })
  : adapterCloudflare()


const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter,
    paths: {
      // Workers serves the SPA from the domain root, so relative module URLs can recurse
      // after an asset miss. Keep every generated chunk URL rooted at `/_app`.
      relative: false,
    },
    alias: {
      '@core-wasm': coreWasmDir,
      '@core-wasm/index': path.resolve(coreWasmDir, 'index.ts'),
    },
  }
}

export default config
