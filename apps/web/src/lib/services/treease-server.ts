import { getSupabaseClient, getSupabaseConfiguration } from '../auth/supabase-auth';
import { workspaceHost } from '../workspace-host';

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

async function getAccessToken(): Promise<string | null> {
  const host = await workspaceHost;
  if (host.surface === 'desktop') {
    const { url, anonKey } = getSupabaseConfiguration();
    return host.refreshAccessToken(url, anonKey);
  }
  const { data, error } = await getSupabaseClient().auth.getSession();
  if (error) throw error;
  return data.session?.access_token ?? null;
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
  const token = await getAccessToken();
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
