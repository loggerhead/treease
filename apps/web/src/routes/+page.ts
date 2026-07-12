import { redirect } from '@sveltejs/kit';

export function load(): void {
  if (import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop') {
    redirect(307, '/editor');
  }
}
