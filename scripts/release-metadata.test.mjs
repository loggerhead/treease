import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const moduleUrl = pathToFileURL(path.resolve('scripts', 'release-metadata.mjs')).href;

async function writeManifests(rootDir, { coreVersion, cliVersion, wasmReleaseDate, cliCoreDependencyVersion }) {
  await writeFile(
    path.join(rootDir, 'packages', 'core', 'Cargo.toml'),
    `[package]
name = "treease-core"
version = "${coreVersion}"

[package.metadata.treease]
wasm_release_date = "${wasmReleaseDate}"
`,
    'utf8'
  );
  await writeFile(
    path.join(rootDir, 'apps', 'cli', 'Cargo.toml'),
    `[package]
name = "treease-cli"
version = "${cliVersion}"

[package.metadata.treease]
wasm_release_date = "${wasmReleaseDate}"

[dependencies]
treease-core = { version = "${cliCoreDependencyVersion}", path = "../../packages/core" }
`,
    'utf8'
  );
}

async function makeFixture(overrides = {}) {
  const rootDir = await mkdtemp(path.join(tmpdir(), 'treease-release-metadata-'));
  const coreDir = path.join(rootDir, 'packages', 'core');
  const cliDir = path.join(rootDir, 'apps', 'cli');
  await import('node:fs/promises').then(({ mkdir }) =>
    Promise.all([mkdir(coreDir, { recursive: true }), mkdir(cliDir, { recursive: true })])
  );
  await writeManifests(rootDir, {
    coreVersion: '1.2.3',
    cliVersion: '2.3.4',
    wasmReleaseDate: '26063009',
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
    cliWasmReleaseDate: '26063009',
    cliCoreDependencyVersion: '1.2.3',
    coreName: 'treease-core',
    coreVersion: '1.2.3',
    coreWasmReleaseDate: '26063009',
    releaseTag: 'treease-v2.3.4',
  });
});

test('loadReleaseMetadata rejects cli/core version drift', async () => {
  const fixtureRoot = await makeFixture({ cliCoreDependencyVersion: '9.9.9' });
  const { loadReleaseMetadata } = await import(moduleUrl);

  await assert.rejects(
    () => loadReleaseMetadata(fixtureRoot),
    /apps\/cli\/Cargo\.toml treease-core dependency version 9\.9\.9 does not match packages\/core\/Cargo\.toml version 1\.2\.3/
  );
});

test('loadReleaseMetadata rejects wasm release date drift', async () => {
  const fixtureRoot = await makeFixture();
  await writeFile(
    path.join(fixtureRoot, 'apps', 'cli', 'Cargo.toml'),
    `[package]
name = "treease-cli"
version = "2.3.4"

[package.metadata.treease]
wasm_release_date = "26070101"

[dependencies]
treease-core = { version = "1.2.3", path = "../../packages/core" }
`,
    'utf8'
  );
  const { loadReleaseMetadata } = await import(moduleUrl);

  await assert.rejects(
    () => loadReleaseMetadata(fixtureRoot),
    /apps\/cli\/Cargo\.toml wasm_release_date 26070101 does not match packages\/core\/Cargo\.toml wasm_release_date 26063009/
  );
});
