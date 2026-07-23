import { expect, test } from '@playwright/test';

test('missing static assets return a real 404 response', async ({ request }) => {
  const response = await request.get('/_app/immutable/nodes/missing.js');

  expect(response.status()).toBe(404);
  expect(response.headers()['content-type'] ?? '').not.toContain('text/html');
  await expect(response.text()).resolves.not.toContain('<html');
});
