import { describe, expect, it } from 'vitest';
import {
  buildColumnNavigatorColumnItems,
  buildColumnNavigatorDirectItems,
  formatColumnNavigatorPath,
  normalizeWorkspaceGraphEdgeRows,
  rebaseColumnNavigatorPath,
  shouldOpenColumnNavigatorContent,
  shouldIgnoreSubgraphOpenCell,
} from './column-navigator-graph';
import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../store/tree-path';
import { PathSegTag } from '@core-wasm/index';

function makeHeaderTableNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'table',
    depth: 1,
    path: [],
    boxArgs: { x: 200, y: 100, width: 260, height: 160, cornerRadius: 4 },
    meta: {
      text: 'steps',
      value: '[2]',
      valueType: 'array',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'steps', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
    table: {
      columns: [],
      rows: [
        {
          boxArgs: { x: 200, y: 132, width: 260, height: 24, cornerRadius: 0 },
          cellBoxArgs: { x: 200, y: 132, width: 260, height: 24, cornerRadius: 0 },
          cells: [],
        },
        {
          boxArgs: { x: 200, y: 156, width: 260, height: 24, cornerRadius: 0 },
          cellBoxArgs: { x: 200, y: 156, width: 260, height: 24, cornerRadius: 0 },
          cells: [],
        },
      ],
      headerHeight: 32,
      totalHeight: 80,
      viewHeight: 80,
      rowHeight: 24,
    },
  };
}

function makeObjectNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'object',
    depth: 2,
    path: [],
    boxArgs: { x: 520, y: 140, width: 180, height: 60, cornerRadius: 4 },
    meta: {
      text: 'child',
      value: '{2}',
      valueType: 'object',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'child', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
  };
}

describe('normalizeWorkspaceGraphEdgeRows', () => {
  it('shifts workspace table edges by header offset when projection rows start at body index zero', () => {
    const nodes = [makeHeaderTableNode(10), makeObjectNode(11)];
    const edges: GraphEdge[] = [
      {
        fromRenderHandle: 10,
        fromRow: 0,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
      {
        fromRenderHandle: 10,
        fromRow: 1,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
    ];

    const normalized = normalizeWorkspaceGraphEdgeRows(nodes, edges);

    expect(normalized.map((edge) => edge.fromRow)).toEqual([1, 2]);
  });

  it('keeps already-offset table rows unchanged', () => {
    const nodes = [makeHeaderTableNode(10), makeObjectNode(11)];
    const edges: GraphEdge[] = [
      {
        fromRenderHandle: 10,
        fromRow: 1,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
      {
        fromRenderHandle: 10,
        fromRow: 2,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
    ];

    const normalized = normalizeWorkspaceGraphEdgeRows(nodes, edges);

    expect(normalized.map((edge) => edge.fromRow)).toEqual([1, 2]);
  });
});

function keySeg(key: string): PathSeg {
  return { tag: PathSegTag.KEY, key: key as any, index: 0 } as PathSeg;
}

function indexSeg(index: number): PathSeg {
  return { tag: PathSegTag.INDEX, key: '' as any, index } as PathSeg;
}

describe('formatColumnNavigatorPath', () => {
  it('keeps the full path so the pane header can use its available width', () => {
    expect(
      formatColumnNavigatorPath([
        keySeg('agent_steps'),
        indexSeg(0),
        keySeg('steps'),
        indexSeg(6),
        keySeg('basic_info'),
        keySeg('duration'),
      ]),
    ).toBe('agent_steps[0].steps[6].basic_info.duration');
  });
});

describe('buildColumnNavigatorDirectItems', () => {
  it('builds a column directly from snapshot children without graph cells', () => {
    expect(buildColumnNavigatorDirectItems([keySeg('messages')], [
      { kind: 'index', index: 0, preview: '{2}', valueType: 'object', semType: 7, isContainer: true },
      { kind: 'index', index: 1, preview: '{}', valueType: 'object', semType: 7, isContainer: false },
    ])).toEqual([
      {
        path: [keySeg('messages'), indexSeg(0)],
        pathKey: 'k:messages|i:0',
        label: '0',
        preview: '{2}',
        valueType: 'object',
        semType: 7,
        isContainer: true,
      },
      {
        path: [keySeg('messages'), indexSeg(1)],
        pathKey: 'k:messages|i:1',
        label: '1',
        preview: '{}',
        valueType: 'object',
        semType: 7,
        isContainer: false,
      },
    ]);
  });
});

describe('rebaseColumnNavigatorPath', () => {
  it('rebases relative workspace cell paths onto the workspace root path', () => {
    expect(
      rebaseColumnNavigatorPath([keySeg('preview'), keySeg('uris')], [indexSeg(1)]),
    ).toEqual([keySeg('preview'), keySeg('uris'), indexSeg(1)]);
  });

  it('preserves absolute paths that already include the workspace root', () => {
    const absolutePath = [keySeg('preview'), keySeg('uris'), indexSeg(1)];
    expect(
      rebaseColumnNavigatorPath([keySeg('preview'), keySeg('uris')], absolutePath),
    ).toBe(absolutePath);
  });

  it('maps empty relative paths back to the workspace root path', () => {
    expect(rebaseColumnNavigatorPath([keySeg('preview')], [])).toEqual([keySeg('preview')]);
  });
});

describe('shouldIgnoreSubgraphOpenCell', () => {
  it('ignores miss placeholder cells for subgraph expansion', () => {
    expect(
      shouldIgnoreSubgraphOpenCell({
        text: 'miss',
        value: 'miss',
        isMissing: true,
        valueType: 'object',
        isIndex: false,
        path: [],
        editable: false,
        boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
        textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'miss', textAlign: 'left', verticalAlign: 'middle', editable: false },
      }),
    ).toBe(true);
  });

  it('keeps real user values clickable even when their text is miss', () => {
    expect(
      shouldIgnoreSubgraphOpenCell({
        text: 'miss',
        value: 'miss',
        isMissing: false,
        valueType: 'string',
        isIndex: false,
        path: [keySeg('label')],
        editable: true,
        boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
        textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'miss', textAlign: 'left', verticalAlign: 'middle', editable: true },
      }),
    ).toBe(false);
  });
});

describe('shouldOpenColumnNavigatorContent', () => {
  it('keeps scalar values on the column-navigator path', () => {
    expect(shouldOpenColumnNavigatorContent({ valueType: 'number', displayText: '123' })).toBe(true);
    expect(shouldOpenColumnNavigatorContent({ valueType: 'string', displayText: '"Alice"' })).toBe(true);
  });

  it('keeps non-empty containers on the graph-pane path', () => {
    expect(shouldOpenColumnNavigatorContent({ valueType: 'object', displayText: '{2}' })).toBe(false);
    expect(shouldOpenColumnNavigatorContent({ valueType: 'array', displayText: '[3]' })).toBe(false);
  });

  it('treats empty containers as single-cell column detail editors', () => {
    expect(shouldOpenColumnNavigatorContent({ valueType: 'object', displayText: '{}' })).toBe(true);
    expect(shouldOpenColumnNavigatorContent({ valueType: 'array', displayText: '[]' })).toBe(true);
  });
});

describe('buildColumnNavigatorColumnItems', () => {
  it('shows child counts for container rows from the projected subtree', () => {
    const path = [keySeg('preview')];
    const graph = {
      path: [],
      pathKey: '$',
      nodes: [
        {
          renderHandle: 1,
          kind: 'object',
          depth: 0,
          path: [],
          boxArgs: {} as any,
          meta: null,
          rows: [
            {
              boxArgs: {} as any,
              cellBoxArgs: {} as any,
              cells: [
                {
                  text: 'preview',
                  value: 'preview',
                  formatText: '{7}',
                  valueType: 'object',
                  path,
                  editable: false,
                  boxArgs: {} as any,
                  textArgs: {} as any,
                },
              ],
            },
          ],
        },
      ],
      edges: [],
      minX: 0,
      minY: 0,
      width: 0,
      height: 0,
    } as any;

    expect(buildColumnNavigatorColumnItems(graph, [])).toEqual([
      expect.objectContaining({ path, preview: '{}', isContainer: false }),
    ]);
  });

  it('keeps nil visible and does not make empty containers navigable', () => {
    const graph = {
      path: [], pathKey: '$', edges: [], minX: 0, minY: 0, width: 0, height: 0,
      nodes: [{
        renderHandle: 1, kind: 'object', depth: 0, path: [], boxArgs: {} as any, meta: null,
        rows: [{ boxArgs: {} as any, cellBoxArgs: {} as any, cells: [
          { text: '', value: '', valueType: 'null', path: [keySeg('nil')], editable: false, boxArgs: {} as any, textArgs: {} as any },
          { text: 'arr0', value: 'arr0', valueType: 'array', path: [keySeg('arr0')], editable: false, boxArgs: {} as any, textArgs: {} as any },
          { text: 'obj0', value: 'obj0', valueType: 'object', path: [keySeg('obj0')], editable: false, boxArgs: {} as any, textArgs: {} as any },
        ] }],
      }],
    } as any;
    expect(buildColumnNavigatorColumnItems(graph, [])).toEqual([
      expect.objectContaining({ label: 'nil', preview: 'null', isContainer: false }),
      expect.objectContaining({ label: 'arr0', preview: '[]', isContainer: false }),
      expect.objectContaining({ label: 'obj0', preview: '{}', isContainer: false }),
    ]);
  });

  it('projects only direct children and rebases relative paths without duplicating key/value cells', () => {
    const basePath = [keySeg('user')];
    const namePath = [...basePath, keySeg('name')];
    const profilePath = [...basePath, keySeg('profile')];
    const graph = {
      path: basePath,
      pathKey: 'k:user',
      nodes: [
        {
          renderHandle: 1,
          kind: 'object',
          depth: 0,
          path: basePath,
          boxArgs: { x: 0, y: 0, width: 100, height: 100, cornerRadius: 4 },
          meta: {
            text: 'user',
            value: '{2}',
            valueType: 'object',
            path: basePath,
            editable: false,
            boxArgs: {} as any,
            textArgs: {} as any,
          },
          rows: [
            {
              boxArgs: {} as any,
              cellBoxArgs: {} as any,
              cells: [
                {
                  text: 'name',
                  value: 'name',
                  valueType: 'string',
                  path: namePath,
                  editable: true,
                  boxArgs: {} as any,
                  textArgs: {} as any,
                },
                {
                  text: '"Alice"',
                  value: '"Alice"',
                  valueType: 'string',
                  semType: 3,
                  path: namePath,
                  editable: true,
                  boxArgs: {} as any,
                  textArgs: {} as any,
                },
              ],
            },
            {
              boxArgs: {} as any,
              cellBoxArgs: {} as any,
              cells: [
                {
                  text: 'profile',
                  value: '{1}',
                  valueType: 'object',
                  semType: 0,
                  path: [keySeg('profile')],
                  editable: true,
                  boxArgs: {} as any,
                  textArgs: {} as any,
                },
              ],
            },
            {
              boxArgs: {} as any,
              cellBoxArgs: {} as any,
              cells: [
                {
                  text: 'deep',
                  value: 'ignored',
                  valueType: 'string',
                  path: [...profilePath, keySeg('deep')],
                  editable: true,
                  boxArgs: {} as any,
                  textArgs: {} as any,
                },
              ],
            },
          ],
        },
      ],
      edges: [],
      minX: 0,
      minY: 0,
      width: 100,
      height: 100,
    } as any;

    expect(buildColumnNavigatorColumnItems(graph, basePath)).toEqual([
      expect.objectContaining({
        path: namePath,
        pathKey: 'k:user|k:name',
        label: 'name',
        preview: '"Alice"',
        valueType: 'string',
        isContainer: false,
      }),
      expect.objectContaining({
        path: profilePath,
        pathKey: 'k:user|k:profile',
        label: 'profile',
        valueType: 'object',
        isContainer: true,
      }),
    ]);
  });

  it('drops missing placeholder rows so they cannot open another column', () => {
    const graph = {
      path: [],
      pathKey: '',
      nodes: [{
        renderHandle: 1,
        kind: 'object',
        depth: 0,
        path: [],
        boxArgs: {} as any,
        meta: { path: [], valueType: 'object', boxArgs: {}, textArgs: {} },
        rows: [{
          boxArgs: {},
          cellBoxArgs: {},
          cells: [{
            text: 'miss',
            value: 'miss',
            valueType: 'object',
            isMissing: true,
            path: [keySeg('miss')],
            boxArgs: {},
            textArgs: {},
          }],
        }],
      }],
      edges: [],
      minX: 0,
      minY: 0,
      width: 0,
      height: 0,
    } as any;

    expect(buildColumnNavigatorColumnItems(graph, [])).toEqual([]);
  });
});
