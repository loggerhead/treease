type WasmModule = typeof import('@core-wasm/pkg');

let wasmModule: WasmModule | null = null;

export async function initWasm(options?: {
  wasmURL?: string;
  wasmModule?: WebAssembly.Module;
  wasmBytes?: ArrayBuffer;
}) {
  if (wasmModule) return;
  if (options?.wasmBytes) {
    const mod = await import('@core-wasm/pkg');
    mod.initSync({ module: options.wasmBytes });
    mod.init_wasm();
    wasmModule = mod;
    return;
  }

  const mod = await import('@core-wasm/pkg');
  await mod.default({ module_or_path: options?.wasmURL });
  mod.init_wasm();
  wasmModule = mod;
}

export async function callWasm<T>(fn: (mod: WasmModule) => T): Promise<T> {
  await ensureModule();
  return fn(wasmModule!);
}

export type ChunkSizeConfig = {
  defaultChunkSize: number;
  largeFileThreshold: number;
  largeFileChunkSize: number;
};

export async function getChunkSizeConfig(): Promise<ChunkSizeConfig> {
  return callWasm((mod) => mod.get_chunk_size_config() as unknown as ChunkSizeConfig);
}

async function ensureModule(): Promise<void> {
  if (wasmModule) return;
  await initWasm();
}
