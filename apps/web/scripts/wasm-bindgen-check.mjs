import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { generatedOutputs, runBindgen, webDir } from './wasm-bindgen.mjs';

const tempDir = mkdtempSync(path.join(os.tmpdir(), 'treease-wasm-bindgen-check-'));

try {
  const snapshots = generatedOutputs.map((filePath, index) => {
    const snapshotPath = path.join(tempDir, `${index}-${path.basename(filePath)}`);
    writeFileSync(snapshotPath, existsSync(filePath) ? readFileSync(filePath, 'utf8') : '');
    return { filePath, snapshotPath };
  });

  runBindgen();

  const changed = snapshots.filter(({ filePath, snapshotPath }) => {
    const next = existsSync(filePath) ? readFileSync(filePath, 'utf8') : '';
    return next !== readFileSync(snapshotPath, 'utf8');
  });

  if (changed.length > 0) {
    for (const { filePath, snapshotPath } of changed) {
      try {
        execFileSync('git', ['diff', '--no-index', '--', snapshotPath, filePath], {
          cwd: webDir,
          stdio: 'inherit'
        });
      } catch (error) {
        if (error?.status !== 1) {
          throw error;
        }
      }
    }
    process.exitCode = 1;
  }
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
