/**
 * Application configuration constants.
 * Keep hard-coded configuration values centralized for easier maintenance.
 */

declare const __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__: number;
declare const __TREEASE_WASM_STREAM_CHUNK_TEST__: number;

export function resolveCompileTimeNumber(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : fallback;
}

/** Editor configuration. */
export const EDITOR_CONFIG = {
  /** Limit open tabs so editor state, worker synchronization, and UI management remain bounded. */
  maxTabs: 9,
} as const;

/** Graph rendering configuration. */
export const GRAPH_CONFIG = {
  /** Estimate average character width from a stable character set for node truncation, column widths, and initial layout. */
  measureTextSample: 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789',
} as const;

/** WASM Worker configuration. */
export const WASM_CONFIG = {
  /** Streaming chunk-size configuration. */
  streamChunkSize: {
    /** Production default when totalBytes is unavailable.
     * The main entry point (graph-render-session.ts) dynamically selects a size with selectChunkSize(totalBytes).
     * 128KB is the best overall balance from the benchmark across five candidates and four size buckets. */
    production: resolveCompileTimeNumber(
      typeof __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__ !== 'undefined'
        ? __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__
        : undefined,
      128 * 1024,
    ),
    /** Use smaller fixed chunks in tests to cover multi-chunk output and boundary conditions. */
    test: resolveCompileTimeNumber(
      typeof __TREEASE_WASM_STREAM_CHUNK_TEST__ !== 'undefined' ? __TREEASE_WASM_STREAM_CHUNK_TEST__ : undefined,
      16 * 1024,
    ),
  },
} as const;

/** Detect whether the code is running in a test environment. */
export const isTestEnv = typeof process !== 'undefined' && process.env?.NODE_ENV === 'test';

/** Get the streaming chunk size for the static fallback path. */
export function getStreamChunkSize(): number {
  return isTestEnv ? WASM_CONFIG.streamChunkSize.test : WASM_CONFIG.streamChunkSize.production;
}
