import { cp, mkdir, readdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const buildImmutableDir = path.resolve(webDir, 'build/_app/immutable');
const historyDir = path.resolve(process.env.TREEASE_IMMUTABLE_ASSET_HISTORY_DIR ?? path.resolve(webDir, '.cache/immutable-assets'));
const historyLimit = Number.parseInt(process.env.TREEASE_IMMUTABLE_ASSET_HISTORY_LIMIT ?? '3', 10);
const snapshotName = process.env.TREEASE_IMMUTABLE_ASSET_SNAPSHOT ?? `${Date.now()}`;

if (!Number.isInteger(historyLimit) || historyLimit < 1) {
  throw new Error('TREEASE_IMMUTABLE_ASSET_HISTORY_LIMIT must be a positive integer');
}
if (!existsSync(buildImmutableDir)) {
  throw new Error(`missing immutable asset directory: ${buildImmutableDir}`);
}

await mkdir(historyDir, { recursive: true });
const snapshots = (await readdir(historyDir, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort()
  .reverse();

for (const snapshot of snapshots) {
  await cp(path.resolve(historyDir, snapshot), buildImmutableDir, { recursive: true, force: false });
}

const snapshotDir = path.resolve(historyDir, snapshotName);
await rm(snapshotDir, { recursive: true, force: true });
await cp(buildImmutableDir, snapshotDir, { recursive: true });

for (const oldSnapshot of snapshots.slice(historyLimit - 1)) {
  await rm(path.resolve(historyDir, oldSnapshot), { recursive: true, force: true });
}

process.stdout.write(`[immutable-assets] retained ${snapshots.length + 1} deployment snapshots\n`);
