import type { WorkspaceHost, WorkspaceSurface } from './contract';
import { browserWorkspaceHost } from './browser-host';

export * from './contract';
export { browserWorkspaceHost } from './browser-host';

export function resolveWorkspaceSurface(value: string | undefined): WorkspaceSurface {
  return value === 'desktop' ? 'desktop' : 'browser';
}

export async function createWorkspaceHost(surface = resolveWorkspaceSurface(import.meta.env.PUBLIC_WORKSPACE_SURFACE)): Promise<WorkspaceHost> {
  if (surface === 'browser') return browserWorkspaceHost;
  const { desktopWorkspaceHost } = await import('./desktop-host');
  return desktopWorkspaceHost;
}

export const workspaceHost = createWorkspaceHost();
