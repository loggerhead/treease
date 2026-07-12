type SemanticTypeKey = 'map' | 'key' | 'seq' | 'str' | 'int' | 'float' | 'boolean' | 'nil';
export type AutoSaveMode = 'off' | 'afterDelay' | 'onFocusChange' | 'onWindowChange';

export const semanticTypeColors = {
  // 对象类型的语义色
  map: '#a31515',
  key: '#a31515',
  // 数组类型的语义色
  seq: '#a31515',
  // 字符串类型的语义色
  str: '#0451a5',
  // 整数类型的语义色
  int: '#098658',
  // 浮点类型的语义色
  float: '#098658',
  // 布尔类型的语义色
  boolean: '#0451a5',
  // 空值类型的语义色
  nil: '#0451a5',
} as Record<SemanticTypeKey, string>;

export const editorUiColors = {
  // 编辑器背景色
  'editor.background': '#ffffff',
  // 编辑器前景色
  'editor.foreground': '#0f172a',
  // 行号默认颜色
  'editorLineNumber.foreground': '#94a3b8',
  // 行号高亮颜色
  'editorLineNumber.activeForeground': '#0f172a',
  // 光标颜色
  'editorCursor.foreground': '#0f172a',
  // 选区背景色
  'editor.selectionBackground': '#dbeafe',
  // 选区高亮背景色
  'editor.selectionHighlightBackground': '#dbeafe',
  // 侧边概览尺背景（不透明）
  'editorOverviewRuler.background': '#ffffff',
  'editorOverviewRuler.border': '#e2e8f0',
};

export const formattingOptions = {
  // 默认缩进空格数
  indent: 2,
  // 是否启用智能格式化策略
  smart: true,
  // 单行最大长度
  maxLineLength: 100,
  // 内联结构最大复杂度
  maxInlineComplexity: 1,
  // 数组内联最大元素数
  maxArrayInlineItems: 6,
  // 对齐对象数组的字段
  alignObjectArrays: true,
};

export const graphViewerConfig = {
  // 图视图字体栈
  fontFamily:
    '"SF Mono", Monaco, Menlo, Consolas, "Ubuntu Mono", "Liberation Mono", "DejaVu Sans Mono", "Courier New", monospace',
  colors: {
    // 语义类型配色
    semanticType: semanticTypeColors,
    textMuted: '#6b7280',
    // 连线颜色
    edge: '#cbd5e1',
    node: {
      // 节点背景色
      background: '#ffffff',
      // 节点边框色
      border: '#00000040',
    },
    table: {
      // 表格背景色
      background: '#ffffff',
      // 表格边框色
      border: '#00000040',
      // 表头背景色
      headerBackground: '#f1f5f9',
      // 表头边框色
      headerBorder: '#00000040',
      // 行背景色
      rowBackground: '#ffffff',
      // 行边框色
      rowBorder: '#00000040',
      hoverRowBackground: '#e6f0ff',
      hoverCellBackground: '#ffe27a',
      // 滚动轨道背景色
      trackBackground: '#f8fafc',
      // 滚动轨道边框色
      trackBorder: '#e2e8f0',
      // 滚动条滑块背景色
      thumbBackground: '#cbd5e1',
    },
  },
  layout: {
    // 字体大小
    baseFontSize: 12,
    // 在字体大小下的字符平均宽度
    averageCharWidth: 7.2,
    // 行高
    rowHeight: 18,
    nodeBorderWidth: 1,
    // 行内容左右内边距
    rowPaddingInline: 20,
    // 行内容上下内边距
    rowPaddingBlock: 1,
    headerFontWeight: 600,
    // 同层节点水平间距
    layerGapX: 60,
    // 同层节点垂直间距
    layerGapY: 60,
    // 画布内边距
    canvasPadding: 50,
    // 表格最大可视高度
    tableMaxHeight: 1000,
    // 表头高度
    tableHeaderHeight: 26,
    // 表格行高
    tableRowHeight: 28,
  },
  // key/value 列宽配置
  columns: {
    // key 列最大宽度
    keyColumnMaxWidth: 300,
    // value 列最大宽度
    valueColumnMaxWidth: 500,
  },
  truncation: {
    // 标量值显示截断长度
    scalarValueMaxLength: 28,
    // 对象值显示截断长度
    objectValueMaxLength: 28,
    // 表格单元显示截断长度
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
