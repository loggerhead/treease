import { assetUrl, r2Assets } from '$lib/assets';

export const siteOrigin = 'https://treease.com';
export const defaultSocialImage = assetUrl(r2Assets.heroDemoGraphPoster);

export const socialLinks = {
  github: 'https://github.com/loggerhead/treease',
  x: 'https://x.com/1oggerhead',
  discord: 'https://discord.gg/vzM3Jdvav',
} as const;

export function serializeJsonLd(value: unknown): string {
  return JSON.stringify(value)
    .replaceAll('<', '\\u003c')
    .replaceAll('>', '\\u003e')
    .replaceAll('&', '\\u0026');
}
