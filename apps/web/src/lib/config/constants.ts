/**
 * 应用配置常量
 * 集中管理所有硬编码配置值，便于维护和调整
 */

declare const __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__: number;
declare const __TREEASE_WASM_STREAM_CHUNK_TEST__: number;

export function resolveCompileTimeNumber(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : fallback;
}

/** 编辑器相关配置 */
export const EDITOR_CONFIG = {
  /** 限制同时打开的标签页数量，避免编辑器状态、worker 同步和 UI 管理成本持续膨胀。 */
  maxTabs: 9,
} as const;

/** 图形渲染相关配置 */
export const GRAPH_CONFIG = {
  /** 用稳定字符集估算平均字符宽度，影响图节点文本截断、列宽和布局初始值。 */
  measureTextSample: 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789',
} as const;

/** WASM Worker 相关配置 */
export const WASM_CONFIG = {
  /** 流式处理块大小配置 */
  streamChunkSize: {
    /** 生产环境默认 chunk size，用于无法获取 totalBytes 时的 fallback。
     * 主要入口点（graph-render-session.ts）已改用 selectChunkSize(totalBytes) 动态选择。
     * 128KB 是基于 benchmark（5 候选 × 4 size bucket）的全规模最优均衡默认值。 */
    production: resolveCompileTimeNumber(
      typeof __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__ !== 'undefined'
        ? __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__
        : undefined,
      128 * 1024,
    ),
    /** 测试环境固定使用更小分片，便于覆盖多块输出和边界条件。 */
    test: resolveCompileTimeNumber(
      typeof __TREEASE_WASM_STREAM_CHUNK_TEST__ !== 'undefined' ? __TREEASE_WASM_STREAM_CHUNK_TEST__ : undefined,
      16 * 1024,
    ),
  },
} as const;

/** 检测是否为测试环境 */
export const isTestEnv = typeof process !== 'undefined' && process.env?.NODE_ENV === 'test';

/** 获取流式处理块大小（静态 fallback，无法获取 totalBytes 时使用） */
export function getStreamChunkSize(): number {
  return isTestEnv ? WASM_CONFIG.streamChunkSize.test : WASM_CONFIG.streamChunkSize.production;
}
