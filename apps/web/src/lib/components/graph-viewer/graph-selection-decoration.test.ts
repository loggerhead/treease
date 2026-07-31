import { describe, expect, it, vi } from 'vitest';
import { createGraphSelectionDecorationController } from './graph-selection-decoration';

type MockDecoration = { visible: boolean };

describe('graph selection decoration', () => {
  it('activates renderer-owned row and cell decoration slots', () => {
    const rowDecoration: MockDecoration = { visible: false };
    const cellDecoration: MockDecoration = { visible: false };
    const resolveDecorations = vi.fn(() => [rowDecoration, cellDecoration]);
    const selection = createGraphSelectionDecorationController({ resolveDecorations: resolveDecorations as any });
    const highlight = { path: [{ tag: 0, key: 'img', index: 0 }], revision: 1, source: 'search' as const };

    selection.sync(highlight);

    expect(resolveDecorations).toHaveBeenCalledWith(highlight);
    expect(rowDecoration.visible).toBe(true);
    expect(cellDecoration.visible).toBe(true);
    selection.clear();
    expect(rowDecoration.visible).toBe(false);
    expect(cellDecoration.visible).toBe(false);
  });

  it('deactivates the old slots before activating the next selection', () => {
    const first: MockDecoration = { visible: false };
    const second: MockDecoration = { visible: false };
    const resolveDecorations = vi.fn()
      .mockReturnValueOnce([first])
      .mockReturnValueOnce([second]);
    const selection = createGraphSelectionDecorationController({ resolveDecorations: resolveDecorations as any });

    selection.sync({ path: [{ tag: 0, key: 'first', index: 0 }], revision: 1, source: 'breadcrumb' });
    selection.sync({ path: [{ tag: 0, key: 'second', index: 0 }], revision: 1, source: 'breadcrumb' });

    expect(first.visible).toBe(false);
    expect(second.visible).toBe(true);
  });
});
