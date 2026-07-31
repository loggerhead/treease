import { describe, expect, it, vi } from 'vitest';

import { buildPathKey } from '../../graph/graph-viewer-path';
import type { GraphCell } from '@treease/graph-viewer-runtime';
import {
  getCellEntry,
  getHighlightDecorations,
  registerCellBox,
  registerRowBox,
  resolveCellPath,
  unregisterCellBox,
  upsertCellEntry,
} from './graph-anchor-index';
import type { CellBoxEntry } from './model';

describe('graph-anchor-index', () => {
  it('indexes cell entries only by canonical path key', () => {
    const map = new Map<string, CellBoxEntry>();
    const scalarPath = [{ tag: 0, key: 'object', index: 0 }, { tag: 0, key: 'int', index: 0 }] as any[];
    const arrayPath = [{ tag: 0, key: 'array', index: 0 }] as any[];
    const scalarCell = { text: '42', path: scalarPath } as GraphCell;
    const arrayCell = { text: '[2]', path: arrayPath } as GraphCell;

    upsertCellEntry(map, scalarCell, (entry) => {
      entry.cell = scalarCell;
    });
    upsertCellEntry(map, arrayCell, (entry) => {
      entry.cell = arrayCell;
    });

    expect(getCellEntry(map, scalarPath)?.cell?.text).toBe('42');
    expect(getCellEntry(map, arrayPath)?.cell?.text).toBe('[2]');
    expect(map.get(buildPathKey(scalarPath))).not.toBe(map.get(buildPathKey(arrayPath)));
    expect(map.has('pos:0:0')).toBe(false);
  });

  it('does not resolve path from source position when graph path is missing', async () => {
    const resolveTreePathByPosition = vi.fn(async () => [{ tag: 0, key: 'fromPosition', index: 0 }]);
    const result = await resolveCellPath(
      { text: '42', value: '42', path: [], editable: true, boxArgs: {} as any, textArgs: {} as any, valueType: 'number' } as any,
      resolveTreePathByPosition,
      [],
    );

    expect(result).toEqual([]);
    expect(resolveTreePathByPosition).not.toHaveBeenCalled();
  });

  it('resolves the renderer-owned row and target decoration slots', () => {
    const map = new Map<string, CellBoxEntry>();
    const path = [{ tag: 0, key: 'img', index: 0 }] as any[];
    const cell = { text: 'logo', path } as GraphCell;
    const row = {} as any;
    const value = {} as any;
    const rowDecoration = {} as any;
    const valueDecoration = {} as any;

    registerRowBox(map, cell, row, undefined, undefined, undefined, rowDecoration);
    registerCellBox(map, cell, 'value', value, valueDecoration);

    expect(getHighlightDecorations(getCellEntry(map, path), 'value')).toEqual([
      rowDecoration,
      valueDecoration,
    ]);
    unregisterCellBox(map, cell, 'value', value);
    expect(getHighlightDecorations(getCellEntry(map, path), 'value')).toEqual([rowDecoration]);
  });
});
