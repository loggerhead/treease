import { describe, expect, it } from 'vitest';
import worker from './cloudflare-worker';

describe('Cloudflare asset Worker', () => {
  it('returns a real 404 for a missing JavaScript chunk', async () => {
    const response = worker.fetch(new Request('https://treease.test/_app/immutable/nodes/missing.js'));

    expect(response.status).toBe(404);
    expect(response.headers.get('content-type')).toBe('text/plain; charset=UTF-8');
    expect(response.headers.get('x-content-type-options')).toBe('nosniff');
    await expect(response.text()).resolves.toBe('Not Found');
  });
});
