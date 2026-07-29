import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const moduleUrl = pathToFileURL(path.resolve('scripts', 'release-metadata.mjs')).href;

async function writeManifests(rootDir, { coreVersion, cliVersion, cliCoreDependencyVersion }) {
  await writeFile(
    path.join(rootDir, 'packages', 'core', 'Cargo.toml'),
    `[package]
name = "treease-core"
version = "${coreVersion}"

`,
    'utf8'
  );
  await writeFile(
    path.join(rootDir, 'apps', 'desktop', 'src-tauri', 'Cargo.toml'),
    `[package]
name = "treease-desktop"
version = "0.8.9"
`,
    'utf8'
  );
  await writeFile(
    path.join(rootDir, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'),
    JSON.stringify({ version: '0.8.9' }) + '\n',
    'utf8'
  );
  await writeFile(
    path.join(rootDir, 'apps', 'cli', 'Cargo.toml'),
    `[package]
name = "treease-cli"
version = "${cliVersion}"

[dependencies]
treease-core = { version = "${cliCoreDependencyVersion}", path = "../../packages/core" }
`,
    'utf8'
  );
  await writeFile(path.join(rootDir, 'apps', 'web', 'package.json'), `{"version":"${coreVersion}"}\n`, 'utf8');
}

async function makeFixture(overrides = {}) {
  const rootDir = await mkdtemp(path.join(tmpdir(), 'treease-release-metadata-'));
  const coreDir = path.join(rootDir, 'packages', 'core');
  const cliDir = path.join(rootDir, 'apps', 'cli');
  const webDir = path.join(rootDir, 'apps', 'web');
  const desktopDir = path.join(rootDir, 'apps', 'desktop', 'src-tauri');
  await import('node:fs/promises').then(({ mkdir }) =>
    Promise.all([
      mkdir(coreDir, { recursive: true }),
      mkdir(cliDir, { recursive: true }),
      mkdir(webDir, { recursive: true }),
      mkdir(desktopDir, { recursive: true }),
    ])
  );
  await writeManifests(rootDir, {
    coreVersion: '1.2.3',
    cliVersion: '2.3.4',
    cliCoreDependencyVersion: '1.2.3',
    ...overrides,
  });
  return rootDir;
}

test('loadReleaseMetadata returns normalized release information', async () => {
  const fixtureRoot = await makeFixture();
  const { loadReleaseMetadata } = await import(moduleUrl);

  const metadata = await loadReleaseMetadata(fixtureRoot);

  assert.deepEqual(metadata, {
    cliName: 'treease-cli',
    cliVersion: '2.3.4',
    cliCoreDependencyVersion: '1.2.3',
    coreName: 'treease-core',
    coreVersion: '1.2.3',
    webVersion: '1.2.3',
    coreReleaseTag: 'v1.2.3',
    cliReleaseTag: 'cli-v2.3.4',
    desktopVersion: '0.8.9',
    desktopReleaseTag: 'desktop-v0.8.9',
  });
});

test('loadReleaseMetadata rejects desktop manifest/config version drift', async () => {
  const fixtureRoot = await makeFixture();
  await writeFile(
    path.join(fixtureRoot, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'),
    JSON.stringify({ version: '9.9.9' }) + '\n',
    'utf8'
  );
  const { loadReleaseMetadata } = await import(moduleUrl);

  assert.throws(
    () => loadReleaseMetadata(fixtureRoot),
    /tauri\.conf\.json version 9\.9\.9 does not match apps\/desktop\/src-tauri\/Cargo\.toml version 0\.8\.9/
  );
});

test('loadReleaseMetadata rejects cli/core version drift', async () => {
  const fixtureRoot = await makeFixture({ cliCoreDependencyVersion: '9.9.9' });
  const { loadReleaseMetadata } = await import(moduleUrl);

  assert.throws(
    () => loadReleaseMetadata(fixtureRoot),
    /apps\/cli\/Cargo\.toml treease-core dependency version 9\.9\.9 does not match packages\/core\/Cargo\.toml version 1\.2\.3/
  );
});

test('loadCoreReleaseMetadata returns core version metadata from the manifest single source', async () => {
  const fixtureRoot = await makeFixture({ coreVersion: '7.8.9' });
  const { loadCoreReleaseMetadata } = await import(moduleUrl);

  const metadata = loadCoreReleaseMetadata(fixtureRoot);

  assert.deepEqual(metadata, {
    coreName: 'treease-core',
    coreVersion: '7.8.9',
  });
});

test('synchronizeGeneratedWasmPackageJson rewrites generated package version from core manifest metadata', async () => {
  const { synchronizeGeneratedWasmPackageJson } = await import(moduleUrl);

  const synchronized = synchronizeGeneratedWasmPackageJson(
    JSON.stringify(
      {
        name: 'treease-core',
        version: '0.0.1',
        files: ['core_bg.wasm', 'core.js'],
      },
      null,
      2
    ) + '\n',
    { coreName: 'treease-core', coreVersion: '7.8.9' }
  );

  assert.deepEqual(JSON.parse(synchronized), {
    name: 'treease-core',
    version: '7.8.9',
    files: ['core.wasm', 'core.js'],
  });
});
