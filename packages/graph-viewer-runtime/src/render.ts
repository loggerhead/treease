import { resolveGraphCellDisplayText } from './literal-display';
import { resolveSemanticTypeColor } from './semantic-type-color';
import {
  createTableRuntime,
  describeTableRuntime,
  destroyTableRuntime,
  patchTableContent,
  patchTableStructure,
} from './table/runtime';
import type {
  DrawContext,
  GraphBoxArgs,
  GraphCell,
  GraphCellKind,
  GraphNode,
  GraphRow,
  GraphTable,
  TableRowBinding,
  TableRowRenderEntry,
} from './types';

export type {
  DrawContext,
  GraphBoxArgs,
  GraphCell,
  GraphCellKind,
  GraphEdge,
  GraphNode,
  GraphNodeKey,
  GraphRow,
  GraphTable,
  GraphTextArgs,
  TableRowBinding,
  TableRowRenderEntry,
  TableRuntime,
  ValueType,
} from './types';

type NodeBoxOptions = {
  stroke?: boolean;
};

export function createNodeBox(
  ctx: DrawContext,
  node: GraphNode,
  colors: { background: string; border: string },
  options: NodeBoxOptions = {},
) {
  const shouldStroke = options.stroke !== false;
  const box = new ctx.BoxCtor({
    x: node.boxArgs.x,
    y: node.boxArgs.y,
    width: node.boxArgs.width,
    height: node.boxArgs.height,
    cornerRadius: node.boxArgs.cornerRadius,
    fill: colors.background,
    stroke: shouldStroke ? colors.border : undefined,
    strokeWidth: shouldStroke ? (ctx.styleConfig.layout.nodeBorderWidth ?? 1) : undefined,
    strokeAlign: shouldStroke ? 'inside' : undefined,
  });
  ctx.nodeLayer.add(box);
  return box;
}

export function createRowBox(ctx: DrawContext, parent: any, bounds: GraphBoxArgs) {
  const box = new ctx.BoxCtor({
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    cornerRadius: bounds.cornerRadius,
    fill: 'transparent',
  });
  parent.add(box);
  return box;
}

export function createCellBox(
  ctx: DrawContext,
  parent: any,
  bounds: GraphBoxArgs,
  style: { fill: string; stroke?: string; strokeWidth?: number; strokeAlign?: 'inside' | 'center' | 'outside' },
) {
  const box = new ctx.BoxCtor({
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    cornerRadius: bounds.cornerRadius,
    fill: style.fill,
    stroke: style.stroke,
    strokeWidth: style.strokeWidth,
    strokeAlign: style.stroke ? (style.strokeAlign ?? 'inside') : undefined,
  });
  parent.add(box);
  return box;
}

function isHeaderlessSequenceTable(node?: GraphNode): boolean {
  return !!node?.table && node.table.headerHeight === 0 && (node.table.columns?.length ?? 0) === 0;
}

function isHeaderlessTable(node?: GraphNode): boolean {
  return !!node?.table && node.table.headerHeight === 0;
}


function buildCellTextProps(
  ctx: DrawContext,
  cell: GraphCell,
  kind: GraphCellKind,
  nodeKind?: GraphNode['kind'],
  node?: GraphNode,
) {
  const textArgs = cell.textArgs;
  const cellBox = cell.boxArgs;
  const useCellBounds = kind !== 'meta' && cellBox.height > 0 && textArgs.width > 0 && textArgs.height > 0;
  const resolvedMaxWidth = textArgs.width > 0 ? textArgs.width : undefined;
  const resolvedHeight = textArgs.height > 0 ? textArgs.height : ctx.styleConfig.layout.rowHeight;
  const isHeader = kind === 'header';
  const editingEnabledByContext = ctx.editable ?? true;
  const isEditable = !isHeader && editingEnabledByContext && textArgs.editable;
  const textAlign =
    kind === 'value' && (nodeKind === 'object' || isHeaderlessSequenceTable(node)) ? 'right' : textArgs.textAlign;
  const semanticColors = ctx.styleConfig.colors.semanticType;
  const mutedText = ctx.styleConfig.colors.textMuted;
  const useMutedText =
    kind === 'value' &&
    // Counts and delimiters such as {6}, [3], {} and [] are graph structure summaries,
    // not source scalar values. Monaco renders their counterparts as punctuation.
    (cell.isMissing === true || cell.valueType === 'object' || cell.valueType === 'array' || Boolean(cell.isIndex));
  const baseColor =
    kind === 'header' || kind === 'key'
      ? semanticColors.key
      : kind === 'meta'
        ? mutedText
        : resolveSemanticTypeColor(semanticColors, cell.semType);
  const textColor = useMutedText ? mutedText : baseColor;
  const rowPaddingInline = ctx.styleConfig.layout.rowPaddingInline;
  const hitX = useCellBounds ? 0 : textArgs.x;
  const hitY = useCellBounds ? 0 : textArgs.y;
  const hitWidth = useCellBounds ? cellBox.width : resolvedMaxWidth;
  const hitHeight = useCellBounds ? cellBox.height : resolvedHeight;
  const paddingLeft = useCellBounds ? rowPaddingInline : Math.max(0, textArgs.x - hitX);
  const paddingRight = useCellBounds
    ? rowPaddingInline
    : Math.max(0, hitX + (hitWidth ?? 0) - (textArgs.x + textArgs.width));
  const resolvedPadBlock = Math.max(0, (hitHeight ?? 0) - textArgs.height) / 2;
  const paddingTop = useCellBounds ? resolvedPadBlock : Math.max(0, textArgs.y - hitY);
  const paddingBottom = useCellBounds
    ? resolvedPadBlock
    : Math.max(0, hitY + (hitHeight ?? 0) - (textArgs.x + textArgs.height));
  const cellText =
    kind === 'value'
      ? resolveGraphCellDisplayText(textArgs.text, cell.text || cell.value || '', cell.valueType, ctx.languageIdValue)
      : textArgs.text === '' || textArgs.text == null
        ? (cell.text || cell.value || '')
        : textArgs.text;
  let displayText = cellText;
  if (kind === 'value' && cell.valueType === 'string' && displayText.includes('\n')) {
    displayText = displayText.replace(/\n/g, ' ');
  }
  const shouldKeepFullNullLiteral =
    kind === 'value' && cell.valueType === 'null' && isHeaderlessSequenceTable(node);
  const shouldApplyTextOverflow =
    !!resolvedMaxWidth && !(kind === 'meta' && cell.text === cell.value) && !shouldKeepFullNullLiteral;
  return {
    x: useCellBounds ? hitX : textArgs.x,
    y: useCellBounds ? hitY : textArgs.y,
    text: displayText,
    width: useCellBounds ? hitWidth : resolvedMaxWidth,
    height: useCellBounds ? hitHeight : resolvedHeight,
    textOverflow: shouldApplyTextOverflow ? '...' : undefined,
    textWrap: resolvedMaxWidth ? 'none' : undefined,
    padding: useCellBounds ? [paddingTop, paddingRight, paddingBottom, paddingLeft] : undefined,
    fill: textColor,
    fontSize: ctx.fontSize,
    fontFamily: ctx.styleConfig.fontFamily,
    fontWeight: isHeader ? ctx.styleConfig.layout.headerFontWeight : undefined,
    textAlign,
    verticalAlign: textArgs.verticalAlign,
    editable: isEditable,
    editInner: isEditable ? (ctx.textEditInnerName ?? 'TextEditor') : undefined,
    editConfig: isEditable
      ? { moveable: false, resizeable: false, rotateable: false, skewable: false, flipable: false }
      : undefined,
    hittable: true,
    hitSelf: true,
    hitBox: true,
  };
}

function updateCellText(
  ctx: DrawContext,
  text: any,
  cell: GraphCell,
  kind: GraphCellKind,
  nodeKind?: GraphNode['kind'],
  node?: GraphNode,
) {
  Object.assign(text, buildCellTextProps(ctx, cell, kind, nodeKind, node));
  (text as any).__graphCell = cell;
  (text as any).__graphCellKind = kind;
  (text as any).__graphNodeKind = nodeKind;
}

export function createCellText(
  ctx: DrawContext,
  parent: any,
  cell: GraphCell,
  kind: GraphCellKind,
  nodeKind?: GraphNode['kind'],
  node?: GraphNode,
) {
  const text = new ctx.TextCtor(buildCellTextProps(ctx, cell, kind, nodeKind, node));
  parent.add(text);
  updateCellText(ctx, text, cell, kind, nodeKind, node);
  return text;
}

function getTableStrokeWidth(): number {
  return 1;
}

function createBorderStrokeBox(ctx: DrawContext, parent: any, bounds: GraphBoxArgs, stroke: string, strokeWidth = 1) {
  const box = createCellBox(ctx, parent, bounds, { fill: 'transparent', stroke, strokeWidth, strokeAlign: 'center' });
  box.hittable = false;
  box.hitChildren = false;
  box.hitSelf = false;
  return box;
}

function createTableBodyContent(
  ctx: DrawContext,
  nodeBox: any,
  options: {
    innerOffset: number;
    rowOffsetY: number;
    innerWidth: number;
    contentHeight: number;
    viewportHeight: number;
    enableVerticalScroll: boolean;
  },
) {
  const bodyViewport = new ctx.BoxCtor({
    x: options.innerOffset,
    y: options.innerOffset + options.rowOffsetY,
    width: options.innerWidth,
    height: options.viewportHeight,
    hittable: true,
    fill: 'transparent',
  });
  const bodyContent = new ctx.BoxCtor({
    x: 0,
    y: 0,
    width: options.innerWidth,
    height: options.contentHeight,
    fill: 'transparent',
  });
  const scrollTrack = new ctx.BoxCtor({
    x: Math.max(0, options.innerWidth - 6),
    y: 0,
    width: 6,
    height: options.viewportHeight,
    cornerRadius: 3,
    fill: 'rgba(148, 163, 184, 0.16)',
    visible: options.enableVerticalScroll,
    hittable: true,
  });
  const scrollThumb = new ctx.BoxCtor({
    x: Math.max(0, options.innerWidth - 6),
    y: 0,
    width: 6,
    height: 0,
    cornerRadius: 3,
    fill: 'rgba(100, 116, 139, 0.72)',
    visible: options.enableVerticalScroll,
    hittable: true,
  });
  bodyViewport.add(bodyContent);
  bodyViewport.add(scrollTrack);
  bodyViewport.add(scrollThumb);
  (bodyViewport as any).__graphViewportHeight = options.viewportHeight;
  nodeBox.add(bodyViewport);
  return { bodyViewport, bodyContent, scrollTrack, scrollThumb, headerNodes: [] };
}

function drawTableHeader(ctx: DrawContext, nodeBox: any, table: GraphTable, _innerOffset: number, node: GraphNode) {
  const headerNodes: any[] = [];
  const headerlessTable = isHeaderlessTable(node);
  table.columns.forEach((cell) => {
    cell.isTableCell = true;
    cell.isHeader = true;
    cell.isHeaderlessTable = headerlessTable;
    const strokeWidth = getTableStrokeWidth();
    const contentBounds = {
      ...cell.boxArgs,
      x: cell.boxArgs.x,
      y: cell.boxArgs.y,
    };
    const cellBox = createCellBox(ctx, nodeBox, contentBounds, {
      fill: ctx.styleConfig.colors.table.headerBackground,
    });
    const borderBox = createBorderStrokeBox(
      ctx,
      nodeBox,
      contentBounds,
      ctx.styleConfig.colors.table.headerBorder,
      strokeWidth,
    );
    createCellText(ctx, cellBox, cell, 'header', node.kind, node);
    headerNodes.push(cellBox, borderBox);
  });
  return headerNodes;
}

function createTableRowSlot(
  ctx: DrawContext,
  bodyContent: any,
  templateRow: GraphRow,
  options: {
    innerOffset: number;
    rowStartY: number;
    columns: GraphTable['columns'];
    node: GraphNode;
  },
): TableRowRenderEntry {
  const hideBorders = isHeaderlessSequenceTable(options.node);
  const headerlessTable = isHeaderlessTable(options.node);
  const rowBox = createRowBox(ctx, bodyContent, {
    ...templateRow.boxArgs,
    x: templateRow.boxArgs.x - options.innerOffset,
    y: templateRow.boxArgs.y - options.rowStartY,
  });
  rowBox.fill = ctx.styleConfig.colors.table.rowBackground;
  rowBox.hoverStyle = {
    fill: ctx.styleConfig.colors.table.hoverRowBackground,
  };
  const cellContainer = createCellBox(ctx, rowBox, templateRow.cellBoxArgs, { fill: 'transparent' });
  const cellBoxes: any[] = [];
  const borderBoxes: any[] = [];
  const textNodes: any[] = [];
  const bindings: TableRowBinding[] = [];

  templateRow.cells.forEach((cell, cellIndex) => {
    cell.isTableCell = true;
    cell.isHeader = false;
    cell.isIndex = cellIndex === 0;
    cell.isHeaderlessTable = headerlessTable;
    const kind: GraphCellKind = cellIndex === 0 ? 'key' : 'value';
    const column = options.columns[cellIndex];
    const strokeWidth = getTableStrokeWidth();
    const contentBounds = {
      ...cell.boxArgs,
      x: Math.max(0, (column?.boxArgs.x ?? cell.boxArgs.x) - templateRow.boxArgs.x),
      y: Math.max(0, cell.boxArgs.y - templateRow.boxArgs.y),
      width: column?.boxArgs.width ?? cell.boxArgs.width,
    };
    const box = createCellBox(ctx, cellContainer, contentBounds, { fill: 'transparent' });
    const borderBox = createBorderStrokeBox(
      ctx,
      cellContainer,
      contentBounds,
      ctx.styleConfig.colors.table.rowBorder,
      strokeWidth,
    );
    borderBox.visible = !hideBorders;
    box.hitType = 'all';
    box.hoverStyle = {
      fill: ctx.styleConfig.colors.table.hoverCellBackground,
    };
    const text = createCellText(ctx, box, cell, kind, options.node.kind, options.node);
    cellBoxes.push(box);
    borderBoxes.push(borderBox);
    textNodes.push(text);
    bindings.push({ cell, kind, box, text });
  });

  return {
    rowBox,
    cellContainer,
    cellBoxes,
    borderBoxes,
    textNodes,
    rowIndex: null,
    bindings,
  };
}

function unbindTableRowSlot(ctx: DrawContext, entry: TableRowRenderEntry) {
  if (entry.rowIndex == null) return;
  for (const binding of entry.bindings) {
    ctx.unregisterCellBox?.(binding.cell, binding.kind, binding.box);
    ctx.unregisterRowBox?.(binding.cell, entry.rowBox);
  }
  entry.rowIndex = null;
  entry.rowBox.visible = false;
}

function bindTableRowSlot(
  ctx: DrawContext,
  entry: TableRowRenderEntry,
  row: GraphRow,
  rowIndex: number,
  options: {
    innerOffset: number;
    rowStartY: number;
    columns: GraphTable['columns'];
    rowCount: number;
    node: GraphNode;
    bodyViewport: any;
    viewportHeight: number;
    contentHeight: number;
    force?: boolean;
  },
) {
  const hideBorders = isHeaderlessSequenceTable(options.node);
  const headerlessTable = isHeaderlessTable(options.node);
  if (entry.rowIndex === rowIndex && !options.force) return;
  if (entry.rowIndex != null) {
    for (const binding of entry.bindings) {
      ctx.unregisterCellBox?.(binding.cell, binding.kind, binding.box);
      ctx.unregisterRowBox?.(binding.cell, entry.rowBox);
    }
  }

  entry.rowBox.visible = true;
  entry.rowBox.x = row.boxArgs.x - options.innerOffset;
  entry.rowBox.y = row.boxArgs.y - options.rowStartY;
  entry.rowBox.width = row.boxArgs.width;
  entry.rowBox.height = row.boxArgs.height;

  entry.cellContainer.x = row.cellBoxArgs.x;
  entry.cellContainer.y = row.cellBoxArgs.y;
  entry.cellContainer.width = row.cellBoxArgs.width;
  entry.cellContainer.height = row.cellBoxArgs.height;

  row.cells.forEach((cell, cellIndex) => {
    cell.isTableCell = true;
    cell.isHeader = false;
    cell.isIndex = cellIndex === 0;
    cell.isHeaderlessTable = headerlessTable;
    const kind: GraphCellKind = cellIndex === 0 ? 'key' : 'value';
    const box = entry.cellBoxes[cellIndex];
    const borderBox = entry.borderBoxes[cellIndex];
    const text = entry.textNodes[cellIndex];
    const binding = entry.bindings[cellIndex];
    const column = options.columns[cellIndex];
    const contentX = Math.max(0, (column?.boxArgs.x ?? cell.boxArgs.x) - row.boxArgs.x);
    const contentY = Math.max(0, cell.boxArgs.y - row.boxArgs.y);
    const contentWidth = column?.boxArgs.width ?? cell.boxArgs.width;
    const contentHeight = cell.boxArgs.height;

    box.x = contentX;
    box.y = contentY;
    box.width = contentWidth;
    box.height = contentHeight;
    borderBox.x = contentX;
    borderBox.y = contentY;
    borderBox.width = contentWidth;
    borderBox.height = contentHeight;
    borderBox.visible = !hideBorders;
    updateCellText(ctx, text, cell, kind, options.node.kind, options.node);

    binding.cell = cell;
    binding.kind = kind;
    binding.box = box;
    binding.text = text;

    ctx.registerCellBox(cell, kind, box);
    ctx.registerRowBox(cell, entry.rowBox, options.bodyViewport, options.viewportHeight, options.contentHeight);
    ctx.registerClickTarget(box, cell, kind, options.node.kind);
    ctx.registerClickTarget(text, cell, kind, options.node.kind);
  });

  entry.rowIndex = rowIndex;
}

export function drawTableNode(ctx: DrawContext, node: GraphNode) {
  if (!node.table) return null;
  const nodeColors = isHeaderlessSequenceTable(node)
    ? (ctx.styleConfig.colors.node ?? ctx.styleConfig.colors.table)
    : ctx.styleConfig.colors.table;
  const nodeBox = createNodeBox(ctx, node, nodeColors, { stroke: isHeaderlessSequenceTable(node) });
  const tableRuntime = createTableRuntime(ctx, node, nodeBox, {
    createBodyContent: createTableBodyContent,
    drawHeader: drawTableHeader,
    createRowSlot: createTableRowSlot,
    bindRowSlot: bindTableRowSlot,
    unbindRowSlot: unbindTableRowSlot,
    removeRenderable: (target) => target?.remove?.(),
  });
  if (!tableRuntime) return { nodeBox };
  return { nodeBox, tableRuntime };
}

export const tableRuntimeOps = {
  createBodyContent: createTableBodyContent,
  drawHeader: drawTableHeader,
  createRowSlot: createTableRowSlot,
  bindRowSlot: bindTableRowSlot,
  unbindRowSlot: unbindTableRowSlot,
  removeRenderable: (target: any) => target?.remove?.(),
};

export { createTableRuntime, describeTableRuntime, patchTableStructure, patchTableContent, destroyTableRuntime };

export function drawSimpleNode(ctx: DrawContext, node: GraphNode) {
  const nodeBox = createNodeBox(ctx, node, ctx.styleConfig.colors.node);
  node.rows.forEach((row) => {
    const rowBox = createRowBox(ctx, nodeBox, row.boxArgs);
    rowBox.fill = ctx.styleConfig.colors.table.rowBackground;
    rowBox.hoverStyle = {
      fill: ctx.styleConfig.colors.table.hoverRowBackground,
    };
    const cellBox = createCellBox(ctx, rowBox, row.cellBoxArgs, { fill: 'transparent' });
    row.cells.forEach((cell, cellIndex) => {
      const isValue = cellIndex === 1;
      const box = createCellBox(ctx, cellBox, cell.boxArgs, { fill: 'transparent' });
      box.hitType = 'all';
      box.hoverStyle = {
        fill: ctx.styleConfig.colors.table.hoverCellBackground,
      };
      ctx.registerCellBox(cell, isValue ? 'value' : 'key', box);
      ctx.registerRowBox(cell, rowBox);
      const kind = isValue ? 'value' : 'key';
      ctx.registerClickTarget(box, cell, kind, node.kind);
      const text = createCellText(ctx, box, cell, kind, node.kind, node);
      ctx.registerClickTarget(text, cell, kind, node.kind);
    });
  });
  return { nodeBox };
}
