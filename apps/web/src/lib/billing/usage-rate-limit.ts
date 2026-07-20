const BACKOFF_DELAYS_MS = [60_000, 120_000, 300_000, 900_000] as const;

let coolingUntil = 0;
let backoffIndex = 0;

export function isUsageCoolingDown(now = Date.now()): boolean {
  return coolingUntil > now;
}

export function noteUsageRateLimit(retryAfterMs?: number, now = Date.now()): void {
  const delay = retryAfterMs ?? BACKOFF_DELAYS_MS[Math.min(backoffIndex, BACKOFF_DELAYS_MS.length - 1)];
  coolingUntil = Math.max(coolingUntil, now + delay);
  if (retryAfterMs === undefined) backoffIndex = Math.min(backoffIndex + 1, BACKOFF_DELAYS_MS.length - 1);
}

export function markUsageRequestSucceeded(): void {
  coolingUntil = 0;
  backoffIndex = 0;
}
