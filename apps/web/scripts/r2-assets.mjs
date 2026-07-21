import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const assetSourceDir = path.resolve(webDir, 'static');
const manifestPath = path.resolve(webDir, 'assets', 'r2-manifest.json');

export const assetBaseUrl =
  (process.env.PUBLIC_ASSET_BASE_URL ?? 'https://assets.treease.com').replace(/\/+$/, '');
export const bucketName = process.env.TREEASE_R2_ASSET_BUCKET ?? 'treease-assets';

const contentTypes = new Map([
  ['.png', 'image/png'],
  ['.mp4', 'video/mp4'],
]);

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
const manifestFiles = Object.values(manifest);
if (
  manifestFiles.length === 0 ||
  manifestFiles.some((file) => typeof file !== 'string') ||
  new Set(manifestFiles).size !== manifestFiles.length
) {
  throw new Error(`invalid R2 asset manifest: ${manifestPath}`);
}
for (const relativePath of manifestFiles) getContentType(relativePath);

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
    return null;
  }

  const missing = manifestFiles.filter(
    (file) => !existsSync(path.resolve(assetSourceDir, file)),
  );
  if (missing.length > 0) {
    throw new Error(`missing R2 assets in ${assetSourceDir}: ${missing.join(', ')}`);
  }
  return [...manifestFiles].sort();
}

export { assetSourceDir, webDir };
