import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const moduleUrl = pathToFileURL(path.resolve(path.dirname(fileURLToPath(import.meta.url)), 'wasm-fingerprint.mjs')).href;

async function makeFixture() {
  const rootDir = await mkdtemp(path.join(tmpdir(), 'treease-wasm-fingerprint-'));
  const coreDir = path.join(rootDir, 'packages', 'core');
  await mkdir(path.join(coreDir, 'wasm', 'pkg'), { recursive: true });
  await writeFile(path.join(coreDir, 'wasm', 'document-protocol.generated.ts'), 'export const protocol = 1;\n');
  await writeFile(path.join(coreDir, 'wasm', 'pkg', 'core.js'), 'export const core = 1;\n');
  await writeFile(path.join(coreDir, 'wasm', 'pkg', 'core.d.ts'), 'export declare const core: number;\n');
  await writeFile(path.join(coreDir, 'wasm', 'pkg', 'core.wasm'), Buffer.from([0, 97, 115, 109, 1]));
  await writeFile(path.join(coreDir, 'wasm', 'pkg', 'core.wasm.d.ts'), 'export default function init(): void;\n');
  await writeFile(path.join(coreDir, 'wasm', 'pkg', 'package.json'), '{"name":"treease-core"}\n');
  return { rootDir, wasmPath: path.join(coreDir, 'wasm', 'pkg', 'core.wasm') };
}

test('WASM fingerprint is stable until a runtime input changes', async () => {
  const { computeWasmFingerprint } = await import(moduleUrl);
  const { rootDir, wasmPath } = await makeFixture();

  const first = computeWasmFingerprint(rootDir);
  assert.equal(computeWasmFingerprint(rootDir), first);

  await writeFile(wasmPath, Buffer.from([0, 97, 115, 109, 2]));
  assert.notEqual(computeWasmFingerprint(rootDir), first);
});
