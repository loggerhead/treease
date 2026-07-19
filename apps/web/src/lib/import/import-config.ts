declare const __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__: number;
declare const __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__: number;
declare const __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__: number;
import { resolveCompileTimeNumber } from '../config/constants';

// Slice imported files into chunks for browser reads; affects per-read cost and file full-edit input granularity.
export const IMPORT_FILE_CHUNK_BYTE_SIZE = resolveCompileTimeNumber(
  typeof __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__ !== 'undefined' ? __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__ : undefined,
  128 * 1024,
);

// Flush the imported graph stream from the shared WASM worker at this size; affects incremental graph frequency and worker round trips.
export const IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD = resolveCompileTimeNumber(
  typeof __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__ !== 'undefined'
    ? __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__
    : undefined,
  64 * 1024,
);
// Flush imported text into Monaco at this size; affects visible latency and editor reflow pressure.
export const IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD = resolveCompileTimeNumber(
  typeof __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__ !== 'undefined'
    ? __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__
    : undefined,
  256 * 1024,
);

const MB = 1024 * 1024;

export function selectImportEditorFlushByteThreshold(totalBytes: number): number {
  const size = Number.isFinite(totalBytes) ? Math.max(0, Math.trunc(totalBytes)) : 0;
  if (size < MB) return IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD;
  if (size < 10 * MB) return Math.max(IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD, MB);
  return Math.max(IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD, 4 * MB);
}
