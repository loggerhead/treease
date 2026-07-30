import type { Box, Text } from 'leafer-ui';

import { getClientProbeCoordFromBoxLike, getZoomScale } from '../rendering';
import type { LeaferAppLike, LeaferBox } from '../model';

type LeaferContentHost = LeaferAppLike & { add?: (...args: unknown[]) => unknown };
type LeaferZoomLayerLike = {
  x?: number;
  y?: number;
  scaleX?: number;
  scaleY?: number;
};

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

export function ensureCanvasHintOverlay(
  container: HTMLDivElement | null | undefined,
  leafer: LeaferAppLike | null,
  overlayLayer: Box | null,
  TextCtor: typeof Text | undefined,
  canvasHintText: Text | null,
): Text | null {
  if (!container || !leafer || !overlayLayer || !TextCtor) return canvasHintText;
  if (canvasHintText && canvasHintText.parent === overlayLayer) return canvasHintText;
  const zoomLayer = (leafer as LeaferAppLike & { zoomLayer?: LeaferZoomLayerLike }).zoomLayer;
  if (!zoomLayer) return canvasHintText;
  const { scaleX, scaleY } = getZoomScale(zoomLayer as never);
  if (!Number.isFinite(scaleX) || !Number.isFinite(scaleY) || scaleX <= 0 || scaleY <= 0) return canvasHintText;
  const hintWidth = 320;
  const offsetX = zoomLayer.x ?? 0;
  const offsetY = zoomLayer.y ?? 0;
  const worldX = (container.clientWidth / 2 - offsetX) / scaleX - hintWidth / 2;
  const worldY = (16 - offsetY) / scaleY;
  const hint = new TextCtor({
    text: 'Hold Space and drag to move the canvas',
    width: hintWidth,
    fontSize: 12,
    fontWeight: '500',
    textAlign: 'center',
    fill: '#94a3b8',
    hittable: false,
    hitSelf: false,
    hitChildren: false,
  });
  hint.x = worldX;
  hint.y = worldY;
  overlayLayer.add(hint);
  return hint;
}

export function clearCanvasHintOverlay(canvasHintText: Text | null): Text | null {
  canvasHintText?.remove?.();
  return null;
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
