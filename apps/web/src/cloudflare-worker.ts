export default {
  fetch(_: Request): Response {
    // SPA navigations are served by Static Assets. A missing asset reaches this Worker instead;
    // returning 404 here prevents an HTML shell from being parsed as a JS or CSS module.
    return new Response('Not Found', {
      status: 404,
      headers: {
        'Cache-Control': 'no-store',
        'Content-Type': 'text/plain; charset=UTF-8',
        'X-Content-Type-Options': 'nosniff',
      },
    });
  },
};
