import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const fingerprintInputs = [
  ['wasm/document-protocol.generated.ts', 'wasm/document-protocol.generated.ts'],
  ['wasm/pkg/core.js', 'wasm/pkg/core.js'],
  ['wasm/pkg/core.d.ts', 'wasm/pkg/core.d.ts'],
  ['wasm/pkg/core.wasm', 'wasm/pkg/core.wasm'],
  ['wasm/pkg/core.wasm.d.ts', 'wasm/pkg/core.wasm.d.ts'],
  ['wasm/pkg/package.json', 'wasm/pkg/package.json'],
];

export function computeWasmFingerprint(rootDir) {
  const hash = createHash('sha256');
  for (const [relativePath, hashLabel] of fingerprintInputs) {
    const filePath = path.resolve(rootDir, 'packages', 'core', relativePath);
    if (!existsSync(filePath)) {
      throw new Error(`missing WASM fingerprint input: ${filePath}`);
    }
    hash.update(hashLabel);
    hash.update('\0');
    hash.update(readFileSync(filePath));
    hash.update('\0');
  }
  return `sha256-${hash.digest('hex').slice(0, 16)}`;
}
