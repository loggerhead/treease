/**
 * @vitest-environment jsdom
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TooltipPlugin } from './TooltipPlugin';

describe('TooltipPlugin', () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = '';
    document.head.innerHTML = '';
  });

  it('honors custom close delay before hiding tooltip', () => {
    vi.useFakeTimers();
    const app = {
      view: document.createElement('div'),
      on_: vi.fn(() => 1),
      off_: vi.fn(),
    };
    const plugin = new TooltipPlugin(app, {
      className: 'leafer-x-tooltip',
      closeDelay: 320,
      getContent: () => 'content',
      events: {
        LeaferEvent: { VIEW_READY: 'view:ready' },
        PointerEvent: { MOVE: 'move', CLICK: 'click' },
      },
    });

    const hideTooltipSpy = vi.spyOn(plugin as any, 'hideTooltip');
    (plugin as any).activeNode = { tag: 'Text' };
    (plugin as any).isHoveringTooltip = false;
    (plugin as any).isHoveringNode = false;

    (plugin as any).scheduleClose();
    vi.advanceTimersByTime(319);
    expect(hideTooltipSpy).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(hideTooltipSpy).toHaveBeenCalledTimes(1);

    plugin.destroy();
  });

  it('clears frozen state after leaving tooltip container', () => {
    const app = {
      view: document.createElement('div'),
      on_: vi.fn(() => 1),
      off_: vi.fn(),
    };
    const plugin = new TooltipPlugin(app, {
      className: 'leafer-x-tooltip',
      getContent: () => 'content',
      events: {
        LeaferEvent: { VIEW_READY: 'view:ready' },
        PointerEvent: { MOVE: 'move', CLICK: 'click' },
      },
    });

    const container = document.querySelector('.leafer-x-tooltip') as HTMLDivElement | null;
    expect(container).not.toBeNull();
    (plugin as any).isFrozen = true;
    (plugin as any).isHoveringNode = false;

    container?.dispatchEvent(new MouseEvent('mouseleave', { bubbles: true, relatedTarget: document.body }));
    expect((plugin as any).isFrozen).toBe(false);

    plugin.destroy();
  });

  it('clamps tooltip position inside the viewport', () => {
    const app = {
      view: document.createElement('div'),
      on_: vi.fn(() => 1),
      off_: vi.fn(),
    };
    const plugin = new TooltipPlugin(app, {
      className: 'leafer-x-tooltip',
      getContent: () => 'content',
      events: {
        LeaferEvent: { VIEW_READY: 'view:ready' },
        PointerEvent: { MOVE: 'move', CLICK: 'click' },
      },
    });

    const tooltip = document.createElement('div');
    Object.defineProperty(tooltip, 'offsetWidth', { configurable: true, value: 200 });
    Object.defineProperty(tooltip, 'offsetHeight', { configurable: true, value: 160 });

    const position = (plugin as any).calculateTooltipPosition(
      { clientX: 1000, clientY: 740 },
      tooltip,
    );

    expect(position.x).toBeGreaterThanOrEqual(0);
    expect(position.y).toBeGreaterThanOrEqual(0);
    expect(position.x + tooltip.offsetWidth).toBeLessThanOrEqual(window.innerWidth);
    expect(position.y + tooltip.offsetHeight).toBeLessThanOrEqual(window.innerHeight);

    plugin.destroy();
  });
});
