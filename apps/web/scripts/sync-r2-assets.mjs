import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createR2S3Client } from './r2-s3.mjs';
import { assetSourceDir, bucketName, getContentType, listAssetFiles } from './r2-assets.mjs';

async function main() {
  const files = await listAssetFiles();
  if (files === null) throw new Error(`missing asset source directory: ${assetSourceDir}`);

  const client = createR2S3Client({
    accountId: requiredEnv('R2_ACCOUNT_ID'),
    accessKeyId: requiredEnv('R2_ACCESS_KEY_ID'),
    secretAccessKey: requiredEnv('R2_SECRET_ACCESS_KEY'),
    bucket: bucketName,
  });
  await syncAssets({ client, files });
}

export async function syncAssets({ client, files, sourceDir = assetSourceDir }) {
  for (const relativePath of files) {
    const sourcePath = path.resolve(sourceDir, relativePath);
    const body = await readFile(sourcePath);
    const sha256 = createHash('sha256').update(body).digest('hex');
    const remoteSha256 = await client.headObject(relativePath);
    if (!shouldUpload(remoteSha256, sha256)) {
      process.stdout.write(`[assets:r2:sync] unchanged ${relativePath}\n`);
      continue;
    }

    process.stdout.write(`[assets:r2:sync] upload ${relativePath}\n`);
    await client.putObject({
      key: relativePath,
      body,
      contentType: getContentType(relativePath),
      cacheControl: 'public, max-age=3600',
      sha256,
    });
  }
}

export function shouldUpload(remoteSha256, localSha256) {
  return remoteSha256 !== localSha256;
}

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`[assets:r2:sync] missing required environment variable: ${name}`);
  return value;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  });
}
