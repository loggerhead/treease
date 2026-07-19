import type { UsageCapability } from '../services/treease-server';

export const USAGE_FINGERPRINT_SAMPLE_BYTES = 1024;

function toHex(bytes: ArrayBuffer): string {
  return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

export function usageFingerprintSample(source: string): string {
  const bytes = new TextEncoder().encode(source);
  return new TextDecoder().decode(bytes.slice(0, USAGE_FINGERPRINT_SAMPLE_BYTES)).replace(/\s+/g, '');
}

export async function createUsageIdempotencyKey(
  capability: Exclude<UsageCapability, 'ai_suggestion'>,
  source: string,
): Promise<string> {
  const normalizedSource = usageFingerprintSample(source);
  const input = new TextEncoder().encode(`${capability}\0${normalizedSource}`);
  const digest = await crypto.subtle.digest('SHA-256', input);
  return `${capability}:${toHex(digest)}`;
}
