import { createLeaferVirtualList, type LeaferVirtualListHandle } from '../../../leafer-virtual-list';
import { computeTableRenderCount, type TableVisibleRange } from './table-virtual-window';
import type {
  DrawContext,
  GraphNode,
  GraphRow,
  GraphTable,
  TableRowRenderEntry,
  TableRuntime,
} from '../../../graph/graph-viewer-types';

type TableRuntimeMetrics = {
  headerHeight: number;
  showHeader: boolean;
  innerOffset: number;
  innerWidth: number;
  innerHeight: number;
  rowStartY: number;
  rowEndY: number;
  rowOffsetY: number;
  measuredRowContentHeight: number;
  rowHeight: number;
  viewportHeight: number;
  contentHeight: number;
  renderCount: number;
  layoutSignature: string;
  dataSignature: string;
};

type TableRuntimeSurface = {
  bodyViewport: any;
  bodyContent: any;
  scrollTrack: any;
  scrollThumb: any;
  headerNodes: any[];
};

type TableRuntimeOps = {
  createBodyContent: (
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
  ) => TableRuntimeSurface;
  drawHeader: (
    ctx: DrawContext,
    nodeBox: any,
    table: GraphTable,
    innerOffset: number,
    node: GraphNode,
  ) => any[];
  createRowSlot: (
    ctx: DrawContext,
    bodyContent: any,
    templateRow: GraphRow,
    options: {
      innerOffset: number;
      rowStartY: number;
      columns: GraphTable['columns'];
      node: GraphNode;
    },
  ) => TableRowRenderEntry;
  bindRowSlot: (
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
  ) => void;
  unbindRowSlot: (ctx: DrawContext, entry: TableRowRenderEntry) => void;
  removeRenderable: (target: any) => void;
};

export type InternalTableRuntime = TableRuntime & {
  node: GraphNode;
  nodeBox: any;
  bodyViewport: any;
  bodyContent: any;
  scrollTrack: any;
  scrollThumb: any;
  headerNodes: any[];
  renderedSlots: Map<number, TableRowRenderEntry>;
  scrollY: number;
  layoutSignature: string;
  dataSignature: string;
  virtualList: LeaferVirtualListHandle | null;
  scrollOwner: any;
  metrics: TableRuntimeMetrics;
  destroy: () => void;
  updateWindow: (options?: { forceRebindVisibleRows?: boolean }) => void;
};

function buildLayoutSignature(
  node: GraphNode,
  metrics: Omit<TableRuntimeMetrics, 'layoutSignature' | 'dataSignature'>,
): string {
  const columnSignature = (node.table?.columns ?? [])
    .map((cell) => `${cell.boxArgs.x}:${cell.boxArgs.y}:${cell.boxArgs.width}:${cell.boxArgs.height}`)
    .join('|');
  return [
    node.boxArgs.width,
    node.boxArgs.height,
    metrics.headerHeight,
    metrics.rowHeight,
    metrics.viewportHeight,
    metrics.rowStartY,
    columnSignature,
  ].join('::');
}

function buildDataSignature(node: GraphNode): string {
  const table = node.table;
  if (!table) return '';
  return [
    table.rows.length,
    table.columns.length,
    table.rows[0]?.cells.length ?? 0,
    table.rows[table.rows.length - 1]?.cells.length ?? 0,
  ].join(':');
}

function measureTableRuntime(ctx: DrawContext, node: GraphNode): TableRuntimeMetrics {
  const table = node.table!;
  const headerHeight = table.headerHeight ?? 0;
  const showHeader = headerHeight > 0;
  const borderWidth = ctx.styleConfig.layout.nodeBorderWidth ?? 1;
  const rowStartY = borderWidth + headerHeight;

  const rowHeight =
    table.rowHeight && table.rowHeight > 0
      ? table.rowHeight
      : (table.rows[0]?.boxArgs.height ?? ctx.styleConfig.layout.rowHeight ?? 0);
  const protocolRowContentHeight =
    table.totalHeight && table.totalHeight > 0 ? Math.max(0, table.totalHeight - headerHeight) : 0;
  const measuredRowContentHeight =
    protocolRowContentHeight > 0 ? protocolRowContentHeight : Math.max(0, rowHeight * table.rows.length);
  const rowEndY = rowStartY + measuredRowContentHeight;

  const innerOffset = borderWidth;
  const innerWidth = Math.max(0, node.boxArgs.width - innerOffset * 2);
  const innerHeight = Math.max(0, node.boxArgs.height - innerOffset * 2);
  const rowOffsetY = Math.max(0, rowStartY - innerOffset);
  const availableViewportHeight = Math.max(0, node.boxArgs.height - (innerOffset + rowOffsetY));
  const protocolViewportHeight = table.viewHeight > 0 ? Math.min(table.viewHeight, availableViewportHeight) : availableViewportHeight;
  const viewportHeight = Math.max(0, Math.min(measuredRowContentHeight || protocolViewportHeight, protocolViewportHeight));
  const contentHeight = Math.max(measuredRowContentHeight, viewportHeight);
  const overscan = Math.max(0, ctx.styleConfig.layout.tableWindowOverscan ?? 0);
  const renderCount = computeTableRenderCount({
    rowCount: table.rows.length,
    rowHeight,
    viewportHeight,
    overscan,
  });

  const baseMetrics = {
    headerHeight,
    showHeader,
    innerOffset,
    innerWidth,
    innerHeight,
    rowStartY,
    rowEndY,
    rowOffsetY,
    measuredRowContentHeight,
    rowHeight,
    viewportHeight,
    contentHeight,
    renderCount,
  };

  return {
    ...baseMetrics,
    layoutSignature: buildLayoutSignature(node, baseMetrics),
    dataSignature: buildDataSignature(node),
  };
}

function shouldEnableTableBodyScroll(metrics: TableRuntimeMetrics): boolean {
  return metrics.contentHeight > metrics.viewportHeight;
}

export function describeTableRuntime(
  ctx: DrawContext,
  node: GraphNode,
): Pick<TableRuntimeMetrics, 'layoutSignature' | 'dataSignature'> {
  const metrics = measureTableRuntime(ctx, node);
  return {
    layoutSignature: metrics.layoutSignature,
    dataSignature: metrics.dataSignature,
  };
}

function releaseAllRenderedRows(ctx: DrawContext, runtime: InternalTableRuntime, ops: TableRuntimeOps): void {
  for (const rowIndex of runtime.renderedRows.keys()) {
    const entry = runtime.renderedRows.get(rowIndex);
    if (!entry) continue;
    runtime.renderedRows.delete(rowIndex);
    ops.unbindRowSlot(ctx, entry);
  }
}

function rebuildSurface(
  ctx: DrawContext,
  runtime: InternalTableRuntime,
  node: GraphNode,
  metrics: TableRuntimeMetrics,
  ops: TableRuntimeOps,
): TableRuntimeSurface {
  runtime.headerNodes.forEach((target) => ops.removeRenderable(target));
  if (runtime.bodyViewport) {
    runtime.virtualList?.destroy();
    ops.removeRenderable(runtime.bodyViewport);
  }

  const surface = ops.createBodyContent(ctx, runtime.nodeBox, {
    innerOffset: metrics.innerOffset,
    rowOffsetY: metrics.rowOffsetY,
    innerWidth: metrics.innerWidth,
    contentHeight: metrics.contentHeight,
    viewportHeight: metrics.viewportHeight,
    enableVerticalScroll: shouldEnableTableBodyScroll(metrics),
  });
  const headerNodes = metrics.showHeader
    ? ops.drawHeader(ctx, runtime.nodeBox, node.table!, metrics.innerOffset, node)
    : [];
  return { ...surface, headerNodes };
}

function applyScrollableSurfaceMetrics(runtime: InternalTableRuntime, metrics: TableRuntimeMetrics): void {
  runtime.bodyViewport.height = metrics.viewportHeight;
  runtime.bodyViewport.width = metrics.innerWidth;
  runtime.bodyViewport.x = metrics.innerOffset;
  runtime.bodyViewport.y = metrics.innerOffset + metrics.rowOffsetY;
  (runtime.bodyViewport as { __graphViewportHeight?: number }).__graphViewportHeight = metrics.viewportHeight;
  runtime.bodyContent.height = metrics.contentHeight;
  runtime.bodyContent.width = metrics.innerWidth;
  runtime.bodyContent.x = 0;
  runtime.scrollTrack.visible = shouldEnableTableBodyScroll(metrics);
  runtime.scrollThumb.visible = shouldEnableTableBodyScroll(metrics);
}

function finalizePatchedRuntime(
  ctx: DrawContext,
  runtime: InternalTableRuntime,
  rowCount: number,
  metrics: TableRuntimeMetrics,
  ops: TableRuntimeOps,
  forceRebindVisibleRows = false,
): void {
  applyScrollableSurfaceMetrics(runtime, metrics);
  runtime.virtualList?.setOptions({
    count: rowCount,
    itemSize: metrics.rowHeight,
    viewportSize: metrics.viewportHeight,
    overscan: Math.max(0, ctx.styleConfig.layout.tableWindowOverscan ?? 0),
    scrollOffset: runtime.scrollY,
  });
  runtime.visibleRange = bindVisibleWindow(ctx, runtime, metrics, ops, forceRebindVisibleRows);
  runtime.renderedSlots = runtime.renderedRows;
}

function applyMetrics(runtime: InternalTableRuntime, node: GraphNode, metrics: TableRuntimeMetrics): void {
  runtime.node = node;
  runtime.nodeId = node.renderHandle;
  runtime.bodyHeight = metrics.viewportHeight;
  runtime.contentHeight = metrics.contentHeight;
  runtime.rowHeight = metrics.rowHeight;
  runtime.layoutSignature = metrics.layoutSignature;
  runtime.dataSignature = metrics.dataSignature;
  runtime.metrics = metrics;
}

function createVirtualListController(
  ctx: DrawContext,
  runtime: InternalTableRuntime,
  metrics: TableRuntimeMetrics,
  forceRebindVisibleRows = false,
): LeaferVirtualListHandle {
  return createLeaferVirtualList({
    count: runtime.node.table?.rows.length ?? 0,
    itemSize: metrics.rowHeight,
    viewportSize: metrics.viewportHeight,
    overscan: Math.max(0, ctx.styleConfig.layout.tableWindowOverscan ?? 0),
    scrollOffset: runtime.scrollY,
    host: {
      viewportBox: runtime.bodyViewport,
      contentBox: runtime.bodyContent,
      trackBox: runtime.scrollTrack,
      thumbBox: runtime.scrollThumb,
      bindVerticalScrollGesture: ctx.bindVerticalScrollGesture,
      bindPointerDown: ctx.bindPointerDown,
      getPointFromEvent: ctx.getPointFromEvent,
      requestRender: ctx.requestRender,
    },
    onRangeChange: (state) => {
      runtime.scrollY = state.scrollOffset;
      runtime.visibleRange = bindVisibleWindow(ctx, runtime, runtime.metrics, opsRef.current, forceRebindVisibleRows);
      runtime.renderedSlots = runtime.renderedRows;
      ctx.refreshActiveHighlight?.();
    },
  });
}

const opsRef: { current: TableRuntimeOps } = { current: null as never };

function bindVisibleWindow(
  ctx: DrawContext,
  runtime: InternalTableRuntime,
  metrics: TableRuntimeMetrics,
  ops: TableRuntimeOps,
  forceRebindVisibleRows = false,
): TableVisibleRange {
  const table = runtime.node.table!;
  const visibleRange = runtime.virtualList?.getRange() ?? { start: 0, end: table.rows.length };

  const acquireRowSlot = (row: GraphRow): TableRowRenderEntry => {
    const idleEntry = runtime.rowPool.find((candidate) => candidate.rowIndex == null);
    if (idleEntry) return idleEntry;
    const created = ops.createRowSlot(ctx, runtime.bodyContent, row, {
      innerOffset: metrics.innerOffset,
      rowStartY: metrics.rowStartY,
      columns: table.columns,
      node: runtime.node,
    });
    runtime.rowPool.push(created);
    return created;
  };

  const bindRowSlot = (
    entry: TableRowRenderEntry,
    row: GraphRow,
    rowIndex: number,
    options?: { force?: boolean },
  ): void => {
    ops.bindRowSlot(ctx, entry, row, rowIndex, {
      innerOffset: metrics.innerOffset,
      rowStartY: metrics.rowStartY,
      columns: table.columns,
      rowCount: table.rows.length,
      node: runtime.node,
      bodyViewport: runtime.bodyViewport,
      viewportHeight: metrics.viewportHeight,
      contentHeight: metrics.contentHeight,
      force: options?.force,
    });
    runtime.renderedRows.set(rowIndex, entry);
  };

  const releaseRow = (rowIndex: number): void => {
    const entry = runtime.renderedRows.get(rowIndex);
    if (!entry) return;
    runtime.renderedRows.delete(rowIndex);
    ops.unbindRowSlot(ctx, entry);
  };

  for (const rowIndex of runtime.renderedRows.keys()) {
    if (rowIndex < visibleRange.start || rowIndex >= visibleRange.end) {
      releaseRow(rowIndex);
    }
  }

  for (let rowIndex = visibleRange.start; rowIndex < visibleRange.end; rowIndex += 1) {
    const row = table.rows[rowIndex];
    if (!row) continue;
    const existing = runtime.renderedRows.get(rowIndex);
    if (existing) {
      if (forceRebindVisibleRows) {
        bindRowSlot(existing, row, rowIndex, { force: true });
      }
      continue;
    }
    bindRowSlot(acquireRowSlot(row), row, rowIndex);
  }

  return visibleRange;
}

export function createTableRuntime(
  ctx: DrawContext,
  node: GraphNode,
  nodeBox: any,
  ops: TableRuntimeOps,
): TableRuntime | null {
  if (!node.table) return null;
  opsRef.current = ops;
  const metrics = measureTableRuntime(ctx, node);
  const surface = ops.createBodyContent(ctx, nodeBox, {
    innerOffset: metrics.innerOffset,
    rowOffsetY: metrics.rowOffsetY,
    innerWidth: metrics.innerWidth,
    contentHeight: metrics.contentHeight,
    viewportHeight: metrics.viewportHeight,
    enableVerticalScroll: shouldEnableTableBodyScroll(metrics),
  });
  const headerNodes = metrics.showHeader
    ? ops.drawHeader(ctx, nodeBox, node.table, metrics.innerOffset, node)
    : [];

  const runtime = {
    node,
    nodeId: node.renderHandle,
    nodeBox,
    bodyViewport: surface.bodyViewport,
    bodyContent: surface.bodyContent,
    scrollTrack: surface.scrollTrack,
    scrollThumb: surface.scrollThumb,
    bodyHeight: metrics.viewportHeight,
    contentHeight: metrics.contentHeight,
    rowHeight: metrics.rowHeight,
    visibleRange: { start: 0, end: metrics.renderCount },
    renderedRows: new Map<number, TableRowRenderEntry>(),
    renderedSlots: new Map<number, TableRowRenderEntry>(),
    rowPool: [] as TableRowRenderEntry[],
    scrollY: 0,
    headerNodes,
    virtualList: null,
    scrollOwner: surface.bodyViewport,
    metrics,
    layoutSignature: metrics.layoutSignature,
    dataSignature: metrics.dataSignature,
    updateWindow: () => {},
    destroy: () => {},
  } as InternalTableRuntime;

  runtime.updateWindow = (options?: { forceRebindVisibleRows?: boolean }) => {
    runtime.scrollY = Math.max(0, runtime.virtualList?.getScrollOffset() ?? runtime.scrollY ?? 0);
    runtime.visibleRange = bindVisibleWindow(ctx, runtime, runtime.metrics, ops, !!options?.forceRebindVisibleRows);
    runtime.renderedSlots = runtime.renderedRows;
    ctx.refreshActiveHighlight?.();
    ctx.requestRender?.();
  };

  runtime.destroy = () => {
    runtime.virtualList?.destroy();
    releaseAllRenderedRows(ctx, runtime, ops);
    runtime.headerNodes.forEach((target) => ops.removeRenderable(target));
    runtime.headerNodes = [];
    ops.removeRenderable(runtime.bodyViewport);
    runtime.rowPool.length = 0;
    runtime.renderedSlots = runtime.renderedRows;
  };

  runtime.virtualList = createVirtualListController(ctx, runtime, metrics);
  runtime.updateWindow({ forceRebindVisibleRows: true });
  return runtime;
}

export function patchTableStructure(
  ctx: DrawContext,
  runtime: TableRuntime,
  node: GraphNode,
  ops: TableRuntimeOps,
): TableRuntime {
  const target = runtime as InternalTableRuntime;
  opsRef.current = ops;
  const metrics = measureTableRuntime(ctx, node);
  const previousScrollY = Math.max(0, target.virtualList?.getScrollOffset() ?? target.scrollY ?? 0);

  releaseAllRenderedRows(ctx, target, ops);
  target.rowPool.length = 0;

  const surface = rebuildSurface(ctx, target, node, metrics, ops);
  target.bodyViewport = surface.bodyViewport;
  target.bodyContent = surface.bodyContent;
  target.scrollTrack = surface.scrollTrack;
  target.scrollThumb = surface.scrollThumb;
  target.headerNodes = surface.headerNodes;
  target.scrollY = previousScrollY;
  applyMetrics(target, node, metrics);
  target.virtualList = createVirtualListController(ctx, target, metrics, true);
  finalizePatchedRuntime(ctx, target, node.table?.rows.length ?? 0, metrics, ops, true);
  return target;
}

export function patchTableContent(
  ctx: DrawContext,
  runtime: TableRuntime,
  node: GraphNode,
  ops: TableRuntimeOps,
  forceRebindVisibleRows = false,
): TableRuntime {
  const target = runtime as InternalTableRuntime;
  opsRef.current = ops;
  const metrics = measureTableRuntime(ctx, node);
  target.scrollY = Math.max(0, target.virtualList?.getScrollOffset() ?? target.scrollY ?? 0);
  applyMetrics(target, node, metrics);
  finalizePatchedRuntime(ctx, target, node.table?.rows.length ?? 0, metrics, ops, forceRebindVisibleRows);
  return target;
}

export function destroyTableRuntime(
  ctx: DrawContext,
  runtime: TableRuntime | null | undefined,
  ops: TableRuntimeOps,
): void {
  if (!runtime) return;
  const target = runtime as InternalTableRuntime;
  target.destroy?.();
  releaseAllRenderedRows(ctx, target, ops);
}
