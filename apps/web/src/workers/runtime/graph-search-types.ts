import type { PathSeg } from '@core-wasm/index';
import type { GraphSearchTarget } from './protocol';

export type GraphSearchItem = {
  path: PathSeg[];
  pathKey: string;
  pathText: string;
  label: string;
  keyText: string;
  valueText: string;
  target: GraphSearchTarget;
  lazy?: any;
};

export type GraphSearchResult = {
  nodeId?: number;
  target: GraphSearchTarget;
  label: string;
  path: PathSeg[];
  pathText: string;
};

export type GraphSearchReadResult =
  | { status: 'ready'; data: GraphSearchResult[] }
  | { status: 'snapshotNotReady' };

export type SearchIndexEntry = {
  snapshotId: number;
  items: GraphSearchItem[];
  pathMap?: Map<string, number>;
};
