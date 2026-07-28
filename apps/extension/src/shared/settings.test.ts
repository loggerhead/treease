import { beforeEach, describe, expect, it } from 'vitest';
import { getSettings, isOriginEnabled, updateSettings } from './settings';

const values = new Map<string, unknown>();

beforeEach(() => {
  values.clear();
  Object.assign(globalThis, {
    chrome: {
      storage: {
        local: {
          get: async (defaults: Record<string, unknown>) => Object.fromEntries(Object.entries(defaults).map(([key, value]) => [key, values.get(key) ?? value])),
          set: async (next: Record<string, unknown>) => { Object.entries(next).forEach(([key, value]) => values.set(key, value)); },
        },
      },
    },
  });
});

describe('extension settings', () => {
  it('starts disabled until the privacy disclosure is acknowledged', async () => {
    expect(await getSettings()).toMatchObject({ enabled: false, privacyAcknowledged: false, disabledOrigins: [] });
  });

  it('persists only settings and applies site-level pauses', async () => {
    const settings = await updateSettings({ enabled: true, privacyAcknowledged: true, disabledOrigins: ['https://private.example'] });
    expect(isOriginEnabled(settings, 'https://public.example')).toBe(true);
    expect(isOriginEnabled(settings, 'https://private.example')).toBe(false);
    expect(values.has('text')).toBe(false);
  });
});
