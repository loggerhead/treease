/// <reference types="vitest/config" />
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite-plus';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { analyzer } from 'vite-bundle-analyzer';

const configDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(configDir, '../..');
const coreWasmDir = path.resolve(repoRoot, 'packages/core/wasm');
const allowFsDirs = [path.resolve(repoRoot, 'packages/core'), path.resolve(repoRoot, 'example')];
const bundleAnalyzeEnabled = process.env.TREEASE_BUNDLE_ANALYZE === '1';
const bundleAnalyzeMode = 'server';

// 仅接受正整数覆盖，避免 benchmark 时把非法字符串静默带进构建产物。
function readPositiveIntEnv(name: string, fallback: number): number {
  const raw = process.env[name];
  if (!raw) return fallback;
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer, got: ${raw}`);
  }
  return value;
}

// 这批 define 是 Web 侧 benchmark 旋钮：构建时固定数值，业务代码只消费编译结果，不直接读环境变量。
const buildDefines = {
  // 基于 benchmark（src 5 候选 [16KB,32KB,64KB,128KB,256KB]），128KB 是全规模文档最优均衡值。
  __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__: readPositiveIntEnv('VITE_TREEASE_WASM_STREAM_CHUNK_PRODUCTION', 128 * 1024),
  // 测试专用小分片，便于覆盖多块输出路径。
  __TREEASE_WASM_STREAM_CHUNK_TEST__: readPositiveIntEnv('VITE_TREEASE_WASM_STREAM_CHUNK_TEST', 16 * 1024),
  // 浏览器读取导入文件时的切片大小。
  __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__: readPositiveIntEnv('VITE_TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE', 64 * 1024),
  // graph import 累积到该体积后立即 flush。
  __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__: readPositiveIntEnv(
    'VITE_TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD',
    64 * 1024,
  ),
  // 导入文本累计到该体积后立刻刷入编辑器。
  __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__: readPositiveIntEnv(
    'VITE_TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD',
    256 * 1024,
  ),
};

export default defineConfig({
  plugins: [
    sveltekit(),
    tailwindcss(),
    ...(bundleAnalyzeEnabled
      ? [
          analyzer({
            analyzerMode: bundleAnalyzeMode,
            reportTitle: 'Treease Web Bundle Report',
            defaultSizes: 'gzip',
            openAnalyzer: false,
          }),
        ]
      : []),
  ],
  // 以编译期常量注入，保证 benchmark 可复现，避免运行时代码各自解析环境变量。
  define: Object.fromEntries(Object.entries(buildDefines).map(([key, value]) => [key, JSON.stringify(value)])),
  build: bundleAnalyzeEnabled
    ? {
        sourcemap: 'hidden',
      }
    : undefined,
  resolve: {
    alias: [
      {
        find: /^@core-wasm(\/.*)?$/,
        replacement: `${coreWasmDir}$1`,
      },
    ],
  },
  ssr: {
    noExternal: [/^@core-wasm(\/|$)/],
  },
  server: {
    port: 8080,
    hmr: process.env.VITE_BENCHMARK_MODE ? false : undefined,
    fs: {
      allow: allowFsDirs,
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'test/integration/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      exclude: ['src/lib/components/ui/**', 'src/lib/leafer-x-tooltip/**', 'test/**', 'src/**/*_gen.ts'],
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: 'coverage',
      thresholds: {
        statements: 35,
        branches: 38,
        functions: 25,
        lines: 35,
      },
    },
  },
  lint: {
    rules: {
      'no-unassigned-vars': 'off',
    },
  },
});
