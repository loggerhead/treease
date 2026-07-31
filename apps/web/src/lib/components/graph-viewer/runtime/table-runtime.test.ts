import { describe, expect, it, vi } from 'vitest';
import { createTableRuntime, patchTableContent } from '@treease/graph-viewer-runtime';
import type { DrawContext, GraphCell, GraphNode, GraphRow } from '@treease/graph-viewer-runtime';

class MockBox {
  x = 0;
  y = 0;
  width = 0;
  height = 0;
  visible = true;

  constructor(props: Partial<MockBox> = {}) {
    Object.assign(this, props);
  }

  removeAll(): void {}
  remove(): void {}
}

function makeCell(text: string, rowIndex: number): GraphCell {
  return {
    text,
    value: text,
    valueType: 'string',
    path: [{ tag: 1, index: rowIndex, key: '' }],
    editable: false,
    boxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
    textArgs: { x: 0, y: 0, width: 120, height: 20, text, textAlign: 'left', verticalAlign: 'middle', editable: false },
  };
}

function makeRow(index: number): GraphRow {
  return {
    boxArgs: { x: 0, y: index * 20, width: 120, height: 20, cornerRadius: 0 },
    cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
    cells: [makeCell(`row-${index}`, index)],
  };
}

function makeNode(rowCount: number): GraphNode {
  const rows = Array.from({ length: rowCount }, (_, index) => makeRow(index));
  return {
    renderHandle: 1,
    kind: 'table',
    depth: 0,
    boxArgs: { x: 0, y: 0, width: 124, height: 102, cornerRadius: 4 },
    path: [],
    meta: makeCell('$', 0),
    rows: [],
    table: {
      columns: [makeCell('name', 0)],
      rows,
      headerHeight: 0,
      totalHeight: rowCount * 20,
      viewHeight: 100,
      rowHeight: 20,
    },
  };
}

function makeContext(): DrawContext {
  return {
    nodeLayer: new MockBox(),
    styleConfig: { layout: { nodeBorderWidth: 1, rowHeight: 20, tableWindowOverscan: 0 } },
    languageIdValue: 'json',
    fontSize: 14,
    BoxCtor: MockBox,
    TextCtor: MockBox,
    PenCtor: MockBox,
    valueTypeToSemType: {
      string: 'string',
      number: 'number',
      boolean: 'boolean',
      null: 'null',
      object: 'object',
      array: 'array',
    },
    registerCellBox: vi.fn(),
    unregisterCellBox: vi.fn(),
    registerRowBox: vi.fn(),
    unregisterRowBox: vi.fn(),
    registerClickTarget: vi.fn(() => 'target'),
    requestRender: vi.fn(),
    refreshActiveHighlight: vi.fn(),
  };
}

describe('table-runtime', () => {
  it('does not rebind visible rows when appended rows are outside the visible window', () => {
    const ctx = makeContext();
    const bindRowSlot = vi.fn((_ctx: unknown, entry: { rowIndex: number | null }, _row: unknown, rowIndex: number) => {
      entry.rowIndex = rowIndex;
    });
    const ops = {
      createBodyContent: () => ({
        bodyViewport: new MockBox({ width: 122, height: 100 }),
        bodyContent: new MockBox({ width: 122, height: 200 }),
        scrollTrack: new MockBox(),
        scrollThumb: new MockBox(),
        headerNodes: [],
      }),
      drawHeader: () => [],
      createRowSlot: () => ({
        rowBox: new MockBox(),
        cellContainer: new MockBox(),
        cellBoxes: [],
        cellSelectionDecorations: [],
        borderBoxes: [],
        textNodes: [],
        rowSelectionDecoration: new MockBox(),
        rowIndex: null,
        bindings: [],
      }),
      bindRowSlot,
      unbindRowSlot: vi.fn((_ctx: unknown, entry: { rowIndex: number | null }) => {
        entry.rowIndex = null;
      }),
      removeRenderable: vi.fn(),
    } satisfies Parameters<typeof createTableRuntime>[3];

    const initialRuntime = createTableRuntime(ctx, makeNode(10), new MockBox(), ops);
    if (!initialRuntime) throw new Error('table runtime was not created');
    bindRowSlot.mockClear();

    patchTableContent(ctx, initialRuntime, makeNode(11), ops);

    expect(bindRowSlot).not.toHaveBeenCalled();
  });
});
