import type { GraphData } from '../shared/types';

type Path = GraphData['nodes'][number]['path'];

function isPathPrefix(prefix: Path, value: Path): boolean {
  return prefix.length <= value.length && prefix.every((segment, index) => {
    const candidate = value[index];
    return candidate && segment.tag === candidate.tag && (segment.tag === 0 ? segment.key === candidate.key : segment.index === candidate.index);
  });
}

/** Maps a rendered cell to the closest graph node that can own a subgraph. */
export function resolveWorkspacePath(graph: GraphData | null, path: Path): Path | null {
  if (!graph) return null;
  const owner = graph.nodes
    .filter((node) => (node.kind === 'object' || node.kind === 'table') && isPathPrefix(node.path, path))
    .sort((left, right) => right.path.length - left.path.length)[0];
  return owner?.path ?? null;
}
