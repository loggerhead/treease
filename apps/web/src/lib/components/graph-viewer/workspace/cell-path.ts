import { PathSegTag } from '@core-wasm/index';

import type { PathSeg } from '../../../store/tree-path';
import type { GraphCell } from '../../../graph/graph-viewer-render';

export function buildPathSegFromCell(cell: GraphCell | undefined, rowIndex: number): PathSeg | null {
  const raw = String(cell?.text ?? '').trim();
  if (!raw) {
    return cell?.isIndex ? ({ tag: PathSegTag.INDEX, key: '' as any, index: rowIndex } as PathSeg) : null;
  }
  const bracketMatch = raw.match(/^\[(\d+)\]$/);
  if (bracketMatch) {
    return { tag: PathSegTag.INDEX, key: '' as any, index: Number.parseInt(bracketMatch[1], 10) } as PathSeg;
  }
  if (cell?.isIndex && /^\d+$/.test(raw)) {
    return { tag: PathSegTag.INDEX, key: '' as any, index: Number.parseInt(raw, 10) } as PathSeg;
  }
  return { tag: PathSegTag.KEY, key: raw as any, index: 0 } as PathSeg;
}
