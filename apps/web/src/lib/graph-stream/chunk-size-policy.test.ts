import { describe, expect, it } from 'vitest';
import { selectGraphStreamChunkSize } from './chunk-size-policy';

describe('chunk-size-policy', () => {
  it('uses 128KB for documents below 256KB', () => {
    expect(selectGraphStreamChunkSize(128 * 1024)).toBe(128 * 1024);
  });

  it('uses 64KB for documents from 256KB up to 1MB', () => {
    expect(selectGraphStreamChunkSize(512 * 1024)).toBe(64 * 1024);
  });

  it('uses 128KB for documents from 1MB up to 4MB', () => {
    expect(selectGraphStreamChunkSize(2 * 1024 * 1024)).toBe(128 * 1024);
  });

  it('uses 256KB for documents at least 4MB', () => {
    expect(selectGraphStreamChunkSize(8 * 1024 * 1024)).toBe(256 * 1024);
  });

  it('uses the default chunk size for unknown totals', () => {
    expect(selectGraphStreamChunkSize(Number.NaN, 32 * 1024)).toBe(32 * 1024);
  });
});
