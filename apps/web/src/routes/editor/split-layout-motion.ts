import type { SplitLayoutMode } from './split-layout-controller';

export type SplitLayoutMotion = {
  leftPaneCollapsed: boolean;
  rightPaneCollapsed: boolean;
  collapsedControlFlyX: number;
};

export function resolveSplitLayoutMotion(layoutMode: SplitLayoutMode): SplitLayoutMotion {
  if (layoutMode === 'left-only') {
    return {
      leftPaneCollapsed: false,
      rightPaneCollapsed: true,
      collapsedControlFlyX: 8,
    };
  }

  if (layoutMode === 'right-only') {
    return {
      leftPaneCollapsed: true,
      rightPaneCollapsed: false,
      collapsedControlFlyX: -8,
    };
  }

  return {
    leftPaneCollapsed: false,
    rightPaneCollapsed: false,
    collapsedControlFlyX: 0,
  };
}
