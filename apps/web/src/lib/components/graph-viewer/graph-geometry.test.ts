import { describe, expect, it, vi } from 'vitest';
import {
  getClientProbeCoordFromBoxLike,
  getClientRectFromWorldRect,
  getWorldRectFromBoxLike,
} from '@treease/graph-viewer-runtime';

describe('graph-geometry', () => {
  it('resolves world rects through the nearest Leafer world transform', () => {
    const worldRoot = {
      getWorldPointByBox: ({ x, y }: { x: number; y: number }) => ({ x: x + 100, y: y + 50 }),
    };
    const box = {
      x: 10,
      y: 5,
      width: 30,
      height: 20,
      parent: worldRoot,
    };

    expect(getWorldRectFromBoxLike(box as any)).toEqual({
      left: 110,
      top: 55,
      width: 30,
      height: 20,
    });
  });

  it('maps world rects into client rects through Leafer APIs', () => {
    const updateClientBounds = vi.fn();
    const app = {
      updateClientBounds,
      getClientPointByWorld: ({ x, y }: { x: number; y: number }) => ({ x: x * 2, y: y * 3 }),
    };

    expect(
      getClientRectFromWorldRect(
        {
          left: 10,
          top: 20,
          width: 30,
          height: 40,
        },
        app as any,
      ),
    ).toEqual({
      left: 20,
      top: 60,
      width: 60,
      height: 120,
    });
    expect(updateClientBounds).toHaveBeenCalledTimes(1);
  });

  it('falls back to the client rect center when probe conversion cannot use the center point directly', () => {
    const box = {
      width: 30,
      height: 20,
      getWorldPointByBox: ({ x, y }: { x: number; y: number }) => ({ x: x + 10, y: y + 20 }),
    };
    const app = {
      updateClientBounds: vi.fn(),
      getClientPointByWorld: ({ x, y }: { x: number; y: number }) => {
        if (x === 25 && y === 30) return { x: Number.NaN, y: Number.NaN };
        return { x: x * 2, y: y * 2 };
      },
    };

    expect(getClientProbeCoordFromBoxLike(box as any, app as any)).toEqual({
      x: 50,
      y: 60,
    });
  });
});
