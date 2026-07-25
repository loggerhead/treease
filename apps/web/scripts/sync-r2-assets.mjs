import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { assetSourceDir, bucketName, getContentType, listAssetFiles } from './r2-assets.mjs';

// Cloudflare's experimental remote bulk endpoint intermittently returns 400s for concurrent puts.
// Serializing the small asset set keeps deployment deterministic.
const bulkConcurrency = 1;
const cacheControl = 'public, max-age=3600';
const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function buildBulkManifest({ files, sourceDir = assetSourceDir }) {
  return files.map((relativePath) => ({
    key: relativePath,
    file: path.resolve(sourceDir, relativePath),
  }));
}

export function groupManifestByContentType(manifest) {
  const groups = new Map();
  for (const entry of manifest) {
    const contentType = getContentType(entry.key);
    const entries = groups.get(contentType) ?? [];
    entries.push(entry);
    groups.set(contentType, entries);
  }
  return groups;
}

async function main() {
  const files = await listAssetFiles();
  if (files === null) throw new Error(`missing asset source directory: ${assetSourceDir}`);

  const manifest = buildBulkManifest({ files });
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'treease-r2-'));
  try {
    for (const [contentType, entries] of groupManifestByContentType(manifest)) {
      await uploadGroup({ bucket: bucketName, contentType, entries, tempDir });
    }
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function uploadGroup({ bucket, contentType, entries, tempDir }) {
  const manifestPath = path.join(tempDir, `${contentType.replace('/', '-')}.json`);
  await writeFile(manifestPath, `${JSON.stringify(entries)}\n`, 'utf8');
  await run('pnpm', [
    'exec',
    'wrangler',
    'r2',
    'bulk',
    'put',
    bucket,
    '--filename',
    manifestPath,
    '--remote',
    '--force',
    '--concurrency',
    String(bulkConcurrency),
    '--content-type',
    contentType,
    '--cache-control',
    cacheControl,
  ]);
}

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: webDir, stdio: 'inherit' });
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (signal) {
        reject(new Error(`${command} terminated by ${signal}`));
      } else if (code !== 0) {
        reject(new Error(`${command} exited with status ${code}`));
      } else {
        resolve();
      }
    });
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`[assets:r2:sync] ${error.message}\n`);
    process.exit(1);
  });
}
