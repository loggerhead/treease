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
  collapsedPaneWidthPx: number;
};

type SplitLayoutDragRect = Pick<DOMRect, 'left' | 'width'>;

export type SplitLayoutDragUpdate = {
  state: SplitLayoutState;
  offsetX: number;
  containerWidth: number;
  collapseSide: SplitLayoutCollapseSide | null;
};

export type SplitLayoutCollapseSide = 'left' | 'right';

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

export function resizeSplit(
  state: SplitLayoutState,
  nextRatio: number,
  containerWidth: number,
  config: SplitLayoutConfig,
): SplitLayoutState {
  const nextLeftWidthPx = nextRatio * containerWidth;
  const collapseThresholdPx = config.minPaneWidthPx / 2;
  if (nextLeftWidthPx < collapseThresholdPx) return collapseEditor(state);
  if (containerWidth - nextLeftWidthPx < collapseThresholdPx) return collapseViewer(state);

  return syncSplitRatio(
    { ...state, layoutMode: 'split', splitRatio: nextRatio },
    containerWidth,
    config,
  );
}

export function createSplitLayoutDragController(config: SplitLayoutConfig) {
  let dragRect: SplitLayoutDragRect | null = null;

  function update(state: SplitLayoutState, clientX: number): SplitLayoutDragUpdate | null {
    if (!dragRect?.width) return null;
    const offsetX = clamp(clientX - dragRect.left, 0, dragRect.width);
    const collapseThresholdPx = config.minPaneWidthPx / 2;
    const collapseHintLimitPx = config.minPaneWidthPx * 1.5;
    const rightWidthPx = dragRect.width - offsetX;
    const collapseSide = dragRect.width > config.minPaneWidthPx * 2
      && offsetX > collapseThresholdPx
      && offsetX < collapseHintLimitPx
      ? 'left'
      : rightWidthPx > collapseThresholdPx && rightWidthPx < collapseHintLimitPx
        ? 'right'
        : null;
    return {
      state: resizeSplit(state, offsetX / dragRect.width, dragRect.width, config),
      offsetX,
      containerWidth: dragRect.width,
      collapseSide,
    };
  }

  return {
    start(state: SplitLayoutState, clientX: number, rect: SplitLayoutDragRect): SplitLayoutDragUpdate | null {
      dragRect = rect;
      return update(state, clientX);
    },
    move: update,
    end(): void {
      dragRect = null;
    },
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
  // A collapsed pane has no reserved visual rail; the configured width only positions its expand control.
  const collapsedPaneWidthPx = Math.min(config.collapsedPaneWidthPx, Math.max(Math.floor(containerWidth / 2), 0));

  if (state.layoutMode === 'left-only') {
    const leftPaneWidthPx = Math.max(containerWidth, 0);
    return {
      leftPaneWidthPx,
      rightPaneWidthPx: 0,
      splitterLeftPx: Math.max(leftPaneWidthPx - config.dividerWidthPx, 0),
      splitterControlLeftPx: Math.max(leftPaneWidthPx - collapsedPaneWidthPx / 2, 0),
    };
  }

  if (state.layoutMode === 'right-only') {
    return {
      leftPaneWidthPx: 0,
      rightPaneWidthPx: Math.max(containerWidth, 0),
      splitterLeftPx: 0,
      splitterControlLeftPx: collapsedPaneWidthPx / 2,
    };
  }

  const leftPaneWidthPx = Math.round(containerWidth * state.splitRatio);
  const rightPaneWidthPx = Math.max(containerWidth - leftPaneWidthPx, 0);
  const splitterLeftPx = leftPaneWidthPx;
  const splitterControlLeftPx = config.collapsedControlInsetPx;
  return { leftPaneWidthPx, rightPaneWidthPx, splitterLeftPx, splitterControlLeftPx };
}
