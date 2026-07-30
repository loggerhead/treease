import { captureFrontendException } from '$lib/observability/sentry';

export function handleError({ error, event }: { error: unknown; event: { url: URL } }): void {
  captureFrontendException(error, { route: event.url.pathname });
}
