import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
const coreDir = path.resolve(rootDir, 'packages', 'core');
const documentProtocolOutput = path.resolve(coreDir, 'wasm', 'document-protocol.generated.ts');
const wasmPkgDir = path.resolve(coreDir, 'wasm', 'pkg');

export const generatedOutputs = [
  documentProtocolOutput,
  path.resolve(wasmPkgDir, 'core.js'),
  path.resolve(wasmPkgDir, 'core.d.ts'),
  path.resolve(wasmPkgDir, 'core.wasm'),
  path.resolve(wasmPkgDir, 'core.wasm.d.ts'),
  path.resolve(wasmPkgDir, 'package.json'),
];

export const watchedFiles = generatedOutputs;

export function runBindgen() {
  // 1. Generate TS types from Rust document protocol
  execFileSync('cargo', ['run', '--locked', '--bin', 'export_document_protocol'], {
    cwd: coreDir,
    stdio: 'inherit'
  });

  // 2. Build WASM with wasm-pack (generates pkg/ with wasm-bindgen bindings)
  execFileSync('wasm-pack', ['build', 'packages/core', '--target', 'web', '--out-dir', 'wasm/pkg', '--out-name', 'core'], {
    cwd: rootDir,
    stdio: 'inherit'
  });

  // 3. Rename core_bg.wasm → core.wasm, core_bg.wasm.d.ts → core.wasm.d.ts
  //    wasm-bindgen always appends _bg to the wasm binary, but we don't want that.
  const bgWasm = path.resolve(wasmPkgDir, 'core_bg.wasm');
  const outWasm = path.resolve(wasmPkgDir, 'core.wasm');
  if (fs.existsSync(bgWasm)) {
    fs.renameSync(bgWasm, outWasm);
  }
  const bgWasmDts = path.resolve(wasmPkgDir, 'core_bg.wasm.d.ts');
  const outWasmDts = path.resolve(wasmPkgDir, 'core.wasm.d.ts');
  if (fs.existsSync(bgWasmDts)) {
    fs.renameSync(bgWasmDts, outWasmDts);
  }

  // 4. 去掉 core.js 中的静态 new URL('...wasm', import.meta.url) 引用，
  //    并用 core.wasm 替换所有 core_bg.wasm 引用。
  const generatedJs = path.resolve(wasmPkgDir, 'core.js');
  let code = fs.readFileSync(generatedJs, 'utf-8');
  code = code
    .replace(
      /module_or_path = new URL\(['"]core_bg\.wasm['"],\s*import\.meta\.url\);/,
      "module_or_path = 'core.wasm';",
    )
    .replace(/'core_bg\.wasm'/g, "'core.wasm'")
    .replace(/core_bg\.wasm\.d\.ts/g, 'core.wasm.d.ts');
  fs.writeFileSync(generatedJs, code, 'utf-8');

  // 5. Update package.json: core_bg.wasm → core.wasm
  const pkgPath = path.resolve(wasmPkgDir, 'package.json');
  let pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));
  if (pkg.files) {
    pkg.files = pkg.files.map((f) => f.replace(/^core_bg\.wasm$/, 'core.wasm'));
  }
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf-8');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runBindgen();
}