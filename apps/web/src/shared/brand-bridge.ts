import type { BuilderConfig, DiffResult, PathSeg, TreeNode } from '@core-wasm/index';

type PlainPathSeg = {
  tag: number;
  key: string;
  index: number;
};

type PlainTreeNode = {
  kind: number;
  semType: number;
  tag: string;
  value: string;
  children: TreeNode[];
};


export function toWasmStringSlice<T>(value: string): T {
  return value as unknown as T;
}

export function toWasmPathSeg(pathSeg: PlainPathSeg): PathSeg {
  return {
    tag: pathSeg.tag as PathSeg['tag'],
    key: toWasmStringSlice<PathSeg['key']>(pathSeg.key),
    index: pathSeg.index,
  } as unknown as PathSeg;
}

export function toWasmTreeNode(node: PlainTreeNode): TreeNode {
  return node as unknown as TreeNode;
}

export function toWasmBuilderConfig<T extends Record<string, number>>(config: T): BuilderConfig {
  return config as unknown as BuilderConfig;
}

export function createEmptyDiffResult(): DiffResult {
  return { pairs: [] } as unknown as DiffResult;
}
