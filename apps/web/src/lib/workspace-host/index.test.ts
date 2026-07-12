import { describe, expect, it } from 'vitest';
import { createWorkspaceHost, parseDesktopDeepLinks, parseEditorDeepLinks, resolveWorkspaceSurface } from './index';

describe('resolveWorkspaceSurface', () => {
  it('selects the desktop adapter only for the desktop build surface', () => {
    expect(resolveWorkspaceSurface('desktop')).toBe('desktop');
  });

  it('keeps browser as the default build surface', () => {
    expect(resolveWorkspaceSurface(undefined)).toBe('browser');
  });

  it('exposes file lifecycle operations only through the desktop host', async () => {
    const host = await createWorkspaceHost('desktop');
    expect(host.surface).toBe('desktop');
    expect(host.openFile).toBeTypeOf('function');
    expect(host.saveFile).toBeTypeOf('function');
    expect(host.watchFile).toBeTypeOf('function');
    expect(host.saveSession).toBeTypeOf('function');
    expect(host.takeStartupFiles).toBeTypeOf('function');
    expect(host.onCommand).toBeTypeOf('function');
    expect(host.getInitialDeepLinks).toBeTypeOf('function');
  });

  it('keeps only editor deep links at the shared-host boundary', () => {
    expect(parseEditorDeepLinks(['treease://editor?lang=json', 'treease://auth/callback', 'https://treease.com/editor'])).toEqual([
      new URL('treease://editor?lang=json'),
    ]);
  });

  it('accepts only editor presets and the explicit auth callback desktop routes', () => {
    expect(parseDesktopDeepLinks([
      'treease://editor?lang=json',
      'treease://auth/callback?code=one-time-code',
      'treease://auth/other',
      'treease://settings',
      'https://treease.com/editor',
    ])).toEqual([
      new URL('treease://editor?lang=json'),
      new URL('treease://auth/callback?code=one-time-code'),
    ]);
  });
});
