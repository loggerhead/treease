// 职责：Graph hover panel 布局与 shell：viewport 尺寸计算、skeleton markup、Leafer app config
export const tooltipPanelContainerSelector = '[data-tooltip-panel]';
export const tooltipPanelMinWidth = 120;
export const tooltipPanelMinHeight = 80;
export const tooltipPanelViewportMargin = 48;
export const tooltipPanelMaxViewportWidth = 960;
export const tooltipPanelMaxViewportHeight = 720;

export function resolveTooltipPanelViewportSize(
  contentWidth: number,
  contentHeight: number,
  viewportWidth: number,
  viewportHeight: number,
): { width: number; height: number } {
  const availableWidth = Math.max(
    tooltipPanelMinWidth,
    Math.min(tooltipPanelMaxViewportWidth, Math.max(tooltipPanelMinWidth, viewportWidth - tooltipPanelViewportMargin)),
  );
  const availableHeight = Math.max(
    tooltipPanelMinHeight,
    Math.min(tooltipPanelMaxViewportHeight, Math.max(tooltipPanelMinHeight, viewportHeight - tooltipPanelViewportMargin)),
  );
  return {
    width: Math.max(tooltipPanelMinWidth, Math.min(contentWidth, availableWidth)),
    height: Math.max(tooltipPanelMinHeight, Math.min(contentHeight, availableHeight)),
  };
}

export function buildGraphTooltipPanelShellMarkup(): string {
  return `
    <div class="graph-tooltip-panel-shell">
      <div data-tooltip-panel class="graph-tooltip-panel graph-tooltip-panel--loading">
        <div class="graph-tooltip-panel-skeleton" aria-hidden="true">
          <div class="graph-tooltip-panel-skeleton__bar graph-tooltip-panel-skeleton__bar--wide"></div>
          <div class="graph-tooltip-panel-skeleton__bar"></div>
          <div class="graph-tooltip-panel-skeleton__bar graph-tooltip-panel-skeleton__bar--short"></div>
        </div>
      </div>
    </div>
  `;
}

export function buildTooltipPanelAppConfig(view: HTMLDivElement) {
  return {
    view,
    type: 'viewport',
    editor: {
      visible: true,
      hittable: true,
      hover: false,
      moveable: false,
      resizeable: false,
      rotateable: false,
      skewable: false,
      flipable: false,
    },
    move: { drag: false, holdSpaceKey: true, holdRightKey: true, scroll: true },
    zoom: { disabled: true },
    wheel: { zoomMode: false },
    multiTouch: { disabled: true },
  };
}
