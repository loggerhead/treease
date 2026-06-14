import type { BuilderConfig } from '@core-wasm/index';
import type { GraphViewerConfig } from '../settings/ui-settings';
import { toWasmBuilderConfig } from '../../shared/brand-bridge';

export function buildGraphStreamBuilderConfig(renderConfig: GraphViewerConfig): BuilderConfig {
  return toWasmBuilderConfig({
    keyWidth: renderConfig.columns.keyColumnMaxWidth,
    valueWidth: renderConfig.columns.valueColumnMaxWidth,
    rowHeight: renderConfig.layout.rowHeight,
    rowPaddingX: renderConfig.layout.rowPaddingInline,
    rowPaddingY: renderConfig.layout.rowPaddingBlock,
    nodeBorderWidth: renderConfig.layout.nodeBorderWidth ?? 1,
    vGap: renderConfig.layout.layerGapY,
    hGap: renderConfig.layout.layerGapX,
    tableMaxHeight: renderConfig.layout.tableMaxHeight,
    tableRowHeight: renderConfig.layout.tableRowHeight,
    tableHeaderHeight: renderConfig.layout.tableHeaderHeight,
    tableColumnWidth: renderConfig.columns.valueColumnMaxWidth,
    avgCharWidthX10: Math.round(renderConfig.layout.averageCharWidth * 10),
    fontSize: renderConfig.layout.baseFontSize,
    metaPathMinSegments: renderConfig.truncation.metaPathMinSegments,
    metaPathMinChars: renderConfig.truncation.metaPathMinChars,
    metaPathKeepTailSegments: renderConfig.truncation.metaPathKeepTailSegments,
  });
}
