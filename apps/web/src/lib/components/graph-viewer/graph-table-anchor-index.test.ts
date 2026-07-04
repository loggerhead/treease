import { describe, expect, it } from 'vitest';
import { PathSegTag } from '@core-wasm/index';
import {
  indexTableCellAnchorsForNode,
  rebuildTableCellAnchorIndex,
  removeTableCellAnchorsForNode,
} from './graph-table-anchor-index';
import { buildPathKey } from '../../graph/graph-viewer-path';

function keySeg(key: string) {
  return { tag: PathSegTag.KEY, key: key as any, index: 0 } as any;
}

function indexSeg(index: number) {
  return { tag: PathSegTag.INDEX, key: '' as any, index } as any;
}

function createTableNode(renderHandle = 10): any {
  const rowPath = [keySeg('rows'), indexSeg(7)];
  return {
    renderHandle,
    kind: 'table',
    path: [keySeg('rows')],
    table: {
      rows: [
        {
          cells: [
            { path: rowPath },
            { path: [...rowPath, keySeg('name')] },
          ],
        },
      ],
    },
  };
}

describe('graph-table-anchor-index', () => {
  it('indexes table cell canonical paths by node and row', () => {
    const node = createTableNode();
    const index = rebuildTableCellAnchorIndex([node]);

    expect(index.get(buildPathKey([keySeg('rows'), indexSeg(7), keySeg('name')]))).toEqual({
      nodeId: 10,
      rowIndex: 0,
      cellIndex: 1,
      target: 'value',
    });
  });

  it('removes entries for a replaced table node', () => {
    const index = new Map();
    indexTableCellAnchorsForNode(index, createTableNode(10));
    indexTableCellAnchorsForNode(index, createTableNode(11));

    removeTableCellAnchorsForNode(index, 10);

    expect([...index.values()].map((anchor) => anchor.nodeId)).toEqual([11, 11]);
  });
});
