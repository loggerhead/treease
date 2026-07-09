export type ShareResourceType = 'editor_text_snapshot' | 'command_run';

export type ShareResource = {
  type: ShareResourceType;
  payload: Record<string, unknown>;
};

export type ShareLink = {
  slug: string;
  shareUrl: string;
  expiresAt: string;
  createdAt: string;
};

const apiOrigin = import.meta.env.PROD ? 'https://api.treease.com' : 'http://localhost:3000';

function getAccessToken(): string | null {
  if (typeof window === 'undefined') return null;
  const explicitToken = window.localStorage.getItem('treease.accessToken');
  if (explicitToken) return explicitToken;

  // Supabase stores browser sessions under sb-<project-ref>-auth-token.
  for (let index = 0; index < window.localStorage.length; index += 1) {
    const key = window.localStorage.key(index);
    if (!key?.startsWith('sb-') || !key.endsWith('-auth-token')) continue;
    try {
      const session = JSON.parse(window.localStorage.getItem(key) ?? '') as { access_token?: unknown };
      if (typeof session.access_token === 'string' && session.access_token) return session.access_token;
    } catch {
      // Ignore unrelated or stale local-storage entries.
    }
  }
  return null;
}

async function readError(response: Response): Promise<Error> {
  let message = `Treease server request failed (${response.status})`;
  try {
    const body = (await response.json()) as { message?: string; error?: string };
    message = body.message || body.error || message;
  } catch {
    // Keep the HTTP status when the server does not return JSON.
  }
  return new Error(message);
}

export async function createShareLink(resource: ShareResource, expiresInDays = 7): Promise<ShareLink> {
  const token = getAccessToken();
  if (!token) throw new Error('请先登录 Treease，再创建分享链接。');

  const response = await fetch(`${apiOrigin}/v1/share-links`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ resource, expiresInDays }),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as ShareLink;
}

export type PublicShare = {
  slug: string;
  resourceType: ShareResourceType;
  resourcePayload: Record<string, unknown>;
  expiresAt: string;
  createdAt: string;
};

export async function getPublicShare(slug: string): Promise<PublicShare> {
  const response = await fetch(`${apiOrigin}/v1/public/shares/${encodeURIComponent(slug)}`);
  if (!response.ok) throw await readError(response);
  return (await response.json()) as PublicShare;
}
