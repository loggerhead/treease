import type { ExtensionSettings } from './types';

export const defaultSettings: ExtensionSettings = {
  enabled: false,
  disabledOrigins: [],
  allowlist: [],
  blocklist: ['*://treease.com/*', '*://*.treease.com/*'],
  theme: 'system',
  privacyAcknowledged: false,
};

export async function getSettings(): Promise<ExtensionSettings> {
  const stored = await chrome.storage.local.get(defaultSettings);
  return {
    enabled: stored.enabled === true,
    disabledOrigins: Array.isArray(stored.disabledOrigins)
      ? stored.disabledOrigins.filter((origin): origin is string => typeof origin === 'string')
      : [],
    allowlist: sanitizePatterns(stored.allowlist),
    blocklist: sanitizePatterns(stored.blocklist, defaultSettings.blocklist),
    theme: stored.theme === 'light' || stored.theme === 'dark' ? stored.theme : 'system',
    privacyAcknowledged: stored.privacyAcknowledged === true,
  };
}

export async function updateSettings(patch: Partial<ExtensionSettings>): Promise<ExtensionSettings> {
  const current = await getSettings();
  const next: ExtensionSettings = {
    ...current,
    ...patch,
    disabledOrigins: patch.disabledOrigins ?? current.disabledOrigins,
    allowlist: patch.allowlist ?? current.allowlist,
    blocklist: patch.blocklist ?? current.blocklist,
  };
  await chrome.storage.local.set(next);
  return next;
}

function sanitizePatterns(value: unknown, fallback: string[] = []): string[] {
  return Array.isArray(value) ? value.filter((pattern): pattern is string => typeof pattern === 'string' && pattern.length > 0).slice(0, 64) : fallback;
}

export function isOriginEnabled(settings: ExtensionSettings, pageUrl: string): boolean {
  const origin = new URL(pageUrl).origin;
  if (!settings.enabled || !settings.privacyAcknowledged || settings.disabledOrigins.includes(origin)) return false;
  if (matchesAny(settings.blocklist, pageUrl)) return false;
  return settings.allowlist.length === 0 || matchesAny(settings.allowlist, pageUrl);
}

export function matchesAny(patterns: string[], pageUrl: string): boolean {
  return patterns.some((pattern) => {
    try {
      const expression = '^' + pattern.replace(/[.+^${}()|[\\]\\]/g, '\\$&').replace(/\\\*/g, '.*') + '$';
      return new RegExp(expression, 'i').test(pageUrl);
    } catch { return false; }
  });
}
