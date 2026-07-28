export type GraphViewerRenderConfig = {
  fontFamily: string;
  colors: {
    semanticType: Record<'map' | 'key' | 'seq' | 'str' | 'int' | 'float' | 'boolean' | 'nil', string>;
    textMuted: string;
    edge: string;
    node: { background: string; border: string };
    table: Record<string, string>;
  };
  layout: Record<string, number>;
  columns: Record<string, number>;
  truncation: Record<string, number>;
};

export const defaultGraphViewerRenderConfig: GraphViewerRenderConfig = {
  fontFamily: '"SF Mono", Monaco, Menlo, Consolas, "Ubuntu Mono", "Liberation Mono", "DejaVu Sans Mono", "Courier New", monospace',
  colors: { semanticType: { map: '#a31515', key: '#a31515', seq: '#a31515', str: '#0451a5', int: '#098658', float: '#098658', boolean: '#0451a5', nil: '#0451a5' }, textMuted: '#6b7280', edge: '#cbd5e1', node: { background: '#ffffff', border: '#00000040' }, table: { background: '#ffffff', border: '#00000040', headerBackground: '#f1f5f9', headerBorder: '#00000040', rowBackground: '#ffffff', rowBorder: '#00000040', hoverRowBackground: '#e6f0ff', hoverCellBackground: '#ffe27a', trackBackground: '#f8fafc', trackBorder: '#e2e8f0', thumbBackground: '#cbd5e1' } },
  layout: { baseFontSize: 12, averageCharWidth: 7.2, rowHeight: 18, nodeBorderWidth: 1, rowPaddingInline: 20, rowPaddingBlock: 1, headerFontWeight: 600, layerGapX: 60, layerGapY: 60, canvasPadding: 50, tableMaxHeight: 1000, tableHeaderHeight: 26, tableRowHeight: 28 },
  columns: { keyColumnMaxWidth: 300, valueColumnMaxWidth: 500 },
  truncation: { scalarValueMaxLength: 28, objectValueMaxLength: 28, tableValueMaxLength: 18, metaPathMinSegments: 4, metaPathMinChars: 28, metaPathKeepTailSegments: 1 },
};
