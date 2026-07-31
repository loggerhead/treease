import { describe, expect, it, vi } from 'vitest';
import { defaultGraphViewerRenderConfig } from './config';
import { drawSimpleNode } from './render';
import type { DrawContext, GraphCell, GraphNode } from './types';

class MockBox {
  children: MockBox[] = [];
  parent: MockBox | null = null;
  [key: string]: unknown;

  constructor(config: Record<string, unknown> = {}) {
    Object.assign(this, config);
  }

  add(child: MockBox) {
    child.parent = this;
    this.children.push(child);
  }
}

class MockText extends MockBox {}

function cell(text: string, x: number, width: number): GraphCell {
  return {
    text,
    value: text,
    valueType: 'string',
    path: [{ key: text }],
    editable: true,
    boxArgs: { x, y: 0, width, height: 22, cornerRadius: 0 },
    textArgs: {
      x: 0,
      y: 0,
      width,
      height: 22,
      text,
      textAlign: 'left',
      verticalAlign: 'middle',
      editable: true,
    },
  };
}

describe('graph cell selection render structure', () => {
  it('creates named row and cell decorations before textual content', () => {
    const nodeLayer = new MockBox();
    const registerCellBox = vi.fn();
    const registerRowBox = vi.fn();
    const ctx: DrawContext = {
      nodeLayer,
      styleConfig: defaultGraphViewerRenderConfig,
      languageIdValue: 'json',
      fontSize: 12,
      BoxCtor: MockBox,
      TextCtor: MockText,
      PenCtor: MockBox,
      registerCellBox,
      registerRowBox,
      registerClickTarget: vi.fn(() => 'target'),
    };
    const node: GraphNode = {
      renderHandle: 1,
      kind: 'object',
      depth: 0,
      path: [],
      boxArgs: { x: 0, y: 0, width: 200, height: 22, cornerRadius: 0 },
      meta: cell('$', 0, 200),
      rows: [{
        boxArgs: { x: 0, y: 0, width: 200, height: 22, cornerRadius: 0 },
        cellBoxArgs: { x: 0, y: 0, width: 200, height: 22, cornerRadius: 0 },
        cells: [cell('img', 0, 80), cell('https://treease.com/logo.png', 80, 120)],
      }],
    };

    drawSimpleNode(ctx, node);

    const rowBox = nodeLayer.children[0]?.children[0];
    const rowDecoration = rowBox?.children[0];
    const cellContainer = rowBox?.children[1];
    const valueBox = cellContainer?.children[1];
    const valueDecoration = valueBox?.children[0];
    const valueText = valueBox?.children[1];
    expect(rowDecoration).toMatchObject({ name: 'graph-selection-decoration', visible: false, fill: '#e6f0ff' });
    expect(valueDecoration).toMatchObject({ name: 'graph-selection-decoration', visible: false, fill: '#ffe27a' });
    expect(valueText).toBeInstanceOf(MockText);
    expect(registerCellBox.mock.calls[1]?.[3]).toBe(valueDecoration);
    expect(registerRowBox.mock.calls[1]?.[5]).toBe(rowDecoration);
  });
});
