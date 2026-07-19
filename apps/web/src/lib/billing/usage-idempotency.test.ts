import { describe, expect, it } from 'vitest';

import { createUsageIdempotencyKey, usageFingerprintSample } from './usage-idempotency';

describe('usage idempotency', () => {
  it('uses the same 1 KiB source window as language guessing, then removes whitespace', () => {
    expect(usageFingerprintSample(' a\n b\t c ')).toBe('abc');
    expect(new TextEncoder().encode(usageFingerprintSample(`${'x'.repeat(1024)} tail`)).byteLength).toBe(1024);
  });

  it('deduplicates whitespace-only changes but keeps capabilities separate', async () => {
    await expect(createUsageIdempotencyKey('large_file_processing', '{ "a": 1 }')).resolves.toBe(
      await createUsageIdempotencyKey('large_file_processing', '{"a":1}'),
    );
    await expect(createUsageIdempotencyKey('bidirectional_edit', '{"a":1}')).resolves.not.toBe(
      await createUsageIdempotencyKey('large_file_processing', '{"a":1}'),
    );
  });
});
