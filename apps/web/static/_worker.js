const assetPathPattern = /\.(?:css|js|json|map|mjs|wasm|woff2?|ttf|png|jpe?g|gif|svg|webp|ico)$/i;
const htmlCacheControl = 'public, max-age=0, must-revalidate';
const immutableCacheControl = 'public, max-age=31536000, immutable';

function isStaticAssetPath(pathname) {
  return pathname.startsWith('/_app/immutable/') || assetPathPattern.test(pathname);
}

function isDocumentRequest(request) {
  return request.method === 'GET' && (request.headers.get('accept') ?? '').includes('text/html');
}

function withCacheControl(response, pathname) {
  if (response.status >= 400) return response;
  const headers = new Headers(response.headers);
  if (pathname === '/' || pathname.endsWith('.html') || response.headers.get('content-type')?.includes('text/html')) {
    headers.set('Cache-Control', htmlCacheControl);
  } else if (pathname.startsWith('/_app/immutable/')) {
    headers.set('Cache-Control', immutableCacheControl);
  } else if (pathname === '/_app/env.js' || pathname === '/_app/version.json') {
    headers.set('Cache-Control', htmlCacheControl);
  } else if (pathname === '/treease-logo.png' || pathname.startsWith('/landing/')) {
    headers.set('Cache-Control', 'public, max-age=86400');
  }
  return new Response(response.body, { status: response.status, headers });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const response = await env.ASSETS.fetch(request);
    const contentType = response.headers.get('content-type') ?? '';

    // Pages' SPA fallback can turn a missing chunk into 200.html. Never expose that HTML as a script/style response.
    if (isStaticAssetPath(url.pathname) && (response.status === 404 || contentType.includes('text/html'))) {
      return new Response('Not Found', {
        status: 404,
        headers: {
          'Cache-Control': 'no-store',
          'Content-Type': 'text/plain; charset=UTF-8',
          'X-Content-Type-Options': 'nosniff',
        },
      });
    }

    // Only navigations may receive the SPA shell. Assets must preserve their missing status.
    if (response.status === 404 && isDocumentRequest(request)) {
      const shell = await env.ASSETS.fetch(new Request(new URL('/200.html', request.url)));
      return withCacheControl(shell, '/200.html');
    }

    return withCacheControl(response, url.pathname);
  },
};
