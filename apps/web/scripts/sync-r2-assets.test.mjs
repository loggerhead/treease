import test from 'node:test';
import assert from 'node:assert/strict';
import { buildBulkManifest, groupManifestByContentType } from './sync-r2-assets.mjs';

test('builds a Wrangler bulk manifest with absolute source paths', () => {
  const manifest = buildBulkManifest({
    files: ['treease-logo.png', 'landing/hero-demo-graph.mp4'],
    sourceDir: '/tmp/treease-static',
  });

  assert.deepEqual(manifest, [
    { key: 'treease-logo.png', file: '/tmp/treease-static/treease-logo.png' },
    { key: 'landing/hero-demo-graph.mp4', file: '/tmp/treease-static/landing/hero-demo-graph.mp4' },
  ]);
});

test('groups bulk manifest entries by content type', () => {
  const groups = groupManifestByContentType([
    { key: 'one.png', file: '/tmp/one.png' },
    { key: 'two.mp4', file: '/tmp/two.mp4' },
    { key: 'three.png', file: '/tmp/three.png' },
  ]);

  assert.deepEqual([...groups], [
    ['image/png', [
      { key: 'one.png', file: '/tmp/one.png' },
      { key: 'three.png', file: '/tmp/three.png' },
    ]],
    ['video/mp4', [{ key: 'two.mp4', file: '/tmp/two.mp4' }]],
  ]);
});
