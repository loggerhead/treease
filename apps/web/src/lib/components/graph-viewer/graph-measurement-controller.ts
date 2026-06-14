// 职责：GraphViewer 文本测量控制器：hidden DOM 测量、scene config 同步、pending frame 管理
import type { GraphViewerConfig } from '../../settings/ui-settings';

type MeasurementElements = {
  measureRoot: HTMLDivElement | null;
  measureRow: HTMLDivElement | null;
  measureRowText: HTMLSpanElement | null;
  measureHeader: HTMLDivElement | null;
  measureHeaderText: HTMLSpanElement | null;
};

type MeasurementDeps = {
  getElements: () => MeasurementElements;
  getRenderConfig: () => GraphViewerConfig;
  getMeasureTextSample: () => string;
  tick: () => Promise<void>;
  saveGraphViewerConfig: (config: GraphViewerConfig) => Promise<void>;
};

function toNumber(value: string | null | undefined): number {
  const numeric = Number.parseFloat(value ?? '');
  return Number.isFinite(numeric) ? numeric : 0;
}

export function createGraphMeasurementController(deps: MeasurementDeps) {
  let pendingMeasureFrame: number | null = null;

  function scheduleMeasure(): void {
    if (!deps.getElements().measureRoot) return;
    if (pendingMeasureFrame) cancelAnimationFrame(pendingMeasureFrame);
    pendingMeasureFrame = requestAnimationFrame(() => {
      pendingMeasureFrame = null;
      void measureAndSync();
    });
  }

  async function measureAndSync(): Promise<void> {
    const { measureRoot, measureRow, measureRowText, measureHeader, measureHeaderText } = deps.getElements();
    if (!measureRoot || !measureRow || !measureRowText || !measureHeader || !measureHeaderText) return;
    await deps.tick();
    const renderConfig = deps.getRenderConfig();
    const layout = renderConfig.layout;
    const sample = deps.getMeasureTextSample();
    measureRoot.style.fontFamily = renderConfig.fontFamily;
    measureRow.style.fontSize = `${layout.baseFontSize}px`;
    measureRow.style.padding = `${layout.rowPaddingBlock}px ${layout.rowPaddingInline}px`;
    measureRow.style.borderWidth = '1px';
    measureRow.style.borderStyle = 'solid';
    measureRow.style.borderColor = 'transparent';
    measureRowText.textContent = sample;
    measureHeader.style.fontSize = `${layout.baseFontSize}px`;
    measureHeader.style.padding = `${layout.rowPaddingBlock}px ${layout.rowPaddingInline}px`;
    measureHeader.style.borderWidth = '1px';
    measureHeader.style.borderStyle = 'solid';
    measureHeader.style.borderColor = 'transparent';
    measureHeaderText.textContent = sample;

    const rowTextRect = measureRowText.getBoundingClientRect();
    const rowRect = measureRow.getBoundingClientRect();
    const headerRect = measureHeader.getBoundingClientRect();
    const rowStyle = getComputedStyle(measureRow);
    const textStyle = getComputedStyle(measureRowText);
    const averageCharWidth = Math.ceil(rowTextRect.width / sample.length);
    const nextLayout = {
      ...layout,
      averageCharWidth,
      baseFontSize: Math.round(toNumber(textStyle.fontSize)) || layout.baseFontSize,
      rowHeight: Math.ceil(rowRect.height),
      rowPaddingInline: Math.round(toNumber(rowStyle.paddingLeft)) || layout.rowPaddingInline,
      rowPaddingBlock: Math.round(toNumber(rowStyle.paddingTop)) || layout.rowPaddingBlock,
      tableRowHeight: Math.ceil(rowRect.height),
      tableHeaderHeight: Math.ceil(headerRect.height),
    };
    const changed =
      Math.abs(nextLayout.averageCharWidth - layout.averageCharWidth) > 0.5 ||
      Math.abs(nextLayout.baseFontSize - layout.baseFontSize) > 0.5 ||
      Math.abs(nextLayout.rowHeight - layout.rowHeight) > 0.5 ||
      Math.abs(nextLayout.rowPaddingInline - layout.rowPaddingInline) > 0.5 ||
      Math.abs(nextLayout.rowPaddingBlock - layout.rowPaddingBlock) > 0.5 ||
      Math.abs(nextLayout.tableRowHeight - layout.tableRowHeight) > 0.5 ||
      Math.abs(nextLayout.tableHeaderHeight - layout.tableHeaderHeight) > 0.5;
    if (changed) {
      await deps.saveGraphViewerConfig({ ...renderConfig, layout: nextLayout });
    }
  }

  function dispose(): void {
    if (pendingMeasureFrame) cancelAnimationFrame(pendingMeasureFrame);
    pendingMeasureFrame = null;
  }

  return { scheduleMeasure, measureAndSync, dispose };
}
