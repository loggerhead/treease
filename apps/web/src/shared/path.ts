import type { PathSeg } from '@core-wasm/index';

export function pathSegKeyValue(seg: PathSeg): string {
  return seg.key;
}
