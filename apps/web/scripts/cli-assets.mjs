import { copyFile, mkdir, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
const coreManifest = path.resolve(rootDir, 'packages', 'core', 'Cargo.toml');
const buildDir = path.resolve(webDir, 'build');
const cliAssetsRoot = path.resolve(buildDir, 'cli-assets');

async function main() {
  const version = readManifestReleaseDate();
  if (!existsSync(buildDir)) {
    throw new Error(`missing build output: ${buildDir}`);
  }

  const versionDir = path.resolve(cliAssetsRoot, version);
  await rm(versionDir, { recursive: true, force: true });
  await mkdir(versionDir, { recursive: true });

  const files = [];
  await copyBuildTree(buildDir, versionDir, files);
  files.sort((left, right) => left.path.localeCompare(right.path));

  const manifest = {
    version,
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

async function copyBuildTree(sourceDir, targetDir, files, relativePrefix = '') {
  const entries = await readdir(sourceDir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;
    if (relativePrefix === '' && entry.name === 'cli-assets') continue;

    const sourcePath = path.resolve(sourceDir, entry.name);
    const relativePath = relativePrefix ? `${relativePrefix}/${entry.name}` : entry.name;
    const targetPath = path.resolve(targetDir, relativePath);

    if (entry.isDirectory()) {
      await mkdir(targetPath, { recursive: true });
      await copyBuildTree(sourcePath, targetDir, files, relativePath);
      continue;
    }
    if (!entry.isFile()) continue;

    await mkdir(path.dirname(targetPath), { recursive: true });
    await copyFile(sourcePath, targetPath);
    const bytes = await readFile(sourcePath);
    const size = (await stat(sourcePath)).size;
    files.push({
      path: relativePath.replaceAll(path.sep, '/'),
      sha256: createHash('sha256').update(bytes).digest('hex'),
      size,
    });
  }
}

main().catch((error) => {
  process.stderr.write(`[cli-assets] ${error.message}\n`);
  process.exit(1);
});
