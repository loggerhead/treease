import { execFileSync } from 'node:child_process';
import { cp, mkdir, readdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
const coreDir = path.resolve(rootDir, 'packages', 'core');
const litePkgDir = path.resolve(coreDir, 'wasm', 'lite-pkg');
const tmpDir = path.resolve(coreDir, 'wasm', '.lite-pkg-tmp');

async function main() {
  // 1. Build lite wasm via wasm-pack (produces JS bindings + optimized wasm)
  execFileSync(
    'wasm-pack',
    ['build', coreDir, '--target', 'web', '--out-dir', tmpDir, '--release', '--out-name', 'core-lite', '--', '--features', 'lite'],
    { cwd: rootDir, stdio: 'inherit', shell: true },
  );

  // 2. Rename core-lite_bg.wasm → core-lite.wasm, core-lite_bg.wasm.d.ts → core-lite.wasm.d.ts
  const bgWasm = path.join(tmpDir, 'core-lite_bg.wasm');
  const outWasm = path.join(tmpDir, 'core-lite.wasm');
  try { await rename(bgWasm, outWasm); } catch {}
  const bgWasmDts = path.join(tmpDir, 'core-lite_bg.wasm.d.ts');
  const outWasmDts = path.join(tmpDir, 'core-lite.wasm.d.ts');
  try { await rename(bgWasmDts, outWasmDts); } catch {}

  // 3. Update JS references from core-lite_bg.wasm to core-lite.wasm
  const jsPath = path.join(tmpDir, 'core-lite.js');
  try {
    let code = await readFile(jsPath, 'utf-8');
    code = code.replace(/'core-lite_bg\.wasm'/g, "'core-lite.wasm'");
    await writeFile(jsPath, code, 'utf-8');
  } catch {}

  // 4. Copy files to lite-pkg
  await mkdir(litePkgDir, { recursive: true });
  const files = await readdir(tmpDir);
  for (const file of files) {
    if (file !== '.gitignore') {
      await cp(path.join(tmpDir, file), path.join(litePkgDir, file), { recursive: true, force: true });
    }
  }

  // 5. Clean up tmp
  await rm(tmpDir, { recursive: true, force: true });

  process.stdout.write(`[wasm:lite-pkg] lite wasm built → ${litePkgDir}\n`);
}

main().catch((err) => {
  process.stderr.write(`[wasm:lite-pkg] failed: ${err.message}\n`);
  process.exitCode = 1;
});