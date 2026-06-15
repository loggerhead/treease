import { execFileSync } from 'node:child_process';
import { copyFileSync } from 'node:fs';

const wasmOptArgs = [
  '-Oz',
  '--enable-bulk-memory',
  '--enable-nontrapping-float-to-int',
  '--strip-debug',
  '--strip-producers',
  '--vacuum',
];

export function optimizeWasmSync(inputPath, outputPath, logger = process.stderr) {
  try {
    execFileSync('wasm-opt', [...wasmOptArgs, inputPath, '-o', outputPath], {
      stdio: 'inherit',
    });
    return { outputPath, optimized: true };
  } catch (error) {
    copyFileSync(inputPath, outputPath);
    logger.write(`[wasm] wasm-opt unavailable or failed; copied raw wasm: ${error.message}\n`);
    return { outputPath, optimized: false };
  }
}
