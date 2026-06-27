import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const defaultLatestPath = path.resolve(webDir, 'build', 'cli-assets', 'latest.json');
const defaultSiteBaseUrl = (process.env.TREEASE_CLI_ASSET_SITE_BASE_URL ?? 'https://treease.com').replace(/\/+$/, '');
const assetVersionPattern =
  /data-treease-cli-asset-version\s*=\s*(?:"([^"]+)"|'([^']+)')/i;

async function main() {
  const latestPath = resolveArg('--latest-path') ?? defaultLatestPath;
  const localLatest = JSON.parse(await readFile(latestPath, 'utf8'));
  const manifestPath = resolveArg('--manifest-path') ?? localLatest.manifestPath;
  const siteBaseUrl = resolveArg('--site-base-url') ?? defaultSiteBaseUrl;
  const localManifestFile = resolveArg('--local-manifest-file')
    ?? path.resolve(path.dirname(latestPath), localLatest.version, 'manifest.json');

  const localManifest = JSON.parse(await readFile(localManifestFile, 'utf8'));
  assertManifestVersion(localLatest.version, localManifest.version, 'local');
  assertAssetVersionPresent(localManifest.assetVersion, 'local manifest');

  const localIndexFile = path.resolve(path.dirname(localManifestFile), 'index.html');
  const localIndexHtml = await readFile(localIndexFile, 'utf8');
  assertIndexAssetVersion(localManifest.assetVersion, readIndexAssetVersion(localIndexHtml), 'local');

  const remoteManifestUrl = `${siteBaseUrl}${manifestPath}`;
  const remoteManifestResponse = await fetch(remoteManifestUrl);
  if (!remoteManifestResponse.ok) {
    throw new Error(`unexpected ${remoteManifestResponse.status} for ${remoteManifestUrl}`);
  }
  const remoteManifest = await remoteManifestResponse.json();
  assertManifestVersion(localManifest.version, remoteManifest.version, 'remote');
  assertAssetVersionMatch(localManifest.assetVersion, remoteManifest.assetVersion, 'remote manifest');

  const remoteIndexUrl = `${siteBaseUrl}/cli-assets/${localManifest.version}/index.html`;
  const remoteIndexResponse = await fetch(remoteIndexUrl);
  if (!remoteIndexResponse.ok) {
    throw new Error(`unexpected ${remoteIndexResponse.status} for ${remoteIndexUrl}`);
  }
  const remoteIndexHtml = await remoteIndexResponse.text();
  assertIndexAssetVersion(localManifest.assetVersion, readIndexAssetVersion(remoteIndexHtml), 'remote');

  process.stdout.write(
    `[check-cli-assets] verified /cli-assets/${localManifest.version} assetVersion ${localManifest.assetVersion}\n`
  );
}

function readIndexAssetVersion(html) {
  const match = html.match(assetVersionPattern);
  return match ? (match[1] ?? match[2] ?? '') : null;
}

function assertManifestVersion(expected, actual, label) {
  if (actual !== expected) {
    throw new Error(`${label} manifest version mismatch: expected=${expected}, actual=${actual}`);
  }
}

function assertAssetVersionPresent(actual, label) {
  if (!actual) {
    throw new Error(`${label} missing assetVersion`);
  }
}

function assertAssetVersionMatch(expected, actual, label) {
  if (actual !== expected) {
    throw new Error(`${label} assetVersion mismatch: expected=${expected}, actual=${actual ?? '<missing>'}`);
  }
}

function assertIndexAssetVersion(expected, actual, label) {
  assertAssetVersionMatch(expected, actual, `${label} index`);
}

function resolveArg(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return null;
  return process.argv[index + 1] ?? null;
}

main().catch((error) => {
  process.stderr.write(`[check-cli-assets] ${error.message}\n`);
  process.exit(1);
});
