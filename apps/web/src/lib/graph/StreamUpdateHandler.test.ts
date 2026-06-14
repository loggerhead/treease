import { describe, it, expect } from 'vitest';
import {
  applyGraphDeltaToState,
  applyVersionedProjection,
   clearStreamState,
   createEmptyStreamState,
   replaceStreamState,
   streamStateToArrays,
 } from './StreamUpdateHandler';
import type { GraphNode, GraphEdge } from './graph-viewer-render';

function makeNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'scalar',
    depth: 0,
    boxArgs: { x: 0, y: 0, width: 100, height: 50, cornerRadius: 4 },
    path: [],
    meta: {} as any,
    rows: [],
  };
}

function makeObjectNode(renderHandle: number, rowTexts: string[]): GraphNode {
  return {
    ...makeNode(renderHandle),
    kind: 'object',
    rows: rowTexts.map((text, index) => ({
      boxArgs: { x: 0, y: index * 10, width: 100, height: 10, cornerRadius: 0 },
      cellBoxArgs: { x: 0, y: 0, width: 100, height: 10, cornerRadius: 0 },
      cells: [
        {
          text: `k${index}`,
          value: `k${index}`,
          valueType: 'string',
          isIndex: false,
          path: [],
          editable: false,
          boxArgs: { x: 0, y: 0, width: 50, height: 10, cornerRadius: 0 },
          textArgs: { x: 0, y: 0, width: 50, height: 10, text: `k${index}`, textAlign: 'left', verticalAlign: 'middle', editable: false },
        },
        {
          text,
          value: text,
          valueType: 'string',
          isIndex: false,
          path: [],
          editable: false,
          boxArgs: { x: 50, y: 0, width: 50, height: 10, cornerRadius: 0 },
          textArgs: { x: 50, y: 0, width: 50, height: 10, text, textAlign: 'left', verticalAlign: 'middle', editable: false },
        },
      ],
    })) as any,
  };
}

function makeTableNode(renderHandle: number, columnTexts: string[], rowTexts: string[]): GraphNode {
  return {
    ...makeNode(renderHandle),
    kind: 'table',
    table: {
    columns: columnTexts.map((text) => ({
        text,
        value: text,
        valueType: 'string',
        isIndex: false,
        path: [],
        editable: false,
        boxArgs: { x: 0, y: 0, width: 50, height: 10, cornerRadius: 0 },
        textArgs: { x: 0, y: 0, width: 50, height: 10, text, textAlign: 'left', verticalAlign: 'middle', editable: false },
      })),
      rows: rowTexts.map((text, index) => ({
        boxArgs: { x: 0, y: index * 10, width: 100, height: 10, cornerRadius: 0 },
        cellBoxArgs: { x: 0, y: 0, width: 100, height: 10, cornerRadius: 0 },
        cells: [
          {
            text,
            value: text,
            valueType: 'string',
            isIndex: false,
            path: [],
            editable: false,
            boxArgs: { x: 0, y: 0, width: 100, height: 10, cornerRadius: 0 },
            textArgs: { x: 0, y: 0, width: 100, height: 10, text, textAlign: 'left', verticalAlign: 'middle', editable: false },
          },
        ],
      })),
      headerHeight: 11,
      totalHeight: 21,
      viewHeight: 31,
      rowHeight: 41,
    } as any,
  };
}

function makeEdge(from: number, to: number): GraphEdge {
  return {
    fromRenderHandle: from,
    fromRow: 0,
    toRenderHandle: to,
    toRow: 0,
    bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
  };
}

function makeKeyedEdge(from: number, to: number): GraphEdge {
  return {
    ...makeEdge(from, to),
    from: { kind: 'object', path: [], pathKey: `k:${from}`, stableId: `stable-${from}` },
    to: { kind: 'scalar', path: [], pathKey: `k:${to}`, stableId: `stable-${to}` },
  };
}

describe('StreamUpdateHandler', () => {
  describe('createEmptyStreamState', () => {
    it('creates state with empty maps', () => {
      const state = createEmptyStreamState();
      expect(state.nodes.size).toBe(0);
      expect(state.edges.size).toBe(0);
    });
  });

  describe('streamStateToArrays', () => {
    it('converts maps to arrays', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.nodes.set(2, makeNode(2));
      const result = streamStateToArrays(state);
      expect(result.nodes).toHaveLength(2);
      expect(result.edges).toHaveLength(0);
    });
  });

  describe('clearStreamState', () => {
    it('clears nodes and edges', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.edges.set('1:0->2:0', makeEdge(1, 2));
      clearStreamState(state);
      expect(state.nodes.size).toBe(0);
      expect(state.edges.size).toBe(0);
    });
  });

  describe('replaceStreamState', () => {
    it('replaces existing state contents', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.edges.set('1:0->2:0', makeEdge(1, 2));
      replaceStreamState(state, {
        nodes: [makeNode(3)],
        edges: [makeEdge(3, 4)],
      });
      expect(Array.from(state.nodes.keys())).toEqual([3]);
      expect(Array.from(state.edges.values())).toEqual([makeEdge(3, 4)]);
    });
  });

  describe('applyGraphDeltaToState', () => {
    it('ignores null delta', () => {
      const state = createEmptyStreamState();
      applyGraphDeltaToState(null, state);
      expect(state.nodes.size).toBe(0);
    });

    it('adds normalized nodes', () => {
      const state = createEmptyStreamState();
      const delta = {
        normalized: true,
        nodesAdded: [makeNode(1), makeNode(2)],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.size).toBe(2);
    });

    it('stores normalized nodes by render handle when present', () => {
      const state = createEmptyStreamState();
      applyGraphDeltaToState(
        {
          normalized: true,
          nodesAdded: [{ ...makeNode(1), renderHandle: 101 }],
          nodesUpdated: [],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
        },
        state,
      );
      expect(state.nodes.has(101)).toBe(true);
      expect(state.nodes.has(1)).toBe(false);
    });

    it('updates existing nodes', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const updatedNode = { ...makeNode(1), depth: 5 };
      const delta = {
        normalized: true,
        nodesAdded: [],
        nodesUpdated: [updatedNode],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.get(1)?.depth).toBe(5);
    });

    it('removes nodes by id', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.nodes.set(2, makeNode(2));
      const delta = {
        normalized: true,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [1],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.size).toBe(1);
      expect(state.nodes.has(2)).toBe(true);
    });

    it('accepts typed array node removals from decoded graph deltas', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.nodes.set(2, makeNode(2));

      applyGraphDeltaToState(
        {
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [],
          nodesRemoved: new Int32Array([1]),
          edgesAdded: [],
          edgesRemoved: [],
        },
        state,
      );

      expect(state.nodes.size).toBe(1);
      expect(state.nodes.has(2)).toBe(true);
    });

    it('adds and removes edges', () => {
      const state = createEmptyStreamState();
      const edge = makeKeyedEdge(1, 2);
      const addDelta = {
        normalized: true,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [edge],
        edgesRemoved: [],
      };
      applyGraphDeltaToState(addDelta, state);
      expect(state.edges.size).toBe(1);
      expect(Array.from(state.edges.keys())).toEqual(['stable-1:0:stable-2:0']);

      const removeDelta = {
        normalized: true,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [edge],
      };
      applyGraphDeltaToState(removeDelta, state);
      expect(state.edges.size).toBe(0);
    });

    it('clears state when delta.clear === 1', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      state.nodes.set(2, makeNode(2));
      const delta = {
        normalized: true,
        clear: 1,
        nodesAdded: [makeNode(3)],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.size).toBe(1);
      expect(state.nodes.has(3)).toBe(true);
    });

    it('applies normalized node updates for object rows', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeObjectNode(1, ['a', 'b']));
      applyGraphDeltaToState(
        {
          normalized: true,
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [makeObjectNode(1, ['a', 'c'])],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
        },
        state,
      );
      const node = state.nodes.get(1);
      expect(node?.rows).toHaveLength(2);
      expect(node?.rows?.[1]?.cells?.[1]?.text).toBe('c');
    });

    it('applies normalized node updates for table changes', () => {
      const state = createEmptyStreamState();
      state.nodes.set(2, makeTableNode(2, ['old'], ['r1']));
      const updated = makeTableNode(2, ['new'], ['r1', 'r2']);
      updated.boxArgs = { x: 10, y: 20, width: 110, height: 60, cornerRadius: 6 };
      (updated.table as any).headerHeight = 12;
      (updated.table as any).totalHeight = 32;
      (updated.table as any).viewHeight = 22;
      (updated.table as any).rowHeight = 10;
      applyGraphDeltaToState(
        {
          normalized: true,
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [updated],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
        },
        state,
      );
      const node = state.nodes.get(2);
      expect(node?.boxArgs.x).toBe(10);
      expect(node?.table?.columns?.[0]?.text).toBe('new');
      expect(node?.table?.rows).toHaveLength(2);
      expect(node?.table?.totalHeight).toBe(32);
    });

    it('applies table cell patches without node updates', () => {
      const state = createEmptyStreamState();
      state.nodes.set(2, makeTableNode(2, ['name'], ['amy']));
      applyGraphDeltaToState(
        {
          normalized: true,
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
          tableCellPatches: [
            {
              tableRenderHandle: 2,
              rowIndex: 0,
              columnIndex: 0,
              cell: {
                text: 'ada',
                value: 'ada',
                valueType: 'string',
                isIndex: false,
                path: [],
                editable: true,
                boxArgs: { x: 0, y: 0, width: 100, height: 10, cornerRadius: 0 },
                textArgs: { x: 0, y: 0, width: 100, height: 10, text: 'ada', textAlign: 'left', verticalAlign: 'middle', editable: true },
              },
            },
          ],
        },
        state,
      );
      const node = state.nodes.get(2);
      expect(node?.table?.rows?.[0]?.cells?.[0]?.text).toBe('ada');
      expect(state.nodes.size).toBe(1);
      expect(state.edges.size).toBe(0);
    });

    it('ignores non-graphDelta objects without normalized flag', () => {
      const state = createEmptyStreamState();
      applyGraphDeltaToState({ random: 'data' }, state);
      expect(state.nodes.size).toBe(0);
    });
   });
  describe('versioned patch store contract', () => {
    // Simulates the version validation layer that wraps StreamState.
    type VersionedState = {
      version: number;
      state: ReturnType<typeof createEmptyStreamState>;
    };

    function createVersionedState(): VersionedState {
      return { version: 0, state: createEmptyStreamState() };
    }

    type VersionedProjection = {
      clear: boolean;
      delta: any;
      baseGraphVersion: number;
      graphVersion: number;
    };

    function applyVersioned(
      vs: VersionedState,
      proj: VersionedProjection,
    ): void {
      if (proj.baseGraphVersion !== vs.version) {
        throw new Error(
          `graph version mismatch: expected ${proj.baseGraphVersion}, got ${vs.version}`,
        );
      }
      if (proj.clear) {
        clearStreamState(vs.state);
      }
      applyGraphDeltaToState(proj.delta, vs.state);
      vs.version = proj.graphVersion;
    }

    it('applies first clear patch with version 0→1', () => {
      const vs = createVersionedState();
      const delta = {
        normalized: true,
        clear: 1,
        nodesAdded: [{ ...makeNode(1), renderHandle: 1 }],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyVersioned(vs, {
        clear: true,
        delta,
        baseGraphVersion: 0,
        graphVersion: 1,
      });
      expect(vs.version).toBe(1);
      expect(vs.state.nodes.size).toBe(1);
    });

    it('applies chained non-clear patches', () => {
      const vs = createVersionedState();
      vs.version = 1;
      vs.state.nodes.set(1, makeNode(1));

      const delta = {
        normalized: true,
        clear: 0,
        nodesAdded: [{ ...makeNode(2), renderHandle: 2 }],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      applyVersioned(vs, {
        clear: false,
        delta,
        baseGraphVersion: 1,
        graphVersion: 2,
      });
      expect(vs.version).toBe(2);
      expect(vs.state.nodes.size).toBe(2);
    });

    it('rejects version mismatch', () => {
      const vs = createVersionedState();
      vs.version = 1;
      const delta = {
        normalized: true,
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      expect(() => {
        applyVersioned(vs, {
          clear: false,
          delta,
          baseGraphVersion: 0, // stale: state is at 1
          graphVersion: 1,
        });
      }).toThrow(/graph version mismatch/);
    });

    it('rejects skipped patch (version gap)', () => {
      const vs = createVersionedState();
      vs.version = 0;
      const delta = {
        normalized: true,
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      };
      expect(() => {
        applyVersioned(vs, {
          clear: false,
          delta,
          baseGraphVersion: 2, // gap: should be 0
          graphVersion: 3,
        });
      }).toThrow(/graph version mismatch/);
    });

    it('clear resets state regardless of current version', () => {
      const vs = createVersionedState();
      vs.version = 5;
      vs.state.nodes.set(99, makeNode(99));

      applyVersioned(vs, {
        clear: true,
        delta: {
          normalized: true,
          clear: 1,
          nodesAdded: [{ ...makeNode(1), renderHandle: 1 }],
          nodesUpdated: [],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
        },
        baseGraphVersion: 5,
        graphVersion: 6,
      });
      expect(vs.version).toBe(6);
     });
   });

  describe('applyVersionedProjection', () => {
    it('applies patch when version matches', () => {
      const state = createEmptyStreamState();
      expect(state.version).toBe(0);

      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [{ ...makeNode(1), renderHandle: 1 }],
        nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
      };

      applyVersionedProjection(state, delta, { baseGraphVersion: 0, graphVersion: 1 });
      expect(state.version).toBe(1);
      expect(state.nodes.has(1)).toBe(true);
    });

    it('accepts incremental chunk with baseGraphVersion > state (catch-up)', () => {
      const state = createEmptyStreamState();
      state.version = 1;
      state.nodes.set(1, makeNode(1));

      const newNode = makeNode(2);
      const delta = { normalized: true, clear: 0, nodesAdded: [newNode], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] };

      // baseGraphVersion=2 > state.version=1 — advisory, apply anyway
      applyVersionedProjection(state, delta, { baseGraphVersion: 2, graphVersion: 3 });
      expect(state.version).toBe(3);
      expect(state.nodes.has(2)).toBe(true);
    });

    it('skips stale chunks where baseGraphVersion < state.version', () => {
      const state = createEmptyStreamState();
      state.version = 3;
      state.nodes.set(1, makeNode(1));

      const delta = { normalized: true, clear: 0, nodesAdded: [], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] };

      // baseGraphVersion=1 < state.version=3 → stale, skip without error or state change
      applyVersionedProjection(state, delta, { baseGraphVersion: 1, graphVersion: 2 });
      expect(state.version).toBe(3); // unchanged
      expect(state.nodes.has(1)).toBe(true); // untouched
    });

    it('accepts reset patch with baseGraphVersion=0 regardless of current version', () => {
      const state = createEmptyStreamState();
      state.version = 5; // stale version from previous job

      const delta = { normalized: true, clear: 1, nodesAdded: [], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] };

      // baseGraphVersion=0 is a reset — should not throw
      applyVersionedProjection(state, delta, { baseGraphVersion: 0, graphVersion: 1 });
      expect(state.version).toBe(1);
    });
  });

  describe('table patch: tableCreated', () => {
    it('creates a table node from TableCreated patch', () => {
      const state = createEmptyStreamState();
      const delta = {
        normalized: true,
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tablePatches: [{
          kind: 'tableCreated',
          tableHandle: 100,
          columns: [
            { text: 'name', width: 80, height: 20 },
            { text: 'value', width: 120, height: 20 },
          ],
          headerHeight: 20,
        }],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.has(100)).toBe(true);
      const node = state.nodes.get(100);
      expect(node?.kind).toBe('table');
      expect(node?.table?.columns).toHaveLength(2);
      expect(node?.table?.columns?.[0]?.text).toBe('name');
    });

    it('preserves node layout from nodesAdded when TableCreated arrives in the same streaming delta', () => {
      const state = createEmptyStreamState();
      const tableNode = makeTableNode(101, ['name'], []);
      tableNode.boxArgs = { x: 320, y: 180, width: 420, height: 120, cornerRadius: 6 };
      tableNode.path = [{ tag: 1, index: 0, key: '' } as any];
      tableNode.meta = { text: '$[0]', valueType: 'object', path: tableNode.path } as any;
      tableNode.table!.headerHeight = 0;
      tableNode.table!.rowHeight = 22;
      tableNode.table!.viewHeight = 88;
      tableNode.table!.totalHeight = 220;

      const delta = {
        normalized: true,
        clear: 0,
        nodesAdded: [tableNode],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tablePatches: [{
          kind: 'tableCreated',
          tableHandle: 101,
          columns: [{ text: 'name', width: 80, height: 20 }],
        }],
      };

      applyGraphDeltaToState(delta, state);

      const node = state.nodes.get(101);
      expect(node?.boxArgs).toEqual(tableNode.boxArgs);
      expect(node?.path).toEqual(tableNode.path);
      expect(node?.meta).toEqual(tableNode.meta);
      expect(node?.table?.headerHeight).toBe(0);
      expect(node?.table?.rowHeight).toBe(22);
      expect(node?.table?.viewHeight).toBe(88);
      expect(node?.table?.totalHeight).toBe(220);
      expect(node?.table?.columns?.[0]?.text).toBe('name');
    });

    it('handles camelCase table_handle field from protocol', () => {
      const state = createEmptyStreamState();
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        tablePatches: [{
          kind: 'tableCreated',
          table_handle: 200,
          columns: [{ text: 'col', width: 60, height: 20 }],
          headerHeight: 20,
        }],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.has(200)).toBe(true);
    });
  });

  describe('table patch: cellsUpdated', () => {
    it('updates specific cells via CellsUpdated patch', () => {
      const state = createEmptyStreamState();
      state.nodes.set(2, makeTableNode(2, ['name'], ['amy']));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        tablePatches: [{
          kind: 'cellsUpdated',
          tableHandle: 2,
          cells: [
            { rowIndex: 0, columnIndex: 0, cell: { text: 'ada', value: 'ada', valueType: 'string', isIndex: false } },
          ],
        }],
      };
      applyGraphDeltaToState(delta, state);
      const node = state.nodes.get(2);
      expect(node?.table?.rows?.[0]?.cells?.[0]?.text).toBe('ada');
    });
  });

  describe('table patch: rowsAppended', () => {
    it('appends rows without copying existing row storage', () => {
      const state = createEmptyStreamState();
      const node = makeTableNode(5, ['name'], ['amy', 'bea']);
      node.table!.headerHeight = 10;
      node.table!.rowHeight = 20;
      node.table!.totalHeight = 50;
      node.table!.viewHeight = 50;
      state.nodes.set(5, node);
      const rowsBefore = node.table!.rows;

      applyGraphDeltaToState({
        normalized: true,
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tablePatches: [{
          kind: 'rowsAppended',
          tableHandle: 5,
          startIndex: 2,
          rows: [{
            boxArgs: { x: 0, y: 40, width: 100, height: 20, cornerRadius: 0 },
            cellBoxArgs: { x: 0, y: 0, width: 100, height: 20, cornerRadius: 0 },
            cells: [{ text: 'cy', value: 'cy', valueType: 'string', path: [], editable: false }],
          }],
        }],
      }, state);

      const next = state.nodes.get(5);
      expect(next?.table?.rows).toBe(rowsBefore);
      expect(next?.table?.rows).toHaveLength(3);
      expect(next?.table?.rows[2]?.cells[0]?.text).toBe('cy');
      expect(next?.table?.totalHeight).toBe(70);
    });
  });


  describe('table patch: columnsAdded', () => {
    it('appends columns to an existing table', () => {
      const state = createEmptyStreamState();
      state.nodes.set(3, makeTableNode(3, ['a'], ['r1']));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        tablePatches: [{
          kind: 'columnsAdded',
          tableHandle: 3,
          columns: [{ text: 'b', width: 100, height: 20 }],
        }],
      };
      applyGraphDeltaToState(delta, state);
      const node = state.nodes.get(3);
      expect(node?.table?.columns).toHaveLength(2);
      expect(node?.table?.columns?.[1]?.text).toBe('b');
    });

    it('normalizes raw GraphCellData when appending columns', () => {
      const state = createEmptyStreamState();
      state.nodes.set(4, makeTableNode(4, ['a'], ['r1']));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        tablePatches: [{
          kind: 'columnsAdded',
          tableHandle: 4,
          columns: [{
            text: 'b',
            semType: 0,
            boxArgs: { x: 50, y: 0, width: 100, height: 20, cornerRadius: 0 },
            textArgs: { x: 50, y: 0, width: 100, height: 20, text: 'b', textAlign: 2, textVerticalAlign: 2, editable: 0 },
          }],
        }],
      };
      applyGraphDeltaToState(delta, state);

      const column = state.nodes.get(4)?.table?.columns?.[1];
      expect(column).toEqual(expect.objectContaining({
        text: 'b',
        valueType: 'object',
        boxArgs: { x: 50, y: 0, width: 100, height: 20, cornerRadius: 0 },
        textArgs: expect.objectContaining({ textAlign: 'right', verticalAlign: 'bottom' }),
      }));
    });
  });

  describe('layout patch: nodeBoundsUpdated', () => {
    it('updates node boxArgs from NodeBoundsUpdated patch', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        layoutPatches: [{
          kind: 'nodeBoundsUpdated',
          renderHandle: 1,
          boxArgs: { x: 50, y: 60, width: 200, height: 40, cornerRadius: 8 },
        }],
      };
      applyGraphDeltaToState(delta, state);
      const node = state.nodes.get(1);
      expect(node?.boxArgs.x).toBe(50);
      expect(node?.boxArgs.y).toBe(60);
      expect(node?.boxArgs.width).toBe(200);
      expect(node?.boxArgs.height).toBe(40);
    });

    it('handles snake_case box_args from protocol', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        layoutPatches: [{
          kind: 'nodeBoundsUpdated',
          render_handle: 1,
          box_args: { x: 10, y: 20, width: 300, height: 50, cornerRadius: 4 },
        }],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.get(1)?.boxArgs.width).toBe(300);
    });
  });

  describe('layout patch: groupLayoutUpdated', () => {
    it('updates container dimensions', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        layoutPatches: [{
          kind: 'groupLayoutUpdated',
          groupHandle: 1,
          width: 500,
          height: 300,
        }],
      };
      applyGraphDeltaToState(delta, state);
      const node = state.nodes.get(1);
      expect(node?.boxArgs.width).toBe(500);
      expect(node?.boxArgs.height).toBe(300);
    });
  });

  describe('layout patch: viewportLayoutHint', () => {
    it('does not modify state nodes', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        layoutPatches: [{
          kind: 'viewportLayoutHint',
          totalHeight: 1000,
          appendedHeight: 100,
        }],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.get(1)?.boxArgs.width).toBe(100); // unchanged
    });
  });

  describe('table patch: unknown kind is a no-op', () => {
    it('ignores patches with unrecognized kind', () => {
      const state = createEmptyStreamState();
      state.nodes.set(1, makeNode(1));
      const delta = {
        normalized: true, clear: 0,
        nodesAdded: [], nodesUpdated: [], nodesRemoved: [],
        edgesAdded: [], edgesRemoved: [],
        tablePatches: [{ kind: 'unknownThing', foo: 'bar' }],
      };
      applyGraphDeltaToState(delta, state);
      expect(state.nodes.size).toBe(1);
    });
  });

});
