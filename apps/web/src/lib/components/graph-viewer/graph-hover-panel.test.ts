import { describe, expect, it, vi } from 'vitest';
import {
  buildTooltipPanelAppConfig,
  buildGraphTooltipPanelShellMarkup,
  canOpenSubgraphPreviewForCell,
  collectTooltipPanelPrewarmCandidates,
  isGraphHoverTargetOverflowing,
  resolveGraphHoverPreviewRule,
  resolveTooltipPanelViewportSize,
} from './graph-hover-panel';
import { PathSegTag } from '@core-wasm/index'

describe('canOpenSubgraphPreviewForCell', () => {
  it('allows non-empty structured cells from scrollable headerless tables', () => {
    expect(
      canOpenSubgraphPreviewForCell(
        {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          isScrollableTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        'value',
      ),
    ).toBe(true);
  });

  it('rejects structured cells from non-scrollable headerless tables', () => {
    expect(
      canOpenSubgraphPreviewForCell(
        {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        'value',
      ),
    ).toBe(false);
  });
});
describe('resolveGraphHoverPreviewRule', () => {
  const canOpen = vi.fn(() => true);

  it('shows pre for overflowing scalar value cells', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: 'very long string',
          value: 'very long string',
          valueType: 'string',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('pre');
  });

  it('does not show pre for non-overflowing meta path', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '$.meta.short',
          value: '',
          valueType: 'object',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'meta',
        __graphNodeKind: 'table',
        isOverflow: false,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('shows subgraph for non-empty structured table cells', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: false,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('subgraph');
  });

  it('does not show hover panel for non-scrollable headerless table cells', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('shows subgraph for non-empty structured cells in scrollable headerless tables', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          isScrollableTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        } as never,
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('subgraph');
  });

  it('does not show subgraph when the table cell cannot open a panel', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: false,
      },
      () => false,
    );
    expect(result).toBeNull();
  });

  it('falls back to pre for overflowing structured values outside tables', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'object',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('pre');
  });

  it('treats empty composite cells as scalar pre previews only when overflowing', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('pre');
  });

  it('only trusts Leafer isOverflow flag', () => {
    expect(isGraphHoverTargetOverflowing({ isOverflow: true } as never)).toBe(true);
    expect(
      isGraphHoverTargetOverflowing({
        width: 120,
        height: 18,
        isOverflow: false,
        getBounds: () => ({ width: 180, height: 18 }),
      } as never),
    ).toBe(false);
  });

  it('uses resolved overflow for scalar values', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: 'https://example.com/really/long/value',
          value: 'https://example.com/really/long/value',
          valueType: 'string',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result?.previewKind).toBe('pre');
  });
});

describe('resolveTooltipPanelViewportSize', () => {
  it('caps oversized panel content to the tooltip viewport budget', () => {
    expect(resolveTooltipPanelViewportSize(2856, 168910, 2600, 1400)).toEqual({
      width: 960,
      height: 720,
    });
  });

  it('keeps smaller panel content unchanged', () => {
    expect(resolveTooltipPanelViewportSize(320, 240, 2600, 1400)).toEqual({
      width: 320,
      height: 240,
    });
  });
});

function keySeg(key: string) {
  return {
    tag: PathSegTag.KEY,
    key,
    index: 0,
  } as never;
}

function indexSeg(index: number) {
  return {
    tag: PathSegTag.INDEX,
    key: '' as never,
    index,
  } as never;
}

function structuredCell(path: any[], text = '{1}') {
  return {
    text,
    value: '',
    valueType: 'object',
    isTableCell: true,
    path,
    editable: false,
    boxArgs: {} as never,
    textArgs: {} as never,
  } as never;
}

describe('buildGraphTooltipPanelShellMarkup', () => {
  it('renders loading skeleton markup instead of an empty shell', () => {
    const markup = buildGraphTooltipPanelShellMarkup();
    expect(markup).toContain('data-tooltip-panel');
    expect(markup).toContain('graph-tooltip-panel--loading');
    expect(markup).toContain('graph-tooltip-panel-skeleton');
  });
});

describe('buildTooltipPanelAppConfig', () => {
  it('matches the main graph pan affordances for hover subgraphs', () => {
    const view = {} as HTMLDivElement;
    const config = buildTooltipPanelAppConfig(view);

    expect(config.view).toBe(view);
    expect(config.type).toBe('viewport');
    expect(config.move).toEqual({
      drag: false,
      holdSpaceKey: true,
      holdRightKey: true,
      scroll: true,
    });
    expect(config.zoom).toEqual({ disabled: true });
  });
});

describe('collectTooltipPanelPrewarmCandidates', () => {
  it('collects structured table cells in stable DFS order and deduplicates paths', () => {
    const contentPath = [keySeg('Result'), keySeg('Blocks'), indexSeg(0), keySeg('Content')];
    const taskErrorPath = [keySeg('Result'), keySeg('Blocks'), indexSeg(6), keySeg('TaskError')];
    const laterPath = [keySeg('Later')];

    const candidates = collectTooltipPanelPrewarmCandidates(
      {
        nodes: [
          {
            renderHandle: 1,
            kind: 'table',
            boxArgs: { x: 0, y: 20, width: 240, height: 120, cornerRadius: 8 },
            rows: [],
            meta: {} as never,
            path: [],
            depth: 0,
            table: {
              columns: [],
              headerHeight: 24,
              rows: [
                {
                  boxArgs: {} as never,
                  cellBoxArgs: {} as never,
                  cells: [structuredCell(contentPath), structuredCell(taskErrorPath)],
                },
              ],
            },
          } as never,
          {
            renderHandle: 2,
            kind: 'object',
            boxArgs: { x: 320, y: 20, width: 180, height: 80, cornerRadius: 8 },
            depth: 1,
            path: contentPath,
            meta: {} as never,
            rows: [
              {
                index: 1,
                boxArgs: {} as never,
                cellBoxArgs: {} as never,
                cells: [structuredCell(contentPath), structuredCell(laterPath)],
              },
            ],
          } as never,
        ],
        edges: [
          {
            fromRenderHandle: 1,
            fromRow: 1,
            toRenderHandle: 2,
            toRow: 0,
            bezierArgs: {} as never,
          },
        ],
      },
      () => true,
      10,
    );

    expect(candidates).toHaveLength(3);
    expect(candidates[0]?.cell.path).toEqual(contentPath);
    expect(candidates[1]?.cell.path).toEqual(taskErrorPath);
    expect(candidates[2]?.cell.path).toEqual(laterPath);
  });

  it('respects the prewarm limit and canOpen gate', () => {
    const firstPath = [keySeg('A')];
    const blockedPath = [keySeg('B')];
    const thirdPath = [keySeg('C')];
    const candidates = collectTooltipPanelPrewarmCandidates(
      {
        nodes: [
          {
            renderHandle: 1,
            kind: 'table',
            boxArgs: { x: 0, y: 0, width: 240, height: 120, cornerRadius: 8 },
            rows: [],
            meta: {} as never,
            path: [],
            depth: 0,
            table: {
              columns: [],
              headerHeight: 24,
              rows: [
                {
                  boxArgs: {} as never,
                  cellBoxArgs: {} as never,
                  cells: [structuredCell(firstPath), structuredCell(blockedPath), structuredCell(thirdPath)],
                },
              ],
            },
          } as never,
        ],
        edges: [],
      },
      (cell) => cell.path !== blockedPath,
      2,
    );

    expect(candidates).toHaveLength(2);
    expect(candidates[0]?.cell.path).toEqual(firstPath);
    expect(candidates[1]?.cell.path).toEqual(thirdPath);
  });

  it('uses row index directly for headerless tables when traversing child rows', () => {
    const parentPath = [keySeg('items'), indexSeg(0)];
    const childValuePath = [keySeg('child')];
    const candidates = collectTooltipPanelPrewarmCandidates(
      {
        nodes: [
          {
            renderHandle: 1,
            kind: 'table',
            boxArgs: { x: 0, y: 0, width: 240, height: 80, cornerRadius: 8 },
            rows: [],
            meta: {} as never,
            path: [],
            depth: 0,
            table: {
              columns: [],
              headerHeight: 0,
              rows: [
                {
                  boxArgs: {} as never,
                  cellBoxArgs: {} as never,
                  cells: [structuredCell(parentPath), structuredCell(parentPath)],
                },
              ],
            },
          } as never,
          {
            renderHandle: 2,
            kind: 'object',
            boxArgs: { x: 320, y: 0, width: 180, height: 80, cornerRadius: 8 },
            depth: 1,
            path: parentPath,
            meta: {} as never,
            rows: [
              {
                index: 0,
                boxArgs: {} as never,
                cellBoxArgs: {} as never,
                cells: [structuredCell(parentPath), structuredCell(childValuePath)],
              },
            ],
          } as never,
        ],
        edges: [
          {
            fromRenderHandle: 1,
            fromRow: 0,
            toRenderHandle: 2,
            toRow: 0,
            bezierArgs: {} as never,
          },
        ],
      },
      () => true,
      10,
    );

    expect(candidates).toHaveLength(2);
    expect(candidates[0]?.cell.path).toEqual(parentPath);
    expect(candidates[1]?.cell.path).toEqual(childValuePath);
  });

  it('collects scrollable headerless table cells during prewarm collection', () => {
    const parentPath = [keySeg('items'), indexSeg(0)];
    const candidates = collectTooltipPanelPrewarmCandidates(
      {
        nodes: [
          {
            renderHandle: 1,
            kind: 'table',
            boxArgs: { x: 0, y: 0, width: 240, height: 40, cornerRadius: 8 },
            rows: [],
            meta: {} as never,
            path: [],
            depth: 0,
            table: {
              columns: [],
              headerHeight: 0,
              totalHeight: 80,
              viewHeight: 40,
              rowHeight: 20,
              rows: [
                {
                  boxArgs: { x: 0, y: 0, width: 240, height: 20, cornerRadius: 0 },
                  cellBoxArgs: {} as never,
                  cells: [structuredCell(parentPath), structuredCell(parentPath)],
                },
              ],
            },
          } as never,
        ],
        edges: [],
      },
      () => true,
      10,
    );

    expect(candidates).toHaveLength(1);
    expect(candidates[0]?.cell.path).toEqual(parentPath);
    expect(candidates[0]?.cell.isScrollableTable).toBe(true);
  });

  it('skips non-scrollable headerless table cells during prewarm collection', () => {
    const parentPath = [keySeg('items'), indexSeg(0)];
    const candidates = collectTooltipPanelPrewarmCandidates(
      {
        nodes: [
          {
            renderHandle: 1,
            kind: 'table',
            boxArgs: { x: 0, y: 0, width: 240, height: 80, cornerRadius: 8 },
            rows: [],
            meta: {} as never,
            path: [],
            depth: 0,
            table: {
              columns: [],
              headerHeight: 0,
              rows: [
                {
                  boxArgs: {} as never,
                  cellBoxArgs: {} as never,
                  cells: [structuredCell(parentPath), structuredCell(parentPath)],
                },
              ],
            },
          } as never,
        ],
        edges: [],
      },
      () => true,
      10,
    );

    expect(candidates).toHaveLength(0);
  });
});
