import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const wasmPath = path.resolve(webDir, '../../packages/core/wasm/pkg/core.wasm');
const jsPath = path.resolve(webDir, '../../packages/core/wasm/pkg/core.js');

const wasmBytes = await readFile(wasmPath);
const imports = WebAssembly.Module.imports(new WebAssembly.Module(wasmBytes));
const unresolvedWasmImports = imports.filter(({ module }) => module === 'env');
const generatedJs = await readFile(jsPath, 'utf8');
const unresolvedJsImport = /(?:from|import)\s*["']env["']/.test(generatedJs);

if (unresolvedWasmImports.length > 0 || unresolvedJsImport) {
  console.error('Generated WASM contains unresolved env imports.');
  if (unresolvedWasmImports.length > 0) {
    console.error(JSON.stringify(unresolvedWasmImports, null, 2));
  }
  process.exit(1);
}

console.log(`Validated WASM imports: ${imports.length} wasm-bindgen imports, no env imports.`);
