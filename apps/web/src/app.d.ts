declare global {
  namespace App {}

  // 常规文档的默认 wasm stream 分片大小，主要影响 graph/analyze 的跨 worker 往返频率。
  const __TREEASE_WASM_STREAM_CHUNK_PRODUCTION__: number
  // 测试环境专用的小分片，用于稳定覆盖多块输出和边界路径。
  const __TREEASE_WASM_STREAM_CHUNK_TEST__: number
  // 浏览器读取导入文件时的切片大小，影响单次读取成本和 graph import 输入粒度。
  const __TREEASE_IMPORT_FILE_CHUNK_BYTE_SIZE__: number
  // graph import 在 shared wasm worker 侧累计到该体积后立刻 flush，影响增量出图频率。
  const __TREEASE_IMPORT_GRAPH_STREAM_FLUSH_BYTE_THRESHOLD__: number
  // 导入文本累计到该体积后立即刷入编辑器，影响可见内容延迟和 Monaco 重排压力。
  const __TREEASE_IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD__: number
}

declare module 'fuzzysort'

export {}
