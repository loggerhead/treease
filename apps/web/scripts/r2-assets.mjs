import { readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const assetSourceDir = path.resolve(webDir, 'assets', 'r2');

export const assetBaseUrl =
  (process.env.PUBLIC_ASSET_BASE_URL ?? 'https://assets.treease.com').replace(/\/+$/, '');
export const bucketName = process.env.TREEASE_R2_ASSET_BUCKET ?? 'treease-assets';

const contentTypes = new Map([
  ['.png', 'image/png'],
  ['.mp4', 'video/mp4'],
]);

export function toAssetUrl(relativePath) {
  return `${assetBaseUrl}/${relativePath}`;
}

export function getContentType(relativePath) {
  const ext = path.extname(relativePath).toLowerCase();
  const contentType = contentTypes.get(ext);
  if (!contentType) {
    throw new Error(`unsupported asset extension for ${relativePath}`);
  }
  return contentType;
}

export async function listAssetFiles() {
  if (!existsSync(assetSourceDir)) {
    throw new Error(`missing asset source directory: ${assetSourceDir}`);
  }

  const files = [];
  await walk(assetSourceDir, files);
  files.sort((left, right) => left.localeCompare(right));
  return files;
}

async function walk(currentDir, files, relativePrefix = '') {
  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;

    const nextRelative = relativePrefix ? `${relativePrefix}/${entry.name}` : entry.name;
    const nextPath = path.resolve(currentDir, entry.name);

    if (entry.isDirectory()) {
      await walk(nextPath, files, nextRelative);
      continue;
    }
    if (!entry.isFile()) continue;

    getContentType(nextRelative);
    files.push(nextRelative.replaceAll(path.sep, '/'));
  }
}

export { assetSourceDir, webDir };
