type SemanticTypeKey = 'map' | 'key' | 'seq' | 'str' | 'int' | 'float' | 'boolean' | 'nil';
export type AutoSaveMode = 'off' | 'afterDelay' | 'onFocusChange' | 'onWindowChange';

export const semanticTypeColors = {
  // Semantic color for object types
  map: '#a31515',
  key: '#a31515',
  // Semantic color for array types
  seq: '#a31515',
  // Semantic color for string types
  str: '#0451a5',
  // Semantic color for integer types
  int: '#098658',
  // Semantic color for floating-point types
  float: '#098658',
  // Semantic color for boolean types
  boolean: '#0451a5',
  // Semantic color for null types
  nil: '#0451a5',
} as Record<SemanticTypeKey, string>;

export const editorUiColors = {
  // Editor background color
  'editor.background': '#ffffff',
  // Editor foreground color
  'editor.foreground': '#294c66',
  // Default line-number color
  'editorLineNumber.foreground': '#9caebb',
  // Active line-number color
  'editorLineNumber.activeForeground': '#6d8292',
  // Cursor color
  'editorCursor.foreground': '#286b90',
  // Selection background color
  'editor.selectionBackground': '#eaf4fb',
  // Selection-highlight background color
  'editor.selectionHighlightBackground': '#eaf4fb',
  // Opaque overview-ruler background
  'editorOverviewRuler.background': '#ffffff',
  'editorOverviewRuler.border': '#cbd9e3',
};

export const formattingOptions = {
  // Default indentation width in spaces
  indent: 2,
  // Whether to enable smart formatting
  smart: true,
  // Maximum line length
  maxLineLength: 100,
  // Maximum inline-structure complexity
  maxInlineComplexity: 1,
  // Maximum number of inline array items
  maxArrayInlineItems: 6,
  // Whether to align fields in object arrays
  alignObjectArrays: true,
};

export const graphViewerConfig = {
  // Graph-view font stack
  fontFamily:
    '"SF Mono", Monaco, Menlo, Consolas, "Ubuntu Mono", "Liberation Mono", "DejaVu Sans Mono", "Courier New", monospace',
  colors: {
    // Semantic-type colors
    semanticType: semanticTypeColors,
    textMuted: '#74899a',
    // Edge color
    edge: '#aec9dc',
    node: {
      // Node background color
      background: '#ffffff',
      // Node border color
      border: '#afc4d4',
    },
    table: {
      // Table background color
      background: '#ffffff',
      // Table border color
      border: '#afc4d4',
      // Table-header background color
      headerBackground: '#f6f9fb',
      // Table-header border color
      headerBorder: '#d5e1e9',
      // Row background color
      rowBackground: '#ffffff',
      // Row border color
      rowBorder: '#e0e8ee',
      hoverRowBackground: '#eaf4fb',
      hoverCellBackground: '#fff2c8',
      // Scroll-track background color
      trackBackground: '#f2f6f9',
      // Scroll-track border color
      trackBorder: '#cbd9e3',
      // Scroll-thumb background color
      thumbBackground: '#9bb0c0',
    },
  },
  layout: {
    // Font size
    baseFontSize: 12,
    // Average character width at the configured font size
    averageCharWidth: 7.2,
    // Row height
    rowHeight: 18,
    nodeBorderWidth: 1,
    // Horizontal row-content padding
    rowPaddingInline: 20,
    // Vertical row-content padding
    rowPaddingBlock: 1,
    headerFontWeight: 600,
    // Horizontal gap between nodes at the same level
    layerGapX: 60,
    // Vertical gap between nodes at the same level
    layerGapY: 60,
    // Canvas padding
    canvasPadding: 50,
    // Maximum visible table height
    tableMaxHeight: 1000,
    // Table-header height
    tableHeaderHeight: 26,
    // Table-row height
    tableRowHeight: 28,
  },
  // Key/value column-width configuration
  columns: {
    // Maximum key-column width
    keyColumnMaxWidth: 300,
    // Maximum value-column width
    valueColumnMaxWidth: 500,
  },
  truncation: {
    // Scalar-value display truncation length
    scalarValueMaxLength: 28,
    // Object-value display truncation length
    objectValueMaxLength: 28,
    // Table-cell display truncation length
    tableValueMaxLength: 18,
    metaPathMinSegments: 4,
    metaPathMinChars: 28,
    metaPathKeepTailSegments: 1,
  },
};

export const neutralSyntaxColors = {
  punctuation: '#4b5563',
  comment: '#6b7280',
  operator: '#4b5563',
  function: '#4b5563',
};

export const defaultSettings = {
  editor: {
    semanticTypeColors,
    uiColors: editorUiColors,
  },
  formatting: {
    indent: formattingOptions.indent,
    smart: formattingOptions.smart,
    maxLineLength: formattingOptions.maxLineLength,
    maxInlineComplexity: formattingOptions.maxInlineComplexity,
    maxArrayInlineItems: formattingOptions.maxArrayInlineItems,
    alignObjectArrays: formattingOptions.alignObjectArrays,
  },
  viewer: {
    graphViewer: graphViewerConfig,
  },
  interaction: {
    enableSyncScroll: true,
    autoSave: 'off' as AutoSaveMode,
  },
  parser: {
    enableNest: true,
  },
};
