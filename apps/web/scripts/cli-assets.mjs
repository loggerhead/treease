import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
const coreManifest = path.resolve(rootDir, 'packages', 'core', 'Cargo.toml');
const buildDir = path.resolve(webDir, 'build');
const cliAssetsRoot = path.resolve(buildDir, 'cli-assets');
const indexAssetAttribute = 'data-treease-cli-asset-version';

async function main() {
  const version = readManifestReleaseDate();
  const assetVersion = readAssetVersion();
  if (!existsSync(buildDir)) {
    throw new Error(`missing build output: ${buildDir}`);
  }

  const versionDir = path.resolve(cliAssetsRoot, version);
  await rm(versionDir, { recursive: true, force: true });
  await mkdir(versionDir, { recursive: true });

  const files = [];
  await copyBuildTree(buildDir, versionDir, files, assetVersion);
  files.sort((left, right) => left.path.localeCompare(right.path));

  const manifest = {
    version,
    assetVersion,
    generatedAt: new Date().toISOString(),
    files,
  };

  await writeFile(path.resolve(versionDir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  await writeFile(
    path.resolve(cliAssetsRoot, 'latest.json'),
    `${JSON.stringify({ version, manifestPath: `/cli-assets/${version}/manifest.json` }, null, 2)}\n`,
    'utf8'
  );
  process.stdout.write(`[cli-assets] wrote /cli-assets/${version}\n`);
}

function readManifestReleaseDate() {
  if (!existsSync(coreManifest)) {
    throw new Error(`missing core manifest: ${coreManifest}`);
  }
  const manifest = readFileSync(coreManifest, 'utf8');
  const match = manifest.match(/^\s*wasm_release_date\s*=\s*"([0-9]{8})"\s*$/m);
  if (!match) {
    throw new Error(`missing package.metadata.treease.wasm_release_date in ${coreManifest}`);
  }
  return match[1];
}

function readAssetVersion() {
  const override = process.env.TREEASE_CLI_ASSET_VERSION;
  if (override) {
    return override;
  }

  const result = spawnSync('git', ['log', '-1', '--format=%ct'], {
    cwd: rootDir,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    throw new Error(`failed to read git commit timestamp: ${result.stderr.trim() || result.stdout.trim()}`);
  }

  const seconds = result.stdout.trim();
  if (!/^\d+$/.test(seconds)) {
    throw new Error(`invalid git commit timestamp: ${seconds}`);
  }
  return `${seconds}000`;
}

async function copyBuildTree(sourceDir, targetDir, files, assetVersion, relativePrefix = '') {
  const entries = await readdir(sourceDir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;
    if (relativePrefix === '' && entry.name === 'cli-assets') continue;

    const sourcePath = path.resolve(sourceDir, entry.name);
    const relativePath = relativePrefix ? `${relativePrefix}/${entry.name}` : entry.name;
    const targetPath = path.resolve(targetDir, relativePath);

    if (entry.isDirectory()) {
      await mkdir(targetPath, { recursive: true });
      await copyBuildTree(sourcePath, targetDir, files, assetVersion, relativePath);
      continue;
    }
    if (!entry.isFile()) continue;

    await mkdir(path.dirname(targetPath), { recursive: true });
    const sourceBytes = await readFile(sourcePath);
    const outputBytes =
      relativePath === 'index.html' ? injectAssetVersionAttribute(sourceBytes, assetVersion) : sourceBytes;
    await writeFile(targetPath, outputBytes);
    files.push({
      path: relativePath.replaceAll(path.sep, '/'),
    });
  }
}

function injectAssetVersionAttribute(bytes, assetVersion) {
  const html = bytes.toString('utf8');
  const updated = html.replace(
    /<html\b([^>]*)>/i,
    (_match, attrs) => `<html${attrs} ${indexAssetAttribute}="${assetVersion}">`
  );
  if (updated === html) {
    throw new Error('failed to inject cli asset version into index.html');
  }
  return Buffer.from(updated, 'utf8');
}

main().catch((error) => {
  process.stderr.write(`[cli-assets] ${error.message}\n`);
  process.exit(1);
});
