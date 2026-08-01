import * as Sentry from '@sentry/browser';
import packageJson from '../../../package.json' with { type: 'json' };
import { authUser } from '../auth/auth-user-store';
import { getUsageClientId } from '../billing/client-id';

const dsn = String(import.meta.env.PUBLIC_SENTRY_DSN ?? '').trim();
const release = `treease-web@${packageJson.version}`;
const appType = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop' ? 'desktop' : 'web';

export const sentryEnabled = Boolean(dsn);

if (sentryEnabled) {
  Sentry.init({
    dsn,
    environment: import.meta.env.MODE,
    release,
    tracesSampleRate: 0,
    sendDefaultPii: false,
    beforeSend(event) {
      // Never send editor contents, auth headers, or uploaded files as error context.
      if (event.request) {
        delete event.request.cookies;
        delete event.request.headers;
        delete event.request.data;
      }
      return event;
    },
  });

  Sentry.setTag('app_type', appType);
  authUser.subscribe((user) => {
    Sentry.setUser(user ? { id: user.id } : null);
  });
  void getUsageClientId()
    .then((clientId) => {
      Sentry.setTag('client_id', clientId);
    })
    .catch(() => {
      // Client identity is best-effort and must not affect application startup.
    });
}

export function captureFrontendException(
  error: unknown,
  context?: { route?: string; requestId?: string; status?: number; code?: string | null },
): void {
  if (!sentryEnabled) return;
  Sentry.withScope((scope) => {
    if (context?.route) scope.setTag('route', context.route);
    if (context?.status !== undefined) scope.setTag('http.status_code', String(context.status));
    if (context?.code) scope.setTag('error.code', context.code);
    if (context?.requestId) scope.setTag('request_id', context.requestId);
    Sentry.captureException(error);
  });
}
