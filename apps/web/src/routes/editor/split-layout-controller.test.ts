// Responsibility: unit tests for split-layout-controller.
import { describe, expect, it } from 'vitest';
import {
  collapseEditor,
  collapseViewer,
  computePaneWidths,
  createSplitLayoutState,
  expandSplit,
  getClampedPaneSize,
  getClampedSplitRatio,
  resizeSplit,
  syncSplitRatio,
} from './split-layout-controller';

const config = {
  defaultSplitRatio: 0.28,
  minPaneWidthPx: 200,
  dividerWidthPx: 10,
  collapsedControlInsetPx: 16,
  collapsedPaneWidthPx: 44,
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
    expect(split.rightPaneWidthPx).toBe(750);

    const rightOnly = computePaneWidths(collapseEditor(createSplitLayoutState(0.3)), 1000, config);
    expect(rightOnly.leftPaneWidthPx).toBe(0);
    expect(rightOnly.rightPaneWidthPx).toBe(1000);

    const leftOnly = computePaneWidths(collapseViewer(createSplitLayoutState(0.3)), 1000, config);
    expect(leftOnly.leftPaneWidthPx).toBe(1000);
    expect(leftOnly.rightPaneWidthPx).toBe(0);
    expect(leftOnly.splitterLeftPx).toBe(990);
  });

  it('syncs split ratio and last ratio only in split mode', () => {
    const state = syncSplitRatio({ layoutMode: 'right-only', splitRatio: 0.1, lastSplitRatio: 0.4 }, 1000, config);
    expect(state.splitRatio).toBe(0.2);
    expect(state.lastSplitRatio).toBe(0.4);
  });

  it('keeps a pane at its minimum width before collapsing and restores split mode when dragged inward', () => {
    const initial = { layoutMode: 'split' as const, splitRatio: 0.4, lastSplitRatio: 0.4 };

    expect(resizeSplit(initial, 0.15, 1_000, config)).toMatchObject({
      layoutMode: 'split',
      splitRatio: 0.2,
    });

    const editorCollapsed = resizeSplit(initial, 0.09, 1_000, config);
    expect(editorCollapsed).toMatchObject({ layoutMode: 'right-only', lastSplitRatio: 0.4 });

    expect(resizeSplit(initial, 0.85, 1_000, config)).toMatchObject({
      layoutMode: 'split',
      splitRatio: 0.8,
    });

    const viewerCollapsed = resizeSplit(initial, 0.91, 1_000, config);
    expect(viewerCollapsed).toMatchObject({ layoutMode: 'left-only', lastSplitRatio: 0.4 });

    expect(resizeSplit(editorCollapsed, 0.35, 1_000, config)).toMatchObject({
      layoutMode: 'split',
      splitRatio: 0.35,
      lastSplitRatio: 0.35,
    });
  });

  it('still allows dragging to collapse when the container is narrower than two minimum panes', () => {
    expect(resizeSplit(createSplitLayoutState(0.3), 0.1, 300, config)).toMatchObject({
      layoutMode: 'right-only',
      lastSplitRatio: 0.3,
    });
    expect(resizeSplit(createSplitLayoutState(0.3), 0.9, 300, config)).toMatchObject({
      layoutMode: 'left-only',
      lastSplitRatio: 0.3,
    });
  });

  it('clamps vertical pane size between fixed minimum and container fraction cap', () => {
    expect(getClampedPaneSize(220, 700, 260, 0.5)).toBe(260);
    expect(getClampedPaneSize(420, 700, 260, 0.5)).toBe(350);
    expect(getClampedPaneSize(320, 480, 260, 0.5)).toBe(260);
  });
});
