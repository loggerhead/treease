import type { GraphViewerConfig } from "../../settings/ui-settings";
import { type GraphNode } from '@treease/graph-viewer-runtime';
import { createGraphSceneRuntime } from "./graph-scene-runtime";
import type { GraphSceneLayers, GraphViewerClickTargetStore } from "./model";

type GraphSceneControllerDeps = {
  getContainer: () => HTMLElement | null;
  getLeafer: () => any;
  getRenderRoot: () => any | null;
  getBoxCtor: () => any;
  getTextCtor: () => any;
  getPenCtor: () => any;
  getRenderConfig: () => GraphViewerConfig;
  getLanguageId: () => string;
  /** @deprecated Scene rendering reads GraphCell.semType. */
  getValueTypeToSemType?: () => Record<string, string>;
  isReadonly?: () => boolean;
  getLastAutoOffset: () => { x: number; y: number } | null;
  setLastAutoOffset: (value: { x: number; y: number } | null) => void;
  getLayers: () => GraphSceneLayers;
  setLayers: (layers: Partial<GraphSceneLayers>) => void;
  buildPathSegFromCell: (cell: any, rowIndex: number) => any;
  clearSearchHighlight: () => void;
  beginMainGraphRedraw: (nodes: GraphNode[]) => Map<number, any>;
  setFullGraph: (nodes: GraphNode[]) => void;
  getNodeDataMap: () => Map<number, GraphNode>;
  getNodeBoxMap: () => Map<number, any>;
  getClickTargetProbes: () => any[];
  getClickTargetProbeStore: () => GraphViewerClickTargetStore;
  upsertCellEntry: (
    map: Map<string, any>,
    cell: any,
    updater: (entry: any) => void,
  ) => void;
  updateCellEntry: (
    map: Map<string, any>,
    cell: any,
    updater: (entry: any) => void,
  ) => void;
  upsertClickTargetProbe: (
    store: GraphViewerClickTargetStore,
    targetIds: WeakMap<object, string>,
    box: any,
    cell: any,
    kind: any,
  ) => string;
  bindPointerClick: (
    target: any,
    handler: (event: unknown) => void | Promise<void>,
  ) => void;
  toGraphClickTarget: (kind: any) => "key" | "value" | "node";
  getSuppressGraphPointerUntil: () => number;
  resolveTreePathByPosition: (row: number, column: number) => Promise<any[]>;
  resolveInteractiveCellPath: (
    cell: any,
    fallbackPath: any[],
  ) => Promise<any[]>;
  emitReveal: (
    path: any[],
    target: "key" | "value" | "node",
    source: "click" | "test-hook",
  ) => void;
  registerCellBox: (cell: any, kind: any, box: any) => void;
  unregisterCellBox: (cell: any, kind: any, box: any) => void;
  registerRowBox: (
    cell: any,
    rowBox: any,
    scrollOwner?: any,
    bodyHeight?: number,
    contentHeight?: number,
  ) => void;
  unregisterRowBox: (cell: any, rowBox: any) => void;
  registerClickTarget: (
    target: any,
    cell: any,
    kind: any,
    nodeKind?: GraphNode["kind"],
  ) => string;
  bindVerticalScrollGesture?: (
    target: any,
    handler: (gesture: {
      event: unknown;
      deltaY: number;
      moveType?: string;
      stop: () => void;
      stopNow: () => void;
    }) => void,
  ) => (() => void) | void;
  bindPointerDown?: (
    target: any,
    handler: (event: unknown) => void | Promise<void>,
  ) => (() => void) | void;
  getPointFromEvent?: (
    hostApp: any,
    target: any,
    event: unknown,
    space: "client" | "box" | "local" | "world",
  ) => { x: number; y: number } | null;
  refreshActiveHighlight?: () => void;
  updateLeafer: () => void;
  handleError: (
    error: unknown,
    context: {
      component: string;
      operation: string;
      metadata?: Record<string, unknown>;
    },
  ) => void;
};

export function createGraphSceneController(deps: GraphSceneControllerDeps) {
  const runtime = createGraphSceneRuntime({
    getContainer: deps.getContainer,
    getLeafer: deps.getLeafer,
    getRenderRoot: deps.getRenderRoot,
    getBoxCtor: deps.getBoxCtor,
    getTextCtor: deps.getTextCtor,
    getPenCtor: deps.getPenCtor,
    getRenderConfig: deps.getRenderConfig,
    getLanguageId: deps.getLanguageId,
    isReadonly: deps.isReadonly,
    getLastAutoOffset: deps.getLastAutoOffset,
    setLastAutoOffset: deps.setLastAutoOffset,
    getLayers: deps.getLayers,
    setLayers: deps.setLayers,
    buildPathSegFromCell: deps.buildPathSegFromCell,
    clearSearchHighlight: deps.clearSearchHighlight,
    beginMainGraphRedraw: deps.beginMainGraphRedraw,
    setFullGraph: deps.setFullGraph,
    getNodeDataMap: deps.getNodeDataMap,
    getNodeBoxMap: deps.getNodeBoxMap,
    getClickTargetProbes: deps.getClickTargetProbes,
    getClickTargetProbeStore: deps.getClickTargetProbeStore,
    registerCellBox: deps.registerCellBox,
    unregisterCellBox: deps.unregisterCellBox,
    registerRowBox: deps.registerRowBox,
    unregisterRowBox: deps.unregisterRowBox,
    registerClickTarget: deps.registerClickTarget,
    bindVerticalScrollGesture: deps.bindVerticalScrollGesture,
    bindPointerDown: deps.bindPointerDown,
    getPointFromEvent: deps.getPointFromEvent,
    refreshActiveHighlight: deps.refreshActiveHighlight,
    updateLeafer: deps.updateLeafer,
  });

  return {
    clear: runtime.clear,
    inferGraphPaths: runtime.inferGraphPaths,
    ensureLayers: runtime.ensureLayers,
    drawEdges: runtime.drawEdges,
    replaceAll: runtime.replaceAll,
    applyGraphDelta: runtime.applyGraphDelta,
    flushPendingRenderWork: runtime.flushPendingRenderWork,
    getInteractionState: runtime.getInteractionState,
    cancelActiveRenderWork: runtime.cancelActiveRenderWork,
    getLastGraphData: runtime.getLastGraphData,
    setLastViewData: runtime.setLastViewData,
    updateRenderableProjection: runtime.updateRenderableProjection,
    submitMaterializationIntent: runtime.submitMaterializationIntent,
    materializeTarget: runtime.materializeTarget,
    waitForProjection: runtime.waitForProjection,
    getCommittedProjection: runtime.getCommittedProjection,
    isTargetMaterialized: runtime.isTargetMaterialized,
    dispose: runtime.dispose,
    scrollTableToRow: runtime.scrollTableToRow,
  };
}
