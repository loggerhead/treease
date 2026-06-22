import { listAssetFiles, toAssetUrl } from './r2-assets.mjs';

async function main() {
  const files = await listAssetFiles();
  if (files.length === 0) {
    throw new Error('no asset files found to verify');
  }

  for (const relativePath of files) {
    const url = toAssetUrl(relativePath);
    process.stdout.write(`[assets:r2:check] ${url}\n`);
    const response = await fetch(url, { method: 'HEAD' });
    if (!response.ok) {
      throw new Error(`unexpected ${response.status} for ${url}`);
    }
  }
}

main().catch((error) => {
  process.stderr.write(`[assets:r2:check] ${error.message}\n`);
  process.exit(1);
});
