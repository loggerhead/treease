import { describe, expect, it } from 'vitest';
import {
  createSplitLayoutDragController,
  createSplitLayoutState,
  type SplitLayoutConfig,
} from './split-layout-controller';

const config: SplitLayoutConfig = {
  defaultSplitRatio: 0.4,
  minPaneWidthPx: 200,
  dividerWidthPx: 10,
  collapsedControlInsetPx: 16,
  collapsedPaneWidthPx: 44,
};

describe('createSplitLayoutDragController', () => {
  it('converts pointer positions into bounded split and collapsed states for one drag session', () => {
    const controller = createSplitLayoutDragController(config);
    const initial = createSplitLayoutState(config.defaultSplitRatio);
    const rect = { left: 100, width: 1_000 };

    expect(controller.start(initial, 500, rect)).toMatchObject({
      offsetX: 400,
      containerWidth: 1_000,
      state: { layoutMode: 'split', splitRatio: 0.4 },
    });
    expect(controller.move(initial, 1_200)?.state.layoutMode).toBe('left-only');
    expect(controller.move(initial, 0)?.state.layoutMode).toBe('right-only');
    expect(controller.move(initial, 250)?.collapseSide).toBe('left');
    expect(controller.move(initial, 950)?.collapseSide).toBe('right');

    controller.end();
    expect(controller.move(initial, 500)).toBeNull();
  });
});
