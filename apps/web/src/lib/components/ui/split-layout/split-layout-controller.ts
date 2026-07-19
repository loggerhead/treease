// Responsibility: shared split-layout constraints and size calculations.
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

function getRememberedSplitRatio(state: SplitLayoutState): number {
  return state.layoutMode === 'split' ? state.splitRatio : state.lastSplitRatio;
}

export function getClampedSplitRatio(nextRatio: number, containerWidth: number, minPaneWidthPx: number): number {
  if (!containerWidth) return clamp(nextRatio, 0.2, 0.8);
  const minLeftRatio = minPaneWidthPx / containerWidth;
  const maxLeftRatio = 1 - minPaneWidthPx / containerWidth;
  if (minLeftRatio >= maxLeftRatio) return 0.5;
  return clamp(nextRatio, minLeftRatio, maxLeftRatio);
}

export function getClampedPaneSize(
  nextSizePx: number,
  containerSizePx: number,
  minSizePx: number,
  maxFraction: number,
): number {
  const maxSizePx = Math.max(minSizePx, Math.round(containerSizePx * maxFraction));
  return clamp(nextSizePx, minSizePx, maxSizePx);
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
    lastSplitRatio: getRememberedSplitRatio(state),
  };
}

export function collapseEditor(state: SplitLayoutState): SplitLayoutState {
  return {
    ...state,
    layoutMode: 'right-only',
    lastSplitRatio: getRememberedSplitRatio(state),
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
  if (state.layoutMode === 'left-only') {
    return {
      leftPaneWidthPx: containerWidth,
      rightPaneWidthPx: 0,
      splitterLeftPx: Math.max(containerWidth - config.dividerWidthPx, 0),
      splitterControlLeftPx: Math.max(containerWidth - config.collapsedControlInsetPx, 0),
    };
  }

  if (state.layoutMode === 'right-only') {
    return {
      leftPaneWidthPx: 0,
      rightPaneWidthPx: containerWidth,
      splitterLeftPx: 0,
      splitterControlLeftPx: config.collapsedControlInsetPx,
    };
  }

  const leftPaneWidthPx = Math.round(containerWidth * state.splitRatio);
  const rightPaneWidthPx = Math.max(containerWidth - leftPaneWidthPx, 0);
  const splitterLeftPx = leftPaneWidthPx;
  const splitterControlLeftPx = config.collapsedControlInsetPx;
  return { leftPaneWidthPx, rightPaneWidthPx, splitterLeftPx, splitterControlLeftPx };
}
