import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const moduleUrl = pathToFileURL(path.resolve('scripts', 'treease-web-local.mjs')).href;

async function makeFixture() {
  const rootDir = await mkdtemp(path.join(tmpdir(), 'treease-web-local-'));
  const coreDir = path.join(rootDir, 'packages', 'core');
  const cliAssetsDir = path.join(rootDir, 'apps', 'web', 'build', 'cli-assets');
  const versionDir = path.join(cliAssetsDir, '26070206');
  await mkdir(coreDir, { recursive: true });
  await mkdir(versionDir, { recursive: true });
  await writeFile(
    path.join(coreDir, 'Cargo.toml'),
    `[package]
name = "treease-core"
version = "1.0.5"

[package.metadata.treease]
wasm_release_date = "26070206"
`,
    'utf8'
  );
  await writeFile(path.join(cliAssetsDir, 'latest.json'), '{"version":"26070206"}\n', 'utf8');
  await writeFile(path.join(versionDir, 'manifest.json'), '{"version":"26070206","files":[]}\n', 'utf8');
  await writeFile(path.join(versionDir, 'index.html'), '<!doctype html><html></html>\n', 'utf8');
  return { rootDir, cliAssetsDir, versionDir };
}

test('resolveLocalCliAssetConfig returns local asset base url and isolated cache dir', async () => {
  const { resolveLocalCliAssetConfig } = await import(moduleUrl);
  const { rootDir, cliAssetsDir, versionDir } = await makeFixture();

  const config = await resolveLocalCliAssetConfig({
    rootDir,
    port: 4317,
    runToken: 'test-run',
  });

  assert.equal(config.cliAssetsDir, cliAssetsDir);
  assert.equal(config.assetBaseUrl, 'http://127.0.0.1:4317');
  assert.equal(config.latestPath, path.join(cliAssetsDir, 'latest.json'));
  assert.equal(config.versionDir, versionDir);
  assert.equal(config.manifestPath, path.join(versionDir, 'manifest.json'));
  assert.equal(config.wasmReleaseDate, '26070206');
  assert.equal(config.cacheDir, path.join(rootDir, '.tmp', 'treease-web-local-test-run'));
});

test('resolveLocalCliAssetConfig rejects missing cli-assets build output with actionable error', async () => {
  const { resolveLocalCliAssetConfig } = await import(moduleUrl);
  const rootDir = await mkdtemp(path.join(tmpdir(), 'treease-web-local-empty-'));
  const coreDir = path.join(rootDir, 'packages', 'core');
  await mkdir(coreDir, { recursive: true });
  await writeFile(
    path.join(coreDir, 'Cargo.toml'),
    `[package]
name = "treease-core"
version = "1.0.5"

[package.metadata.treease]
wasm_release_date = "26070206"
`,
    'utf8'
  );

  await assert.rejects(
    () => resolveLocalCliAssetConfig({ rootDir, port: 4317, runToken: 'missing' }),
    /missing local cli-assets build output.*pnpm --dir apps\/web build/s
  );
});

test('resolveLocalCliAssetConfig rejects missing wasm-matched bundle with actionable error', async () => {
  const { resolveLocalCliAssetConfig } = await import(moduleUrl);
  const { rootDir, cliAssetsDir } = await makeFixture();
  await writeFile(path.join(cliAssetsDir, 'latest.json'), '{"version":"26062907"}\n', 'utf8');

  await import('node:fs/promises').then(({ rm }) => rm(path.join(cliAssetsDir, '26070206'), { recursive: true, force: true }));

  await assert.rejects(
    () => resolveLocalCliAssetConfig({ rootDir, port: 4317, runToken: 'missing-version' }),
    /missing local cli-assets bundle for wasm_release_date 26070206.*pnpm --dir apps\/web build/s
  );
});

test('startStaticFileServer serves local cli-assets files and blocks path traversal', async () => {
  const { startStaticFileServer } = await import(moduleUrl);
  const { cliAssetsDir, versionDir } = await makeFixture();
  const nestedDir = path.join(versionDir, '_app');
  await mkdir(nestedDir, { recursive: true });
  await writeFile(path.join(nestedDir, 'app.js'), 'console.log("treease");\n', 'utf8');

  const server = await startStaticFileServer({ rootDir: cliAssetsDir, port: 0 });
  try {
    const assetResponse = await fetch(`${server.origin}/26070206/_app/app.js`);
    assert.equal(assetResponse.status, 200);
    assert.equal(await assetResponse.text(), 'console.log("treease");\n');

    const forbiddenResponse = await fetch(`${server.origin}/../package.json`);
    assert.equal(forbiddenResponse.status, 404);
  } finally {
    await server.close();
  }
});

test('writeEnvFile writes local asset overrides for manual shell usage', async () => {
  const { resolveLocalCliAssetConfig, writeEnvFile } = await import(moduleUrl);
  const { rootDir } = await makeFixture();
  const config = await resolveLocalCliAssetConfig({
    rootDir,
    port: 4317,
    runToken: 'env-file',
  });
  const envFile = path.join(rootDir, '.tmp', 'treease-web-local.env');

  await writeEnvFile(envFile, config);

  const contents = await readFile(envFile, 'utf8');
  assert.match(contents, /^TREEASE_WEB_ASSET_BASE_URL=http:\/\/127\.0\.0\.1:4317$/m);
  assert.match(contents, /^TREEASE_WEB_CACHE_DIR=.*treease-web-local-env-file$/m);
});
