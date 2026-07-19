import { describe, expect, it } from 'vitest';
import { GraphKind, PathSegTag, SemType } from '@core-wasm/index'

import { normalizeRawCell, normalizeRawEdge, normalizeRawNode } from './graph-delta-normalize';

const boxArgs = { x: 0, y: 0, width: 120, height: 20, cornerRadius: 0 };
const textArgs = { x: 0, y: 0, width: 120, height: 20, textAlign: 0, textVerticalAlign: 1, editable: 0 };

function keySeg(key: string) {
  return { tag: PathSegTag.KEY, key, index: 0 };
}

function rawCell(options: { text: string; value: string; semType: SemType; path?: unknown[]; textArgsText?: string }) {
  return {
    semType: options.semType,
    path: options.path ?? [],
    text: options.text,
    value: options.value,
    formatText: '',
    boxArgs,
    textArgs: {
      ...textArgs,
      text: options.textArgsText ?? options.text,
    },
  };
}

describe('graph-delta-normalize', () => {
  it('keeps core display text and value for structured root rows', () => {
    const rawNode = {
      renderHandle: 1,
      kind: GraphKind.OBJECT,
      path: [],
      depth: 0,
      boxArgs: { x: 0, y: 0, width: 200, height: 80, cornerRadius: 0 },
      meta: rawCell({ text: '$', value: '$', semType: SemType.MAP }),
      rows: [
        {
          index: 0,
          boxArgs,
          cellBoxArgs: boxArgs,
          cells: [
            rawCell({ text: 'table_without_header', value: 'table_without_header', semType: SemType.STR, path: [keySeg('table_without_header')] }),
            rawCell({ text: '[3]', value: '', semType: SemType.SEQ, path: [keySeg('table_without_header')], textArgsText: '' }),
          ],
        },
      ],
    };

    const node = normalizeRawNode(rawNode);
    expect(node.rows[0]?.cells[1]?.text).toBe('[3]');
    expect(node.rows[0]?.cells[1]?.textArgs.text).toBe('[3]');
    expect(node.rows[0]?.cells[1]?.value).toBe('[3]');
  });

  it('accepts boolean editable flags from document protocol cells', () => {
    const cell = normalizeRawCell({
      ...rawCell({ text: 'Alice', value: 'Alice', semType: SemType.STR, path: [keySeg('user'), keySeg('name')] }),
      textArgs: {
        ...textArgs,
        text: 'Alice',
        editable: true,
      },
    });

    expect(cell.editable).toBe(true);
    expect(cell.textArgs.editable).toBe(true);
  });

  it('preserves Core float and nil semantic types for graph rendering', () => {
    expect(normalizeRawCell(rawCell({ text: '1.0', value: '1.0', semType: SemType.FLOAT })).semType).toBe(SemType.FLOAT);
    expect(normalizeRawCell(rawCell({ text: 'null', value: 'null', semType: SemType.NIL })).semType).toBe(SemType.NIL);
  });

  it('normalizes flat core edge bezier fields into renderer camelCase fields', () => {
    const edge = normalizeRawEdge({
      fromRenderHandle: 1,
      fromKind: GraphKind.OBJECT,
      fromPath: [],
      fromRow: 0,
      toRenderHandle: 2,
      toKind: GraphKind.TABLE,
      toPath: [keySeg('table_without_header')],
      toRow: 0,
      bezierFromX: 10,
      bezierFromY: 20,
      bezierC1x: 30,
      bezierC1y: 40,
      bezierC2x: 50,
      bezierC2y: 60,
      bezierToX: 70,
      bezierToY: 80,
    });
    expect(edge.bezierArgs).toEqual({
      fromX: 10,
      fromY: 20,
      c1x: 30,
      c1y: 40,
      c2x: 50,
      c2y: 60,
      toX: 70,
      toY: 80,
    });
  });
});
