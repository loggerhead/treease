import type { GraphHighlightTarget } from '../../../store/graph-selection-store';
import type { PathSeg } from '../../../store/tree-path';

export type GraphRuntimeRect = {
  left: number;
  top: number;
  width: number;
  height: number;
};

export type GraphRuntimePoint = {
  x: number;
  y: number;
};

export type GraphRuntimeProbeTarget = {
  scope: 'root' | 'workspace';
  id: string;
  target?: 'key' | 'value' | 'node';
  nodeType: string;
  coord: GraphRuntimePoint | null;
  rect: GraphRuntimeRect | null;
  worldRect: GraphRuntimeRect | null;
  cell: {
    text: string;
    valueType: string;
    isTableCell: boolean;
    isHeader: boolean;
    path: PathSeg[];
  } | null;
};

export type GraphRuntimeHighlightState = {
  path: PathSeg[];
  target?: GraphHighlightTarget;
  rect: GraphRuntimeRect | null;
  probe: (GraphRuntimePoint & { source: 'matched-probe' | 'highlight-box' }) | null;
  world:
    | {
        highlight: GraphRuntimePoint;
        viewportCenter: GraphRuntimePoint;
      }
    | null;
};

export type GraphRuntimeRevealState = {
  path: PathSeg[];
  target?: GraphHighlightTarget;
};

export type GraphRuntimeRowScrollState = {
  path: PathSeg[];
  scrollY: number;
  bodyHeight?: number;
  contentHeight?: number;
};

export type GraphRuntimePanelState = {
  path: PathSeg[];
  visible: boolean;
  rect: GraphRuntimeRect | null;
};

export type GraphRuntimeHitResult = {
  scope: 'root';
  point: GraphRuntimePoint;
  hit: GraphRuntimeProbeTarget | null;
};
