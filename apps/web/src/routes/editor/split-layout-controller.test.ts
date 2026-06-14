// 职责：split-layout-controller 的单元测试
import { describe, expect, it } from 'vitest';
import {
  collapseEditor,
  collapseViewer,
  computePaneWidths,
  createSplitLayoutState,
  expandSplit,
  getClampedSplitRatio,
  syncSplitRatio,
} from './split-layout-controller';

const config = {
  defaultSplitRatio: 0.28,
  minPaneWidthPx: 200,
  dividerWidthPx: 10,
  collapsedControlInsetPx: 16,
};

describe('split-layout-controller', () => {
  it('clamps ratio when container width is unavailable', () => {
    expect(getClampedSplitRatio(0.1, 0, config.minPaneWidthPx)).toBe(0.2);
    expect(getClampedSplitRatio(0.9, 0, config.minPaneWidthPx)).toBe(0.8);
  });

  it('uses balanced ratio when min panes exceed container', () => {
    expect(getClampedSplitRatio(0.2, 300, config.minPaneWidthPx)).toBe(0.5);
  });

  it('collapses and expands while preserving last split ratio', () => {
    let state = createSplitLayoutState(config.defaultSplitRatio);
    state = { ...state, splitRatio: 0.4 };
    state = collapseViewer(state);
    expect(state.layoutMode).toBe('left-only');
    expect(state.lastSplitRatio).toBe(0.4);
    state = expandSplit(state, 1000, config);
    expect(state.layoutMode).toBe('split');
    expect(state.splitRatio).toBe(0.4);
  });

  it('computes widths for split and collapsed modes', () => {
    const split = computePaneWidths({ layoutMode: 'split', splitRatio: 0.25, lastSplitRatio: 0.25 }, 1000, config);
    expect(split.leftPaneWidthPx).toBe(250);
    expect(split.rightPaneWidthPx).toBe(740);

    const rightOnly = computePaneWidths(collapseEditor(createSplitLayoutState(0.3)), 1000, config);
    expect(rightOnly.leftPaneWidthPx).toBe(0);
    expect(rightOnly.rightPaneWidthPx).toBe(1000);
  });

  it('syncs split ratio and last ratio only in split mode', () => {
    const state = syncSplitRatio({ layoutMode: 'right-only', splitRatio: 0.1, lastSplitRatio: 0.4 }, 1000, config);
    expect(state.splitRatio).toBe(0.2);
    expect(state.lastSplitRatio).toBe(0.4);
  });
});
