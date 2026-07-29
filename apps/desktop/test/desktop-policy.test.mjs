import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

const config = JSON.parse(await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
const capability = JSON.parse(await readFile(new URL('../src-tauri/capabilities/default.json', import.meta.url), 'utf8'));
const releaseWorkflow = await readFile(new URL('../../../.github/workflows/desktop-release.yml', import.meta.url), 'utf8');

test('desktop CSP permits only the local bundle, approved service endpoints, and HTTPS images', () => {
  const csp = config.app.security.csp;
  assert.match(csp, /script-src 'self'/);
  assert.doesNotMatch(csp, /script-src[^;]*https:/);
  assert.match(csp, /connect-src[^;]*https:\/\/api\.treease\.com/);
  assert.match(csp, /connect-src[^;]*https:\/\/\*\.supabase\.co/);
  assert.match(csp, /img-src[^;]*https:/);
  assert.match(csp, /frame-src 'none'/);
  assert.match(csp, /worker-src 'self' blob:/);
});

test('production capability excludes embedded WebDriver permissions', () => {
  assert.deepEqual(config.app.security.capabilities, ['default']);
  assert.ok(capability.permissions.every((permission) => !permission.startsWith('wdio')));
  assert.ok(capability.permissions.includes('updater:default'));
});

test('updater accepts only signed GitHub Release metadata and keeps Windows interactive', () => {
  const updater = config.plugins.updater;
  assert.match(updater.pubkey, /^[A-Za-z0-9+/=]+$/);
  assert.deepEqual(updater.endpoints, ['https://github.com/loggerhead/treease/releases/latest/download/latest.json']);
  assert.equal(updater.windows.installMode, 'basicUi');
  assert.equal(config.bundle.createUpdaterArtifacts, true);
});

test('desktop release stays isolated from unified releases and uploads updater metadata', () => {
  assert.match(releaseWorkflow, /tags: \["desktop-v\*"\]/);
  assert.match(releaseWorkflow, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(releaseWorkflow, /uploadUpdaterJson: true/);
  assert.match(releaseWorkflow, /uploadUpdaterSignatures: true/);
  assert.match(releaseWorkflow, /releaseDraft: false/);
  assert.match(releaseWorkflow, /windows-latest/);
  assert.match(releaseWorkflow, /macos-latest/);
  assert.doesNotMatch(releaseWorkflow, /tags:\s*\["v\*"\]/);
});
