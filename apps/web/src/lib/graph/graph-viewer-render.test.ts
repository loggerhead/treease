import { describe, expect, it, vi } from 'vitest';
import {
  createCellText,
  drawSimpleNode,
  drawTableNode,
  patchTableContent,
  patchTableStructure,
  tableRuntimeOps,
  type DrawContext,
  type GraphNode,
} from './graph-viewer-render';

class MockBox {
  x: number;
  y: number;
  width: number;
  height: number;
  fill: string | undefined;
  stroke: string | undefined;
  strokeWidth: number | undefined;
  strokeAlign: string | undefined;
  children: any[] = [];
  parent: MockBox | null = null;
  hoverStyle: Record<string, unknown> | undefined;
  hitType: string | undefined;
  overflow: string | undefined;
  scrollConfig: Record<string, unknown> | undefined;
  visible: boolean | undefined;
  scrollY = 0;
  listeners = new Map<string, Array<(event?: unknown) => void>>();

  constructor(props: Record<string, any> = {}) {
    this.x = props.x ?? 0;
    this.y = props.y ?? 0;
    this.width = props.width ?? 0;
    this.height = props.height ?? 0;
    this.fill = props.fill;
    this.stroke = props.stroke;
    this.strokeWidth = props.strokeWidth;
    this.strokeAlign = props.strokeAlign;
    this.overflow = props.overflow;
    this.scrollConfig = props.scrollConfig;
    this.visible = props.visible;
    this.scrollY = props.scrollY ?? 0;
  }

  add(child: any) {
    child.parent = this;
    this.children.push(child);
  }

  remove(child: any) {
    const index = this.children.indexOf(child);
    if (index >= 0) this.children.splice(index, 1);
    if (child && typeof child === 'object') child.parent = null;
  }

  on(event: string, callback: (event?: unknown) => void) {
    const queue = this.listeners.get(event) ?? [];
    queue.push(callback);
    this.listeners.set(event, queue);
  }

  emit(event: string, payload?: unknown) {
    for (const callback of this.listeners.get(event) ?? []) callback(payload);
  }

  scrollTo(options: { x?: number; y?: number } | number, y?: number) {
    const previous = this.scrollY;
    this.scrollY = typeof options === 'number' ? (y ?? 0) : (options.y ?? 0);
    this.emit('property.scroll', { attrName: 'scrollY', oldValue: previous, newValue: this.scrollY });
    this.emit('scroll');
    this.emit('scroll.y');
  }
}

class MockText extends MockBox {
  text: string;
  textOverflow: string | undefined;
  textWrap: string | undefined;
  fontSize: number | undefined;
  editable: boolean | undefined;
  editInner: string | undefined;

  constructor(props: Record<string, any> = {}) {
    super(props);
    this.text = props.text ?? '';
    this.textOverflow = props.textOverflow;
    this.textWrap = props.textWrap;
    this.fontSize = props.fontSize;
    this.editable = props.editable;
    this.editInner = props.editInner;
  }
}

class MockPen {
  ops: Array<{ type: 'moveTo' | 'lineTo'; x: number; y: number }> = [];
  style: Record<string, unknown> | null = null;

  setStyle(style: Record<string, unknown>) {
    this.style = style;
  }

  moveTo(x: number, y: number) {
    this.ops.push({ type: 'moveTo', x, y });
  }

  lineTo(x: number, y: number) {
    this.ops.push({ type: 'lineTo', x, y });
  }
}

function getWorldX(box: MockBox): number {
  let current: MockBox | null = box;
  let x = 0;
  while (current) {
    x += current.x;
    current = current.parent;
  }
  return x;
}

function getWorldY(box: MockBox): number {
  let current: MockBox | null = box;
  let y = 0;
  while (current) {
    y += current.y;
    current = current.parent;
  }
  return y;
}

function createTestDrawContext(overrides: Partial<DrawContext> = {}): DrawContext {
  return {
    nodeLayer: { add: vi.fn() },
    styleConfig: {
      layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
      colors: {
        node: {
          background: '#fafafa',
          border: '#bbb',
        },
        table: {
          background: '#fff',
          border: '#ccc',
          headerBackground: '#fff',
          headerBorder: '#ccc',
          rowBackground: '#fff',
          hoverRowBackground: '#eee',
          rowBorder: '#ddd',
          hoverCellBackground: '#f5f5f5',
        },
        semanticType: {
          string: '#00f',
          number: '#00f',
          boolean: '#00f',
          null: '#00f',
          object: '#00f',
          array: '#00f',
          key: '#f00',
        },
        textMuted: '#999',
      },
      fontFamily: 'sans-serif',
    },
    languageIdValue: 'json',
    fontSize: 12,
    BoxCtor: MockBox,
    TextCtor: MockText,
    PenCtor: MockPen,
    valueTypeToSemType: {
      string: 'string',
      number: 'number',
      boolean: 'boolean',
      null: 'null',
      object: 'object',
      array: 'array',
    },
    registerCellBox: vi.fn(),
    registerRowBox: vi.fn(),
    registerClickTarget: vi.fn(),
    ...overrides,
  };
}

function collectTextNodes(root: MockBox): MockText[] {
  const out: MockText[] = [];
  const stack = [...root.children];
  while (stack.length > 0) {
    const current = stack.shift();
    if (current instanceof MockText) out.push(current);
    if (current instanceof MockBox) stack.push(...current.children);
  }
  return out;
}

function createCell(text: string, valueType: GraphNode['meta']['valueType'], path: any[], x: number, y = 0, width = 40) {
  return {
    text,
    value: text,
    valueType,
    path,
    editable: false,
    boxArgs: { x, y, width, height: 20, cornerRadius: 0 },
    textArgs: {
      x,
      y: 0,
      width,
      height: 20,
      text,
      textAlign: 'left' as const,
      verticalAlign: 'middle' as const,
      editable: false,
    },
  };
}

function expectDrawResult(result: ReturnType<typeof drawTableNode>) {
  if (!result?.nodeBox || !result.tableRuntime) {
    throw new Error('expected table draw result');
  }
  return result;
}


describe('graph-viewer-render', () => {
  it('renders structured value text even when textArgs text is empty', () => {
    const valueCell = {
      ...createCell('[3]', 'array', [{ key: 'table_without_header' }], 40, 0, 80),
      value: '',
      textArgs: {
        x: 40,
        y: 0,
        width: 80,
        height: 20,
        text: '',
        textAlign: 'left' as const,
        verticalAlign: 'middle' as const,
        editable: false,
      },
    };
    const node: GraphNode = {
      renderHandle: 1,
      kind: 'object',
      depth: 0,
      path: [],
      meta: createCell('$', 'object', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 40, cornerRadius: 0 },
      rows: [
        {
          boxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [
            createCell('table_without_header', 'string', [{ key: 'table_without_header' }], 0, 0, 40),
            valueCell,
          ],
        },
      ],
    };

    const result = drawSimpleNode(createTestDrawContext(), node);

    expect(collectTextNodes(result.nodeBox).map((text) => text.text)).toContain('[3]');
  });

  it('renders empty string and null scalar placeholders in value cells', () => {
    const node: GraphNode = {
      renderHandle: 2,
      kind: 'object',
      depth: 0,
      path: [],
      meta: createCell('$', 'object', [], 0),
      boxArgs: { x: 0, y: 0, width: 160, height: 60, cornerRadius: 0 },
      rows: [
        {
          boxArgs: { x: 0, y: 0, width: 160, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 160, height: 20, cornerRadius: 0 },
          cells: [
            createCell('empty string', 'string', [{ key: 'empty string' }], 0, 0, 80),
            {
              ...createCell('', 'string', [{ key: 'empty string' }], 80, 0, 80),
              value: '',
              textArgs: {
                x: 80,
                y: 0,
                width: 80,
                height: 20,
                text: '',
                textAlign: 'left' as const,
                verticalAlign: 'middle' as const,
                editable: false,
              },
            },
          ],
        },
        {
          boxArgs: { x: 0, y: 20, width: 160, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 20, width: 160, height: 20, cornerRadius: 0 },
          cells: [
            createCell('nil', 'string', [{ key: 'nil' }], 0, 20, 80),
            {
              ...createCell('', 'null', [{ key: 'nil' }], 80, 20, 80),
              value: '',
              textArgs: {
                x: 80,
                y: 20,
                width: 80,
                height: 20,
                text: '',
                textAlign: 'left' as const,
                verticalAlign: 'middle' as const,
                editable: false,
              },
            },
          ],
        },
      ],
    };

    const result = drawSimpleNode(createTestDrawContext(), node);
    const texts = collectTextNodes(result.nodeBox).map((text) => text.text);

    expect(texts).toContain('""');
    expect(texts).toContain('null');
  });

  it('does not apply text overflow to full meta paths', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: {
            background: '#fafafa',
            border: '#bbb',
          },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const parent = new MockBox();
    const fullMetaCell = {
      ...createCell('alpha.beta.gamma.delta.epsilon', 'object', [], 0),
      value: 'alpha.beta.gamma.delta.epsilon',
      textArgs: {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
        text: 'alpha.beta.gamma.delta.epsilon',
        textAlign: 'left' as const,
        verticalAlign: 'middle' as const,
        editable: false,
      },
    };
    const truncatedMetaCell = {
      ...createCell('...epsilon', 'object', [], 0),
      value: 'alpha.beta.gamma.delta.epsilon',
      textArgs: {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
        text: '...epsilon',
        textAlign: 'left' as const,
        verticalAlign: 'middle' as const,
        editable: false,
      },
    };

    const fullMetaText = createCellText(ctx, parent, fullMetaCell, 'meta', 'object') as MockText;
    const truncatedMetaText = createCellText(ctx, parent, truncatedMetaCell, 'meta', 'object') as MockText;

    expect(fullMetaText.textOverflow).toBeUndefined();
    expect(fullMetaText.textWrap).toBe('none');
    expect(truncatedMetaText.textOverflow).toBe('...');
  });

  it('applies draw context fontSize to created text nodes', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: {
            background: '#fafafa',
            border: '#bbb',
          },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 18,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };

    const text = createCellText(ctx, new MockBox(), createCell('1', 'number', [], 0), 'value', 'scalar') as MockText;

    expect(text.fontSize).toBe(18);
  });

  it('uses draw context textEditInnerName for editable text and falls back to TextEditor', () => {
    const createCtx = (textEditInnerName?: string, editable?: boolean): DrawContext => ({
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: {
            background: '#fafafa',
            border: '#bbb',
          },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      textEditInnerName,
      editable,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    });
    const parent = new MockBox();
    const editableCell = {
      ...createCell('Alice', 'string', [{ key: 'user' }, { key: 'name' }], 0),
      textArgs: {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
        text: 'Alice',
        textAlign: 'left' as const,
        verticalAlign: 'middle' as const,
        editable: true,
      },
    };

    const defaultText = createCellText(createCtx(), parent, editableCell, 'value', 'object') as MockText;
    const tooltipText = createCellText(createCtx('TooltipTextEditor'), parent, editableCell, 'value', 'object') as MockText;
    const readonlyText = createCellText(createCtx(undefined, false), parent, editableCell, 'value', 'object') as MockText;

    expect(defaultText.editable).toBe(true);
    expect(defaultText.editInner).toBe('TextEditor');
    expect(tooltipText.editInner).toBe('TooltipTextEditor');
    expect(readonlyText.editable).toBe(false);
    expect(readonlyText.editInner).toBeUndefined();
  });

  it('respects cell-level non-editable state even when draw context is editable', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: { background: '#fafafa', border: '#bbb' },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      editable: true,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const parent = new MockBox();
    const missingCell = {
      ...createCell('miss', 'object', [], 0),
      isMissing: true,
      editable: false,
      textArgs: {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
        text: 'miss',
        textAlign: 'left' as const,
        verticalAlign: 'middle' as const,
        editable: false,
      },
    };

    const text = createCellText(ctx, parent, missingCell, 'value', 'object') as MockText;

    expect(text.editable).toBe(false);
    expect(text.editInner).toBeUndefined();
  });

  it('offsets table header and body cells into the inner content box', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: {
            background: '#fafafa',
            border: '#bbb',
          },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 10,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('table', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 121, height: 62, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [createCell('h1', 'string', [], 1, 1, 60), createCell('h2', 'string', [], 61, 1, 59)],
        headerHeight: 20,
        totalHeight: 20,
        viewHeight: 20,
        rowHeight: 20,
        rows: [
          {
            boxArgs: { x: 1, y: 21, width: 119, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 119, height: 20, cornerRadius: 0 },
            cells: [createCell('11', 'number', [], 1, 21, 60), createCell('12', 'number', [], 61, 21, 59)],
          },
        ],
      },
    };

    const renderResult = drawTableNode(ctx, node);

    const { nodeBox, tableRuntime: runtime } = expectDrawResult(renderResult);
    const bodyViewport = nodeBox.children[0] as MockBox;
    const headerFirstCell = nodeBox.children[1] as MockBox;
    const headerFirstBorder = nodeBox.children[2] as MockBox;
    const headerSecondCell = nodeBox.children[3] as MockBox;
    const headerSecondBorder = nodeBox.children[4] as MockBox;
    const rowEntry = runtime.renderedRows.get(0);
    const bodyFirstCell = rowEntry.cellBoxes[0] as MockBox;
    const bodySecondCell = rowEntry.cellBoxes[1] as MockBox;
    const bodyFirstBorder = rowEntry.borderBoxes[0] as MockBox;
    const bodySecondBorder = rowEntry.borderBoxes[1] as MockBox;

    expect(bodyViewport.x).toBe(1);
    expect(headerFirstCell.x).toBe(1);
    expect(headerSecondCell.x).toBe(61);
    expect(bodyFirstCell.x).toBe(0);
    expect(bodySecondCell.x).toBe(60);
    expect(headerFirstBorder.x).toBe(1);
    expect(headerSecondBorder.x).toBe(61);
    expect(bodyFirstBorder.x).toBe(0);
    expect(bodySecondBorder.x).toBe(60);
    expect(getWorldX(headerSecondBorder)).toBe(getWorldX(bodySecondBorder));
    expect(rowEntry.rowBox.x).toBe(0);
  });

  it('renders table header text with the key semantic color', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 1,
      kind: 'object',
      depth: 0,
      meta: createCell('table', 'object', [], 0),
      path: [],
      boxArgs: { x: 0, y: 0, width: 121, height: 62, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [createCell('header', 'string', [], 1, 1, 60)],
        headerHeight: 20,
        totalHeight: 20,
        viewHeight: 20,
        rowHeight: 20,
        rows: [
          {
            boxArgs: { x: 1, y: 21, width: 119, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 119, height: 20, cornerRadius: 0 },
            cells: [createCell('value', 'string', [], 1, 21, 60)],
          },
        ],
      },
    };

    const renderResult = expectDrawResult(drawTableNode(ctx, node));
    const headerCell = renderResult.nodeBox.children[1] as MockBox;
    const headerText = headerCell.children.find((child) => child instanceof MockText && child.text === 'header') as
      | MockText
      | undefined;

    expect(headerText?.fill).toBe('#f00');
  });

  it('keeps the first body column divider aligned with the corrected local cell width', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          node: {
            background: '#fafafa',
            border: '#bbb',
          },
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 11,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('table', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 121, height: 62, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 20,
        viewHeight: 20,
        rowHeight: 20,
        rows: [
          {
            boxArgs: { x: 1, y: 1, width: 119, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 119, height: 20, cornerRadius: 0 },
            cells: [createCell('11', 'number', [], 1, 1, 60), createCell('12', 'number', [], 61, 1, 59)],
          },
        ],
      },
    };

    const drawResult = expectDrawResult(drawTableNode(ctx, node));
    const runtime = drawResult.tableRuntime;
    const firstCellBox = runtime.renderedRows.get(0).cellBoxes[0] as MockBox;
    const firstBorderBox = runtime.renderedRows.get(0).borderBoxes[0] as MockBox;
    const firstValueText = runtime.renderedRows.get(0).textNodes[1] as MockText;

    expect(firstCellBox.x).toBe(0);
    expect(runtime.renderedRows.get(0).rowBox.x).toBe(0);
    expect(firstCellBox.width).toBe(60);
    expect(firstCellBox.stroke).toBeUndefined();
    expect(firstBorderBox.stroke).toBe('#ddd');
    expect(firstBorderBox.strokeWidth).toBe(1);
    expect(firstBorderBox.strokeAlign).toBe('center');
    expect(firstBorderBox.visible).toBe(false);
    expect(firstBorderBox.x).toBe(0);
    expect(firstBorderBox.width).toBe(60);
    expect(drawResult.nodeBox.fill).toBe('#fafafa');
    expect(drawResult.nodeBox.stroke).toBe('#bbb');
    expect((firstValueText as any).textAlign).toBe('right');
  });

  it('registers table row cells with key/value kinds and table node kind', () => {
    const registerCellBox = vi.fn();
    const registerClickTarget = vi.fn();
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'yaml',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox,
      registerRowBox: vi.fn(),
      registerClickTarget,
    };
    const path = [{ tag: 0, key: 'flags', index: 0 }, { tag: 1, key: null, index: 1 }];
    const node: GraphNode = {
      renderHandle: 1,
      kind: 'table',
      depth: 0,
      path,
      meta: createCell('flags', 'array', path, 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 60, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 20,
        viewHeight: 20,
        rowHeight: 20,
        rows: [
          {
            boxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
            cells: [createCell('1', 'number', path, 0), createCell('false', 'boolean', path, 40)],
          },
        ],
      },
    };

    drawTableNode(ctx, node);

    expect(registerCellBox).toHaveBeenNthCalledWith(1, expect.any(Object), 'key', expect.any(MockBox));
    expect(registerCellBox).toHaveBeenNthCalledWith(2, expect.any(Object), 'value', expect.any(MockBox));
    expect(registerClickTarget).toHaveBeenNthCalledWith(1, expect.any(MockBox), expect.any(Object), 'key', 'table');
    expect(registerClickTarget).toHaveBeenNthCalledWith(3, expect.any(MockBox), expect.any(Object), 'value', 'table');
  });


  it('uses current count to derive viewport height while keeping scroll owner stable for small streamed tables', () => {
    const registerRowBox = vi.fn();
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox,
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 12,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 101, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 20,
        rows: Array.from({ length: 3 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: 21 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 0, 0, 120)],
        })),
      },
    };

    drawTableNode(ctx, node);

    const nodeBox = (ctx.nodeLayer.add as any).mock.calls[0][0] as MockBox;
    const bodyViewport = nodeBox.children[0] as MockBox;
    const bodyContent = bodyViewport.children[0] as MockBox;

    expect(bodyViewport.x).toBe(1);
    expect(bodyViewport.y).toBe(21);
    expect(bodyViewport.height).toBe(60);
    expect((bodyViewport as any).__graphViewportHeight).toBe(60);
    expect(bodyViewport.overflow).toBeUndefined();
    expect(bodyContent.height).toBe(60);
    expect(registerRowBox).toHaveBeenCalledTimes(3);
  });

  it('uses the measured body height when protocol viewHeight is absent', () => {
    const registerRowBox = vi.fn();
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox,
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 13,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 2000, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 20,
        rows: Array.from({ length: 60 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: 21 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 0, 0, 120)],
        })),
      },
    };

    drawTableNode(ctx, node);

    const nodeBox = (ctx.nodeLayer.add as any).mock.calls[0][0] as MockBox;
    const bodyViewport = nodeBox.children[0] as MockBox;
    const bodyContent = bodyViewport.children[0] as MockBox;

    expect(bodyViewport.x).toBe(1);
    expect(bodyViewport.y).toBe(21);
    expect(bodyViewport.height).toBe(1200);
    expect((bodyViewport as any).__graphViewportHeight).toBe(1200);
    expect((bodyViewport.children[1] as any).visible).toBe(false);
    expect(bodyContent.height).toBe(1200);
    expect(registerRowBox).toHaveBeenCalledTimes(60);
  });

  it('uses the realized row list height for body content instead of protocol totalHeight', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 9,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 101, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 20,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows: Array.from({ length: 3 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: 21 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 0, 0, 120)],
        })),
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    expect(runtime.bodyHeight).toBe(80);
    expect(runtime.contentHeight).toBe(220);
    expect(runtime.bodyContent.height).toBe(60);
    expect(runtime.bodyViewport.height).toBe(80);
    expect((runtime.bodyViewport as any).__graphViewportHeight).toBe(80);
    expect(runtime.bodyViewport.overflow).toBeUndefined();
  });

  it('uses the rendered node height as the table viewport when streaming table metrics lag behind', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 91,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('Blocks', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 600, height: 266, cornerRadius: 0 },
      rows: [],
      table: {
        columns: Array.from({ length: 3 }, (_, index) => createCell(`h${index}`, 'string', [], index * 100, 1, 100)),
        headerHeight: 22,
        totalHeight: 44,
        viewHeight: 44,
        rowHeight: 22,
        rows: Array.from({ length: 11 }, (_, rowIndex) => ({
          boxArgs: { x: 1, y: 23 + rowIndex * 22, width: 300, height: 22, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 300, height: 22, cornerRadius: 0 },
          cells: [
            createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 60),
            createCell(`id-${rowIndex}`, 'string', [{ row: rowIndex }], 60, 0, 120),
            createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 180, 0, 120),
          ],
        })),
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    expect(runtime.bodyHeight).toBe(22);
    expect(runtime.bodyViewport.height).toBe(22);
    expect(runtime.contentHeight).toBe(22);
    expect(runtime.visibleRange).toEqual({ start: 0, end: 2 });
    expect(runtime.bodyViewport.overflow).toBeUndefined();
  });

  it('renders only the initial table window while keeping full content height', () => {
    const nodeLayer = { add: vi.fn() };
    const registerRowBox = vi.fn();
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer,
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox,
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 7,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    expect(runtime.visibleRange).toEqual({ start: 0, end: 5 });
    expect([...runtime.renderedRows.keys()]).toEqual([0, 1, 2, 3, 4]);
    expect(runtime.bodyViewport.overflow).toBeUndefined();
    expect(runtime.bodyHeight).toBe(80);
    expect(runtime.bodyContent.height).toBe(240);
    expect(runtime.bodyViewport.height).toBe(80);
    expect((runtime.bodyViewport as any).__graphViewportHeight).toBe(80);
    expect(runtime.rowHeight).toBe(20);
    expect(registerRowBox).toHaveBeenCalledTimes(10);
  });

  it('marks visible cells from scrollable headerless tables', () => {
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, rowIndex === 0 ? 'object' : 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const node: GraphNode = {
      renderHandle: 17,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    expectDrawResult(drawTableNode(createTestDrawContext(), node));

    expect((rows[0]!.cells[1] as any).isHeaderlessTable).toBe(true);
    expect((rows[0]!.cells[1] as any).isScrollableTable).toBe(true);
  });

  it('updates the rendered table window after scroll', () => {
    const rowStartY = 40;
    const refreshActiveHighlight = vi.fn();
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: rowStartY + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
      refreshActiveHighlight,
    };
    const node: GraphNode = {
      renderHandle: 8,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    const initialRefreshCalls = refreshActiveHighlight.mock.calls.length;
    runtime.bodyViewport.scrollTo({ x: 0, y: 80 });

    expect(runtime.visibleRange).toEqual({ start: 4, end: 9 });
    expect([...runtime.renderedRows.keys()]).toEqual([4, 5, 6, 7, 8]);
    expect(runtime.bodyViewport.overflow).toBeUndefined();

    const renderedValueTexts = [...runtime.renderedRows.values()].map((entry) => (entry.textNodes[1] as MockText).text);
    expect(renderedValueTexts).toEqual(['value-4', 'value-5', 'value-6', 'value-7', 'value-8']);
    expect(refreshActiveHighlight.mock.calls.length).toBeGreaterThan(initialRefreshCalls);
    expect(renderedValueTexts).not.toContain('value-0');
    expect(renderedValueTexts).not.toContain('value-1');
  });

  it('keeps header and body stroke borders on the same grid lines', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 801,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 61, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [
          createCell('#', 'number', [], 0, 0, 40),
          createCell('title', 'string', [], 40, 0, 80),
        ],
        headerHeight: 20,
        rowHeight: 20,
        rows: [
          {
            boxArgs: { x: 0, y: 20, width: 120, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
            cells: [
              createCell('0', 'number', [{ row: 0 }], 0, 0, 40),
              createCell('title-0', 'string', [{ row: 0 }], 40, 0, 80),
            ],
          },
        ],
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    const nodeBox = (ctx.nodeLayer.add as any).mock.calls[0][0] as MockBox;
    const headerCell = nodeBox.children[1] as MockBox;
    const headerBorderBox = nodeBox.children[2] as MockBox;
    const secondHeaderCell = nodeBox.children[3] as MockBox;
    const secondHeaderBorderBox = nodeBox.children[4] as MockBox;
    const rowEntry = runtime.renderedRows.get(0)!;
    const indexCell = rowEntry.cellBoxes[0] as MockBox;
    const valueCell = rowEntry.cellBoxes[1] as MockBox;
    const indexBorder = rowEntry.borderBoxes[0] as MockBox;
    const valueBorder = rowEntry.borderBoxes[1] as MockBox;

    expect(nodeBox.stroke).toBeUndefined();
    expect(nodeBox.strokeWidth).toBeUndefined();
    expect(headerCell.stroke).toBeUndefined();
    expect(headerBorderBox.stroke).toBe('#ccc');
    expect(headerBorderBox.strokeWidth).toBe(1);
    expect(headerBorderBox.strokeAlign).toBe('center');
    expect(indexCell.stroke).toBeUndefined();
    expect(valueCell.stroke).toBeUndefined();
    expect(indexBorder.stroke).toBe('#ddd');
    expect(indexBorder.strokeWidth).toBe(1);
    expect(indexBorder.strokeAlign).toBe('center');
    expect(valueBorder.stroke).toBe('#ddd');
    expect(valueBorder.strokeWidth).toBe(1);
    expect(valueBorder.strokeAlign).toBe('center');
    expect(headerCell.x).toBe(0);
    expect(secondHeaderCell.x).toBe(40);
    expect(indexCell.x).toBe(0);
    expect(valueCell.x).toBe(40);
    expect(headerBorderBox.x).toBe(0);
    expect(secondHeaderBorderBox.x).toBe(40);
    expect(indexBorder.x).toBe(0);
    expect(valueBorder.x).toBe(40);
    expect(getWorldX(secondHeaderBorderBox)).toBe(getWorldX(valueBorder));
    expect(getWorldY(headerBorderBox) + headerBorderBox.height).toBe(getWorldY(indexBorder));
  });

  it('computes the visible table window from body scroll when rows start below the header', () => {
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: 21 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 81,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 101, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [createCell('name', 'string', [], 0, 0, 120)],
        headerHeight: 20,
        totalHeight: 260,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    runtime.bodyViewport.scrollTo({ x: 0, y: 80 });

    expect(runtime.visibleRange).toEqual({ start: 4, end: 9 });
    expect([...runtime.renderedRows.keys()]).toEqual([4, 5, 6, 7, 8]);
    expect([...runtime.renderedRows.values()].map((entry) => (entry.textNodes[1] as MockText).text)).toEqual([
      'value-4',
      'value-5',
      'value-6',
      'value-7',
      'value-8',
    ]);
  });

  it('keeps body viewport origin anchored to border plus header even when row y is shifted', () => {
    const rows = Array.from({ length: 3 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: 40 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 82,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 101, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [createCell('name', 'string', [], 0, 0, 120)],
        headerHeight: 20,
        totalHeight: 80,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;

    expect(runtime.bodyViewport.y).toBe(21);
    expect(runtime.bodyContent.height).toBe(60);
    expect(runtime.bodyViewport.height).toBe(60);
  });

  it('reuses pooled row instances after scroll', () => {
    const rowStartY = 40;
    const registerCellBox = vi.fn();
    const unregisterCellBox = vi.fn();
    const registerRowBox = vi.fn();
    const unregisterRowBox = vi.fn();
    const registerClickTarget = vi.fn();
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: rowStartY + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox,
      unregisterCellBox,
      registerRowBox,
      unregisterRowBox,
      registerClickTarget,
    };
    const node: GraphNode = {
      renderHandle: 9,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    const initialEntries = [...runtime.renderedRows.values()];
    const initialRowBoxes = initialEntries.map((entry) => entry.rowBox);
    const initialTextNodes = initialEntries.map((entry) => entry.textNodes[1]);
    const initialPoolSize = runtime.rowPool.length;
    const initialRegisterCellBoxCalls = registerCellBox.mock.calls.length;
    const initialRegisterRowBoxCalls = registerRowBox.mock.calls.length;
    const initialRegisterClickTargetCalls = registerClickTarget.mock.calls.length;

    runtime.bodyViewport.scrollTo({ x: 0, y: 80 });

    expect(runtime.rowPool).toHaveLength(initialPoolSize);
    const nextRowBoxes = [...runtime.renderedRows.values()].map((entry) => entry.rowBox);
    const nextTextNodes = [...runtime.renderedRows.values()].map((entry) => entry.textNodes[1]);
    expect(new Set(nextRowBoxes)).toEqual(new Set(initialRowBoxes));
    expect(new Set(nextTextNodes)).toEqual(new Set(initialTextNodes));
    expect([...runtime.renderedRows.keys()]).toEqual([4, 5, 6, 7, 8]);
    expect([...runtime.renderedRows.values()].map((entry) => (entry.textNodes[1] as MockText).text)).toEqual([
      'value-4',
      'value-5',
      'value-6',
      'value-7',
      'value-8',
    ]);
    expect(unregisterCellBox).toHaveBeenCalledTimes(8);
    expect(unregisterRowBox).toHaveBeenCalledTimes(8);
    expect(registerCellBox.mock.calls.length - initialRegisterCellBoxCalls).toBe(8);
    expect(registerRowBox.mock.calls.length - initialRegisterRowBoxCalls).toBe(8);
    expect(registerClickTarget.mock.calls.length - initialRegisterClickTargetCalls).toBe(16);
  });

  it('patches table content without recreating viewport or row pool', () => {
    const registerCellBox = vi.fn();
    const unregisterCellBox = vi.fn();
    const registerRowBox = vi.fn();
    const unregisterRowBox = vi.fn();
    const registerClickTarget = vi.fn();
    const rows = Array.from({ length: 12 }, (_, rowIndex) => ({
      boxArgs: { x: 0, y: 40 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
      cells: [
        createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
        createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
      ],
    }));
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
      valueTypeToSemType: {
        string: 'string',
        number: 'number',
        boolean: 'boolean',
        null: 'null',
        object: 'object',
        array: 'array',
      },
      registerCellBox,
      unregisterCellBox,
      registerRowBox,
      unregisterRowBox,
      registerClickTarget,
    };
    const node: GraphNode = {
      renderHandle: 10,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows,
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    runtime.bodyViewport.scrollTo({ x: 0, y: 80 });

    const initialBodyViewport = runtime.bodyViewport;
    const initialBodyContent = runtime.bodyContent;
    const initialPool = runtime.rowPool;
    const initialRowBoxes = [...runtime.renderedRows.values()].map((entry) => entry.rowBox);
    const nextNode: GraphNode = {
      ...node,
      table: {
        ...node.table!,
        rows: rows.map((row, rowIndex) => ({
          ...row,
          cells: [
            row.cells[0],
            createCell(`updated-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
          ],
        })),
      },
    };

    patchTableContent(ctx, runtime, nextNode, tableRuntimeOps);

    expect(runtime.bodyViewport).toBe(initialBodyViewport);
    expect(runtime.bodyContent).toBe(initialBodyContent);
    expect(runtime.rowPool).toBe(initialPool);
    expect(runtime.bodyViewport.scrollY).toBe(80);
    expect(runtime.bodyViewport.overflow).toBeUndefined();
    expect(runtime.visibleRange).toEqual({ start: 4, end: 9 });
    expect([...runtime.renderedRows.values()].map((entry) => (entry.textNodes[1] as MockText).text)).toEqual([
      'value-4',
      'value-5',
      'value-6',
      'value-7',
      'value-8',
    ]);
    expect(new Set([...runtime.renderedRows.values()].map((entry) => entry.rowBox))).toEqual(new Set(initialRowBoxes));
  });

  it('patches table structure by rebuilding viewport while preserving scroll position', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
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
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 11,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 81, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 80,
        rowHeight: 20,
        rows: Array.from({ length: 12 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: 40 + rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [
            createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
            createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
          ],
        })),
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    const initialBodyViewport = runtime.bodyViewport;
    const initialBodyContent = runtime.bodyContent;
    runtime.bodyViewport.scrollTo({ x: 0, y: 80 });

    const nextNode: GraphNode = {
      ...node,
      boxArgs: { ...node.boxArgs, width: 160 },
      table: {
        ...node.table!,
        rows: node.table!.rows.map((row) => ({
          ...row,
          boxArgs: { ...row.boxArgs, width: 160 },
          cellBoxArgs: { ...row.cellBoxArgs, width: 160 },
          cells: [
            createCell(row.cells[0].text, 'number', row.cells[0].path, 0, 0, 60),
            createCell((row.cells[1] as any).text, 'string', row.cells[1].path, 60, 0, 100),
          ],
        })),
      },
    };

    patchTableStructure(ctx, runtime, nextNode, tableRuntimeOps);

    expect(runtime.bodyViewport).not.toBe(initialBodyViewport);
    expect(runtime.bodyContent).not.toBe(initialBodyContent);
    expect(runtime.bodyViewport.scrollY).toBe(80);
    expect(runtime.bodyViewport.overflow).toBeUndefined();
    expect(runtime.bodyViewport.width).toBe(158);
    expect(runtime.bodyContent.width).toBe(158);
  });

  it('enables y-scroll after table rows grow beyond 50 during structure patch', () => {
    const ctx: DrawContext = {
      nodeLayer: { add: vi.fn() },
      styleConfig: {
        layout: { nodeBorderWidth: 1, rowHeight: 20, rowPaddingInline: 8, headerFontWeight: 600, tableWindowOverscan: 1 },
        colors: {
          table: {
            background: '#fff',
            border: '#ccc',
            headerBackground: '#fff',
            headerBorder: '#ccc',
            rowBackground: '#fff',
            hoverRowBackground: '#eee',
            rowBorder: '#ddd',
            hoverCellBackground: '#f5f5f5',
          },
          semanticType: {
            string: '#00f',
            number: '#00f',
            boolean: '#00f',
            null: '#00f',
            object: '#00f',
            array: '#00f',
            key: '#f00',
          },
          textMuted: '#999',
        },
        fontFamily: 'sans-serif',
      },
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockPen,
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
      registerClickTarget: vi.fn(),
    };
    const node: GraphNode = {
      renderHandle: 14,
      kind: 'table',
      depth: 0,
      path: [],
      meta: createCell('items', 'array', [], 0),
      boxArgs: { x: 0, y: 0, width: 120, height: 1200, cornerRadius: 0 },
      rows: [],
      table: {
        columns: [],
        headerHeight: 0,
        totalHeight: 240,
        viewHeight: 240,
        rowHeight: 20,
        rows: Array.from({ length: 12 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [
            createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
            createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
          ],
        })),
      },
    };

    const runtime = expectDrawResult(drawTableNode(ctx, node)).tableRuntime;
    expect(runtime.bodyViewport.overflow).toBeUndefined();

    const nextNode: GraphNode = {
      ...node,
      table: {
        ...node.table!,
        totalHeight: 1200,
        viewHeight: 1000,
        rows: Array.from({ length: 60 }, (_, rowIndex) => ({
          boxArgs: { x: 0, y: rowIndex * 20, width: 120, height: 20, cornerRadius: 0 },
          cellBoxArgs: { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 },
          cells: [
            createCell(String(rowIndex), 'number', [{ row: rowIndex }], 0, 0, 40),
            createCell(`value-${rowIndex}`, 'string', [{ row: rowIndex }], 40, 0, 80),
          ],
        })),
      },
    };

    patchTableStructure(ctx, runtime, nextNode, tableRuntimeOps);

    expect((runtime.bodyViewport.children[1] as any).visible).toBe(true);
  });
});
