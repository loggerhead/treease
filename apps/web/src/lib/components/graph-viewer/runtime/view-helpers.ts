import type { Box } from 'leafer-ui';

import { getClientProbeCoordFromBoxLike } from '../rendering';
import type { LeaferAppLike, LeaferBox } from '../model';

type LeaferContentHost = LeaferAppLike & { add?: (...args: unknown[]) => unknown };
export function getLeaferContentRoot(target: LeaferContentHost | null): Box | null {
  if (!target) return null;
  const zoomLayer = (target as LeaferContentHost & { zoomLayer?: Box }).zoomLayer;
  if (zoomLayer) return zoomLayer;
  return 'add' in target ? (target as unknown as Box) : null;
}

export function buildClientProbeCoord(
  box: LeaferBox,
  leafer: LeaferAppLike | null,
  container: HTMLElement | null | undefined,
): { x: number; y: number } | null {
  const absoluteProbe = getClientProbeCoordFromBoxLike(box, leafer);
  const containerRect = container?.getBoundingClientRect();
  if (!absoluteProbe || !containerRect) return null;
  return {
    x: Math.round(absoluteProbe.x - containerRect.left),
    y: Math.round(absoluteProbe.y - containerRect.top),
  };
}

export function dispatchGraphEditEvent(
  container: HTMLDivElement | null | undefined,
  type: 'graph-edit-open' | 'graph-edit-commit' | 'graph-edit-replace-fallback' | 'graph-edit-result' | 'graph-edit-probes',
  detail: unknown,
): void {
  if (!container) return;
  container.dispatchEvent(new CustomEvent(type, { detail, bubbles: true }));
}

export async function exportLeaferImage(
  leafer:
    | ({
        export?: (name: string, options?: { trim?: boolean; padding?: [number, number, number, number] }) => Promise<void>;
      } & object)
    | null,
): Promise<void> {
  if (!leafer || typeof leafer.export !== 'function') return;
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  await leafer.export(`treease-${timestamp}.png`, { trim: true, padding: [5, 5, 5, 5] });
}
