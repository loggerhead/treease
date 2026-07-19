declare global {
  namespace App {}

  // Default WASM stream chunk size for regular documents; affects graph/analyze worker round trips.
  const __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__: number
  // Smaller test-only chunks for stable coverage of multi-chunk output and boundary paths.
  const __TREEASE_WASM_STREAM_CHUNK_TEST__: number
  // Browser import-file read chunk size; affects per-read cost and graph-import input granularity.
  const __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__: number
  // Flush graph-import data from the shared WASM worker at this threshold; affects incremental graph frequency.
  const __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__: number
  // Flush imported text into the editor at this threshold; affects visible latency and Monaco reflow pressure.
  const __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__: number
}

declare module 'fuzzysort'

export {}
