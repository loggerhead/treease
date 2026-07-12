declare const __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__: number;
declare const __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__: number;
declare const __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__: number;
import { resolveCompileTimeNumber } from '../config/constants';

// 浏览器读取导入文件时按块切片，影响单次读取开销和后续 file full-edit 的输入粒度。
export const IMPORT_FILE_CHUNK_BYTE_SIZE = resolveCompileTimeNumber(
  typeof __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__ !== 'undefined' ? __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__ : undefined,
  128 * 1024,
);

// 导入图流在 shared wasm worker 侧累计到该体积后立刻 flush，影响增量出图频率与 worker 往返次数。
export const IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD = resolveCompileTimeNumber(
  typeof __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__ !== 'undefined'
    ? __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__
    : undefined,
  64 * 1024,
);
// 导入文本累计到该体积后立即刷入 Monaco，影响用户看到内容的及时性与编辑器重排压力。
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
