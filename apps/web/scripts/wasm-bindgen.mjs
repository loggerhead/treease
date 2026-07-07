import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { optimizeWasmSync } from './wasm-optimize.mjs';
import { loadCoreReleaseMetadata, synchronizeGeneratedWasmPackageJson } from '../../../scripts/release-metadata.mjs';

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
  const { coreName, coreVersion } = loadCoreReleaseMetadata(rootDir);

  // 1. Generate TS types from Rust document protocol
  execFileSync('cargo', ['run', '--locked', '--bin', 'export_document_protocol'], {
    cwd: coreDir,
    stdio: 'inherit'
  });

  // 2. Build WASM with wasm-pack (generates pkg/ with wasm-bindgen bindings)
  execFileSync('wasm-pack', ['build', '.', '--target', 'web', '--out-dir', 'wasm/pkg', '--out-name', 'core'], {
    cwd: coreDir,
    stdio: 'inherit'
  });

  // 3. Optimize core_bg.wasm ourselves; wasm-pack's built-in wasm-opt invocation
  //    does not reliably preserve our feature flags on the current toolchain.
  const bgWasm = path.resolve(wasmPkgDir, 'core_bg.wasm');
  const outWasm = path.resolve(wasmPkgDir, 'core.wasm');
  if (fs.existsSync(bgWasm)) {
    optimizeWasmSync(bgWasm, outWasm);
    fs.rmSync(bgWasm, { force: true });
  }

  // 4. Rename core_bg.wasm.d.ts → core.wasm.d.ts.
  const bgWasmDts = path.resolve(wasmPkgDir, 'core_bg.wasm.d.ts');
  const outWasmDts = path.resolve(wasmPkgDir, 'core.wasm.d.ts');
  if (fs.existsSync(bgWasmDts)) {
    fs.renameSync(bgWasmDts, outWasmDts);
  }

  // 4.5. wasm-pack may also emit a helper file named core_bg.js even though
  //      our rewritten entry point no longer needs it. Remove it so pkg/ only
  //      exposes the normalized core.* surface.
  const bgJs = path.resolve(wasmPkgDir, 'core_bg.js');
  if (fs.existsSync(bgJs)) {
    fs.rmSync(bgJs, { force: true });
  }

  // 5. 去掉 core.js 中的静态 new URL('...wasm', import.meta.url) 引用，
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

  // 6. Rewrite generated package metadata from the Cargo manifest single source.
  const pkgPath = path.resolve(wasmPkgDir, 'package.json');
  const nextPkg = synchronizeGeneratedWasmPackageJson(fs.readFileSync(pkgPath, 'utf-8'), {
    coreName,
    coreVersion,
  });
  fs.writeFileSync(pkgPath, nextPkg, 'utf-8');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runBindgen();
}
