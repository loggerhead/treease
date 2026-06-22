import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { assetSourceDir, bucketName, getContentType, listAssetFiles, webDir } from './r2-assets.mjs';

async function main() {
  const files = await listAssetFiles();
  if (files.length === 0) {
    throw new Error(`no asset files found in ${assetSourceDir}`);
  }

  for (const relativePath of files) {
    const sourcePath = path.resolve(assetSourceDir, relativePath);
    const contentType = getContentType(relativePath);
    process.stdout.write(`[assets:r2:upload] ${relativePath}\n`);
    run([
      'wrangler',
      'r2',
      'object',
      'put',
      `${bucketName}/${relativePath}`,
      '--file',
      sourcePath,
      '--content-type',
      contentType,
      '--cache-control',
      'public, max-age=3600',
      '--remote',
    ]);
  }
}

function run(args) {
  const result = spawnSync('npx', args, {
    cwd: webDir,
    stdio: 'inherit',
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

main().catch((error) => {
  process.stderr.write(`[assets:r2:upload] ${error.message}\n`);
  process.exit(1);
});
