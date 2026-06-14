// 职责：Split layout 控制器：layout mode 状态、split ratio clamping、pane 宽度计算
export type SplitLayoutMode = 'split' | 'left-only' | 'right-only';

export type SplitLayoutState = {
  layoutMode: SplitLayoutMode;
  splitRatio: number;
  lastSplitRatio: number;
};

export type SplitLayoutConfig = {
  defaultSplitRatio: number;
  minPaneWidthPx: number;
  dividerWidthPx: number;
  collapsedControlInsetPx: number;
};

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export function getClampedSplitRatio(nextRatio: number, containerWidth: number, minPaneWidthPx: number): number {
  if (!containerWidth) return clamp(nextRatio, 0.2, 0.8);
  const minLeftRatio = minPaneWidthPx / containerWidth;
  const maxLeftRatio = 1 - minPaneWidthPx / containerWidth;
  if (minLeftRatio >= maxLeftRatio) return 0.5;
  return clamp(nextRatio, minLeftRatio, maxLeftRatio);
}

export function createSplitLayoutState(defaultSplitRatio: number): SplitLayoutState {
  return {
    layoutMode: 'split',
    splitRatio: defaultSplitRatio,
    lastSplitRatio: defaultSplitRatio,
  };
}


export function collapseViewer(state: SplitLayoutState): SplitLayoutState {
  return {
    ...state,
    layoutMode: 'left-only',
    lastSplitRatio: state.layoutMode === 'split' ? state.splitRatio : state.lastSplitRatio,
  };
}

export function collapseEditor(state: SplitLayoutState): SplitLayoutState {
  return {
    ...state,
    layoutMode: 'right-only',
    lastSplitRatio: state.layoutMode === 'split' ? state.splitRatio : state.lastSplitRatio,
  };
}

export function expandSplit(state: SplitLayoutState, containerWidth: number, config: SplitLayoutConfig): SplitLayoutState {
  return {
    ...state,
    layoutMode: 'split',
    splitRatio: getClampedSplitRatio(state.lastSplitRatio || config.defaultSplitRatio, containerWidth, config.minPaneWidthPx),
  };
}

export function syncSplitRatio(state: SplitLayoutState, containerWidth: number, config: SplitLayoutConfig): SplitLayoutState {
  const splitRatio = getClampedSplitRatio(state.splitRatio, containerWidth, config.minPaneWidthPx);
  return {
    ...state,
    splitRatio,
    lastSplitRatio: state.layoutMode === 'split' ? splitRatio : state.lastSplitRatio,
  };
}

export function computePaneWidths(state: SplitLayoutState, containerWidth: number, config: SplitLayoutConfig) {
  const leftPaneWidthPx =
    state.layoutMode === 'left-only'
      ? containerWidth
      : state.layoutMode === 'right-only'
        ? 0
        : Math.round(containerWidth * state.splitRatio);
  const rightPaneWidthPx =
    state.layoutMode === 'right-only'
      ? containerWidth
      : state.layoutMode === 'left-only'
        ? 0
        : Math.max(containerWidth - leftPaneWidthPx - config.dividerWidthPx, 0);
  const splitterLeftPx =
    state.layoutMode === 'split'
      ? leftPaneWidthPx
      : state.layoutMode === 'left-only'
        ? Math.max(containerWidth - config.dividerWidthPx, 0)
        : 0;
  const splitterControlLeftPx =
    state.layoutMode === 'left-only' ? Math.max(containerWidth - config.collapsedControlInsetPx, 0) : config.collapsedControlInsetPx;
  return { leftPaneWidthPx, rightPaneWidthPx, splitterLeftPx, splitterControlLeftPx };
}
