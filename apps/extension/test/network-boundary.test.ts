import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const testDir = path.dirname(fileURLToPath(import.meta.url));
const sourceDir = path.resolve(testDir, '../src');

describe('local processing boundary', () => {
  it('does not add a webpage-data network client', async () => {
    const files = [
      'background/service-worker.ts',
      'content/index.ts',
      'sidepanel/index.ts',
      'sidepanel/graph.worker.ts',
    ];
    const source = await Promise.all(files.map((file) => readFile(path.join(sourceDir, file), 'utf8')));
    expect(source.join('\n')).not.toMatch(/\b(fetch|XMLHttpRequest|WebSocket|sendBeacon)\s*\(/);
  });

  it('keeps the requested Chrome permissions minimal', async () => {
    const manifest = JSON.parse(await readFile(path.resolve(testDir, '../public/manifest.json'), 'utf8')) as {
      permissions: string[];
      host_permissions: string[];
      content_security_policy: { extension_pages: string };
      content_scripts: Array<{ all_frames?: boolean }>;
      name: string;
    };
    expect(manifest.permissions).toEqual(['sidePanel', 'storage']);
    expect(manifest.name).toBe('Treease');
    expect(manifest.host_permissions).toEqual(['<all_urls>']);
    expect(manifest.content_security_policy).toEqual({
      extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'",
    });
    // Treease intentionally supports the top-level document only. In particular,
    // do not inject into sandboxed iframes with opaque origins.
    expect(manifest.content_scripts[0]?.all_frames).not.toBe(true);
  });
});
