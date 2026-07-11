import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../src/lib/wasm/wasm-worker-singleton';
import { initWasmWorkerForTests, shutdownWasmWorkerForTests } from '../wasm-test-helpers';

describe('Worker transport integration', () => {
  beforeAll(async () => {
    await initWasmWorkerForTests();
  }, 5_000);

  afterAll(async () => {
    await shutdownWasmWorkerForTests();
  });

  it('keeps concurrent requests correlated while the Worker serializes their execution', async () => {
    const [formatted, minified, sorted] = await Promise.all([
      callSharedWasmWorker<string>('format', { language: 'json', text: '{"b":2,"a":1}' }),
      callSharedWasmWorker<string>('minify', { language: 'json', text: '{\n  "a": 1\n}' }),
      callSharedWasmWorker<string>('sort', { language: 'json', text: '{"b":2,"a":1}' }),
    ]);

    expect(formatted).toContain('"b": 2');
    expect(minified.trim()).toBe('{"a":1}');
    expect(JSON.parse(sorted)).toEqual({ a: 1, b: 2 });
    expect(sorted.indexOf('"a"')).toBeLessThan(sorted.indexOf('"b"'));
  });

  it('returns a visible error response for an unregistered request and keeps later requests usable', async () => {
    const client = await getSharedWasmWorkerClient();

    await expect(client.call('notRegistered' as never)).rejects.toThrow('Unhandled worker message type: notRegistered');

    await expect(callSharedWasmWorker<string>('minify', { language: 'json', text: '{ "ok": true }' })).resolves.toMatch(/^\{"ok":true\}\s*$/);
  });
});
