import { getStreamChunkSize } from '../config/constants';

const KB = 1024;
const MB = 1024 * KB;

export function selectGraphStreamChunkSize(
  totalBytes: number,
  defaultChunkSize = getStreamChunkSize(),
): number {
  const fallbackChunkSize = Math.max(1, Math.trunc(defaultChunkSize));
  if (!Number.isFinite(totalBytes) || totalBytes < 0) {
    return fallbackChunkSize;
  }
  const size = Math.trunc(totalBytes);
  if (size < 256 * KB) return 128 * KB;
  if (size < MB) return 64 * KB;
  if (size < 4 * MB) return 128 * KB;
  if (size > 10 * MB) return MB;
  return 256 * KB;
}
