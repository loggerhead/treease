import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { assetSourceDir } from './r2-assets.mjs';
import { shouldUpload, syncAssets } from './sync-r2-assets.mjs';

test('uploads an asset when the remote object is absent', () => {
  assert.equal(shouldUpload(null, 'local-hash'), true);
});

test('skips an asset when the stored hash is unchanged', () => {
  assert.equal(shouldUpload('same-hash', 'same-hash'), false);
});

test('uploads an asset when its content hash changed', () => {
  assert.equal(shouldUpload('old-hash', 'new-hash'), true);
});

test('syncs only assets whose remote hash is absent or changed', async () => {
  const relativePath = 'treease-logo.png';
  const body = await readFile(`${assetSourceDir}/${relativePath}`);
  const localHash = createHash('sha256').update(body).digest('hex');
  const uploaded = [];
  const client = {
    async headObject(key) {
      return key === relativePath ? localHash : null;
    },
    async putObject(input) {
      uploaded.push(input.key);
    },
  };

  await syncAssets({ client, files: [relativePath] });
  assert.deepEqual(uploaded, []);

  client.headObject = async () => null;
  await syncAssets({ client, files: [relativePath] });
  assert.deepEqual(uploaded, [relativePath]);
});
