import { describe, expect, it } from 'vitest';
import { resolveSplitLayoutMotion } from './split-layout-motion';

describe('split-layout-motion', () => {
  it('keeps both panes visible in split mode', () => {
    expect(resolveSplitLayoutMotion('split')).toEqual({
      leftPaneCollapsed: false,
      rightPaneCollapsed: false,
      collapsedControlFlyX: 0,
    });
  });

  it('collapses the viewer pane and slides the expand control in from the right', () => {
    expect(resolveSplitLayoutMotion('left-only')).toEqual({
      leftPaneCollapsed: false,
      rightPaneCollapsed: true,
      collapsedControlFlyX: 8,
    });
  });

  it('collapses the editor pane and slides the expand control in from the left', () => {
    expect(resolveSplitLayoutMotion('right-only')).toEqual({
      leftPaneCollapsed: true,
      rightPaneCollapsed: false,
      collapsedControlFlyX: -8,
    });
  });
});
