import { describe, expect, it } from 'vitest';
import { resolveWorkspacePath } from './workspace-path';
import type { GraphData } from '../shared/types';

const root = [] as GraphData['nodes'][number]['path'];
const row = [{ tag: 1, index: 0 }] as GraphData['nodes'][number]['path'];
const scalarCell = [{ tag: 1, index: 0 }, { tag: 0, key: 'name' }] as GraphData['nodes'][number]['path'];

describe('resolveWorkspacePath', () => {
  it('opens the nearest structural owner when a table scalar cell is clicked', () => {
    const graph = {
      nodes: [
        { kind: 'table', path: root },
        { kind: 'object', path: row },
      ],
      edges: [],
      coreGraphAvailable: true,
    } as unknown as GraphData;

    expect(resolveWorkspacePath(graph, scalarCell)).toEqual(row);
  });

  it('returns no workspace for a scalar-only graph', () => {
    const graph = { nodes: [{ kind: 'scalar', path: root }], edges: [], coreGraphAvailable: true } as unknown as GraphData;

    expect(resolveWorkspacePath(graph, scalarCell)).toBeNull();
  });
});
