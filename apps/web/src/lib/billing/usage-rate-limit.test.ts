import { beforeEach, describe, expect, it } from 'vitest';
import { isUsageCoolingDown, markUsageRequestSucceeded, noteUsageRateLimit } from './usage-rate-limit';

describe('usage rate limit', () => {
  beforeEach(() => markUsageRequestSucceeded());

  it('uses Retry-After when the server provides it', () => {
    noteUsageRateLimit(2_000, 1_000);

    expect(isUsageCoolingDown(2_999)).toBe(true);
    expect(isUsageCoolingDown(3_000)).toBe(false);
  });

  it('backs off from one minute to two minutes after repeated 429s', () => {
    noteUsageRateLimit(undefined, 1_000);
    expect(isUsageCoolingDown(60_999)).toBe(true);
    markUsageRequestSucceeded();

    noteUsageRateLimit(undefined, 1_000);
    noteUsageRateLimit(undefined, 61_000);
    expect(isUsageCoolingDown(180_999)).toBe(true);
    expect(isUsageCoolingDown(181_000)).toBe(false);
  });

  it('resets the backoff after a successful request', () => {
    noteUsageRateLimit(undefined, 1_000);
    markUsageRequestSucceeded();
    noteUsageRateLimit(undefined, 1_000);

    expect(isUsageCoolingDown(60_999)).toBe(true);
    expect(isUsageCoolingDown(61_000)).toBe(false);
  });
});
