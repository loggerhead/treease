// 职责：test bridge 类型定义：Editor/Graph/Preview/Settings bridge 接口
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { Settings, SettingsDocument } from '../settings/ui-settings';
import type { SettingsStatus } from '../settings/settings-store';
import type { EditorState, GraphHighlightTarget } from '../store/editor-store';
import type { EditorWorkspaceState } from '../store/editor-workspace';
import type { GraphEdge, GraphNode } from '../../shared/worker-protocol/protocol';

export type TreeaseBridgePathSeg = {
  key?: string;
  index?: number;
  tag?: number;
};

export type TreeaseRuntimePathSeg = {
  key?: unknown;
  index?: number;
  tag?: number;
};

export type TreeaseMonacoHook = {
  getValue: () => string;
  setValue: (value: string) => void;
  setValueExact?: (value: string) => void;
  setPosition?: (lineNumber: number, column: number) => void;
  getScroll?: () => { scrollTop: number; scrollLeft: number };
  setScroll?: (scrollTop: number, scrollLeft?: number) => void;
  getLanguage?: () => string | null;
  getRenderedTokenColor?: (tokenText: string, lineNumber?: number) => string | null;
  getTokenTypeAt?: (lineNumber: number, column: number) => string | null;
  applyEdits?: (edits: Array<{ range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number }; text: string }>) => void;
};

export type TreeaseEditorStoreBridge = {
  getState: () => EditorState;
  getWorkspace: () => EditorWorkspaceState;
  setLanguageId: (value: string) => void;
  setTempGraphSelection: (
    path: TreeaseBridgePathSeg[],
    target?: GraphHighlightTarget | 'node',
  ) => void;
};

export type TreeaseSettingsBridge = {
  getState: () => { document: SettingsDocument; settings: Settings; status: SettingsStatus };
  getStatus: () => SettingsStatus;
  save: (settings: Partial<Settings>) => Promise<void>;
  saveDocument: (document: SettingsDocument) => Promise<void>;
  reset: () => Promise<void>;
};

export type TreeaseGraphProbe = {
  id: string;
  target?: 'key' | 'value' | 'node';
  nodeType?: string;
  coord?: { x?: number; y?: number } | null;
  rect?: { left?: number; top?: number; width?: number; height?: number } | null;
  worldRect?: { left?: number; top?: number; width?: number; height?: number } | null;
  cell?: {
    text?: string;
    valueType?: string;
    isTableCell?: boolean;
    isHeader?: boolean;
    path?: TreeaseRuntimePathSeg[];
    value?: unknown;
  } | null;
};

export type TreeaseGraphInteractionState = {
  documentKey?: string;
  revision?: number;
  snapshotId?: number | null;
  renderToken?: number;
  mode?: 'committed' | 'streaming' | 'json-block';
  current?: boolean;
  hasGraphData?: boolean;
  nodeCount?: number;
  rootProbeCount?: number;
  pendingRenderWork?: boolean;
  interactiveReady?: boolean;
};

export type TreeaseGraphRuntime = {
  getInteractionState?: () => TreeaseGraphInteractionState | null;
  getClickProbeTargets?: (scope?: 'root' | 'panel') => TreeaseGraphProbe[];
  getHighlightTarget?: () => {
    path?: TreeaseRuntimePathSeg[];
    target?: 'key' | 'value' | 'node';
    rect?: { left?: number; top?: number; width?: number; height?: number } | null;
    world?: { highlight?: { x?: number; y?: number }; viewportCenter?: { x?: number; y?: number } } | null;
  } | null;
  getLastReveal?: () => {
    path?: TreeaseRuntimePathSeg[];
    target?: 'key' | 'value' | 'node';
  } | null;
  clearLastReveal?: () => void;
  getRowScrollState?: (path?: TreeaseRuntimePathSeg[] | null) => {
    path?: TreeaseRuntimePathSeg[];
    scrollY?: number;
  } | null;
  getPanelRect?: () => {
    path?: TreeaseRuntimePathSeg[];
    visible?: boolean;
    rect?: { left?: number; top?: number; width?: number; height?: number } | null;
  } | null;
  getHoverPreview?: () => { kind?: 'pre' | 'subgraph'; text?: string; language?: string; visible?: boolean } | null;
  getHoverPanelDebugState?: () => { phase?: string; error?: string } | null;
  getHoverPanelPrewarmDebugSnapshot?: () => {
    scheduledPaths?: TreeaseRuntimePathSeg[][];
    completedPaths?: TreeaseRuntimePathSeg[][];
    inFlightPath?: TreeaseRuntimePathSeg[] | null;
  } | null;
  getHitResult?: (point: { x: number; y: number }) => {
    scope?: 'root' | 'panel';
    point?: { x?: number; y?: number };
    hit?: {
      id?: string;
      target?: 'key' | 'value' | 'node';
      cell?: {
        path?: TreeaseRuntimePathSeg[];
        text?: string;
      } | null;
    } | null;
  } | null;
  getLastGraphData?: () => { nodes?: Array<{ path?: TreeaseRuntimePathSeg[]; kind?: string; depth?: number }> } | null;
  getStreamProgressState?: () => {
    visible?: boolean;
    streamRunId?: string;
    label?: string;
    detail?: string;
    value?: number;
    phase?: string;
    startedAt?: number | null;
    completedAt?: number | null;
  } | null;
  revealPath?: (
    path: TreeaseBridgePathSeg[],
    options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean },
  ) => void;
  activateProbe?: (probeId: string) => Promise<void>;
  commitProbe?: (probeId: string, text: string) => Promise<boolean>;
  scrollTableToRow?: (rowIndex: number) => void;
  refs?: unknown;
};
export type TreeaseGraphStreamState = Record<string, unknown> & {
  partialSeen: boolean;
  finalSeen: boolean;
  revision?: number;
  documentKey?: string;
  language?: string;
  totalBytes?: number;
  chunkSize?: number;
  chunkCount?: number;
  progressEventCount?: number;
  startedAtMs?: number | null;
  firstPartialAtMs?: number | null;
  finalSeenAtMs?: number | null;
  doneAtMs?: number | null;
  failedAtMs?: number | null;
  appliedAtMs?: number | null;
  errorMessage?: string;
  renderCalls?: number;
  lastRenderTextLength?: number;
  lastRenderLanguage?: string;
  lastUseStream?: boolean;
  requested?: boolean;
  receivedEvents?: number;
  acceptedEvents?: number;
  rejectedToken?: number;
  rejectedDocumentKey?: number;
  rejectedStreamId?: number;
  rejectedNotGraphDelta?: number;
  rejectedSeq?: number;
  reactiveRenderCalls?: number;
  lastReactiveRenderTextLength?: number;
  lastReactiveDocumentKey?: string;
  lastReactiveRevision?: number;
  lastReactiveLanguage?: string;
  lastPhase?: string;
};

export type TreeaseGraphTooltipRequest = {
  target: unknown;
  currentData?: unknown;
  language?: SupportedEditorLanguageId | string;
};

export type TreeaseGraphBuildResult = {
  nodes: GraphNode[];
  edges: GraphEdge[];
};

export type TreeaseGraphBridgeExtras = {
  buildTooltipContent?: (request: TreeaseGraphTooltipRequest) => string;
  buildGraph?: () => Promise<TreeaseGraphBuildResult>;
};

export type TreeaseWorkerBridge = {
  callShared: <T = unknown>(
    type: string,
    payload?: Record<string, any>,
    transfer?: Transferable[],
  ) => Promise<T>;
};

export type TreeasePreviewBridge = {
  generate: (options: {
    value: string;
    rawValue?: string;
    language?: SupportedEditorLanguageId | string;
  }) => Promise<string | string[] | null>;
};

export type TreeaseTestGraphEditEventDetail = {
  kind?: string;
  valueType?: string;
  path?: TreeaseRuntimePathSeg[];
  probes?: Array<{ x?: number; y?: number }>;
};

export type TreeaseTestGraphEditEvent = {
  type: string;
  detail: TreeaseTestGraphEditEventDetail;
};

export type WindowTreease = {
  editor: {
    isReady: (hookId: string) => boolean;
    getHookIds: () => string[];
    getState: () => EditorState;
    getWorkspace: () => EditorWorkspaceState;
    setLanguageId: (value: string) => void;
    setTempGraphSelection: (
      path: TreeaseBridgePathSeg[],
      target?: GraphHighlightTarget | 'node',
    ) => void;
    setValueExact?: (hookId: string, value: string) => void;
    getValue: (hookId: string) => string;
    setValue: (hookId: string, value: string) => void;
    setPosition: (hookId: string, lineNumber: number, column: number) => void;
    getScroll: (hookId: string) => { scrollTop: number; scrollLeft: number };
    setScroll: (hookId: string, scrollTop: number, scrollLeft?: number) => void;
    getLanguage: (hookId: string) => string | null;
    getRenderedTokenColor: (hookId: string, tokenText: string, lineNumber?: number) => string | null;
    getTokenTypeAt: (hookId: string, lineNumber: number, column: number) => string | null;
    applyEdits: (hookId: string, edits: Array<{ range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number }; text: string }>) => void;
  };
  settings: {
    getState: () => { document: SettingsDocument; settings: Settings; status: SettingsStatus };
    getStatus: () => SettingsStatus;
    save: (settings: Partial<Settings>) => Promise<void>;
    saveDocument: (document: SettingsDocument) => Promise<void>;
    reset: () => Promise<void>;
  };
  graph: {
    getInteractionState: () => ReturnType<NonNullable<TreeaseGraphRuntime['getInteractionState']>>;
    getClickProbeTargets: (scope?: 'root' | 'panel') => TreeaseGraphProbe[];
    getHighlightTarget: () => ReturnType<NonNullable<TreeaseGraphRuntime['getHighlightTarget']>>;
    getLastReveal: () => ReturnType<NonNullable<TreeaseGraphRuntime['getLastReveal']>>;
    clearLastReveal: () => void;
    getRowScrollState: (path?: TreeaseBridgePathSeg[] | null) => ReturnType<NonNullable<TreeaseGraphRuntime['getRowScrollState']>>;
    getPanelRect: () => ReturnType<NonNullable<TreeaseGraphRuntime['getPanelRect']>>;
    getHoverPreview: () => ReturnType<NonNullable<TreeaseGraphRuntime['getHoverPreview']>>;
    getHoverPanelDebugState: () => ReturnType<NonNullable<TreeaseGraphRuntime['getHoverPanelDebugState']>>;
    getHoverPanelPrewarmDebugSnapshot: () => ReturnType<
      NonNullable<TreeaseGraphRuntime['getHoverPanelPrewarmDebugSnapshot']>
    >;
    getHitResult: (point: { x: number; y: number }) => ReturnType<NonNullable<TreeaseGraphRuntime['getHitResult']>>;
    getLastGraphData: () => ReturnType<NonNullable<TreeaseGraphRuntime['getLastGraphData']>>;
    revealPath: (
      path: TreeaseBridgePathSeg[],
      options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean },
    ) => void;
    activateProbe: (probeId: string) => Promise<void>;
    commitProbe: (probeId: string, text: string) => Promise<boolean>;
    scrollTableToRow: (rowIndex: number) => void;
    getStreamState: () => TreeaseGraphStreamState | null;
    getStreamProgressState: () => ReturnType<NonNullable<TreeaseGraphRuntime['getStreamProgressState']>>;
    buildTooltipContent: (request: TreeaseGraphTooltipRequest) => string;
    buildGraph: () => Promise<TreeaseGraphBuildResult>;
    refs?: unknown;
  };
  worker: {
    callShared: TreeaseWorkerBridge['callShared'];
  };
  preview: {
    generate: TreeasePreviewBridge['generate'];
  };
  test: {
    resetClipboardWrites: () => void;
    pushClipboardWrite: (text: string) => void;
    getClipboardWrites: () => string[];
    resetGraphEditEvents: () => void;
    pushGraphEditEvent: (event: TreeaseTestGraphEditEvent) => void;
    getGraphEditEvents: () => TreeaseTestGraphEditEvent[];
  };
};
