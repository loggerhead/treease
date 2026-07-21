import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { computeWasmFingerprint } from './wasm-fingerprint.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
process.stdout.write(computeWasmFingerprint(rootDir));
