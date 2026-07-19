import assert from 'node:assert/strict';
import worker from '../static/_worker.js';

const pageHtml = '<!doctype html><script type="module" src="./_app/immutable/entry/start.js"></script>';

function request(path, accept = '*/*') {
  return new Request(`https://treease.test${path}`, { headers: { accept } });
}

async function responseFor(request) {
  const pathname = new URL(request.url).pathname;
  if (pathname === '/200.html') {
    return new Response(pageHtml, { headers: { 'content-type': 'text/html; charset=utf-8' } });
  }
  return new Response(pageHtml, {
    headers: {
      'content-type': 'text/html; charset=utf-8',
      link: '<./_app/immutable/entry/start.js>; rel="modulepreload"',
    },
  });
}

const env = { ASSETS: { fetch: responseFor } };

const missingAsset = await worker.fetch(request('/_app/immutable/entry/missing.js'), env);
assert.equal(missingAsset.status, 404);
assert.equal(missingAsset.headers.get('content-type'), 'text/plain; charset=UTF-8');
assert.equal(missingAsset.headers.get('link'), null);

const route = await worker.fetch(request('/editor', 'text/html'), env);
assert.equal(route.status, 200);
assert.equal(route.headers.get('cache-control'), 'public, max-age=0, must-revalidate');
