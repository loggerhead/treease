import type {
  TreeaseEditorStoreBridge,
  TreeaseGraphBridgeExtras,
  TreeaseGraphRuntime,
  TreeaseGraphStreamState,
  TreeaseMonacoHook,
  TreeasePreviewBridge,
  TreeaseSettingsBridge,
  TreeaseTestGraphEditEvent,
  TreeaseUrlPresetState,
  TreeaseWorkerBridge,
  WindowTreease,
} from './types';
import { readRuntimeReadiness } from './runtime-readiness';

const editorHooks = new Map<string, TreeaseMonacoHook>();
let editorStoreBridge: TreeaseEditorStoreBridge | null = null;
let settingsBridge: TreeaseSettingsBridge | null = null;
let graphRuntime: TreeaseGraphRuntime | null = null;
let graphExtras: TreeaseGraphBridgeExtras | null = null;
let graphStreamState: TreeaseGraphStreamState | null = null;
let workerBridge: TreeaseWorkerBridge | null = null;
let previewBridge: TreeasePreviewBridge | null = null;
let urlPresetState: TreeaseUrlPresetState | null = null;
let clipboardWrites: string[] = [];
let graphEditEvents: TreeaseTestGraphEditEvent[] = [];

function missing(name: string): never {
  throw new Error(`window._treease.${name} is unavailable`);
}

function getEditorHook(hookId: string): TreeaseMonacoHook {
  const hook = editorHooks.get(hookId);
  if (!hook) missing(`editor["${hookId}"]`);
  return hook;
}

function getEditorStoreBridge(): TreeaseEditorStoreBridge {
  return editorStoreBridge ?? missing('editor.getState');
}

function getSettingsBridge(): TreeaseSettingsBridge {
  return settingsBridge ?? missing('settings.getState');
}

function getGraphRuntime(): TreeaseGraphRuntime {
  return graphRuntime ?? missing('graph runtime');
}

function getGraphExtras(): TreeaseGraphBridgeExtras {
  return graphExtras ?? missing('graph extras');
}

function getWorkerBridge(): TreeaseWorkerBridge {
  return workerBridge ?? missing('worker.callShared');
}

function getPreviewBridge(): TreeasePreviewBridge {
  return previewBridge ?? missing('preview.generate');
}

export function ensureWindowTreease(): WindowTreease | null {
  if (!(typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test'))) return null;
  const existing = window._treease;
  if (existing) return existing;

  const treease: WindowTreease = {
    editor: {
      isReady: (hookId) => editorHooks.has(hookId),
      getHookIds: () => Array.from(editorHooks.keys()),
      getState: () => getEditorStoreBridge().getState(),
      getWorkspace: () => getEditorStoreBridge().getWorkspace(),
      setLanguageId: (value) => getEditorStoreBridge().setLanguageId(value),
      setTempGraphSelection: (path, target) => getEditorStoreBridge().setTempGraphSelection(path, target),
      getValue: (hookId) => getEditorHook(hookId).getValue(),
      setValueExact: (hookId, value) => {
        const hook = getEditorHook(hookId);
        (hook.setValueExact ?? hook.setValue)(value);
      },
      setValue: (hookId, value) => getEditorHook(hookId).setValue(value),
      setPosition: (hookId, lineNumber, column) => {
        const fn = getEditorHook(hookId).setPosition;
        if (!fn) missing(`editor.setPosition("${hookId}")`);
        fn(lineNumber, column);
      },
      getScroll: (hookId) => {
        const fn = getEditorHook(hookId).getScroll;
        if (!fn) missing(`editor.getScroll("${hookId}")`);
        return fn();
      },
      setScroll: (hookId, scrollTop, scrollLeft) => {
        const fn = getEditorHook(hookId).setScroll;
        if (!fn) missing(`editor.setScroll("${hookId}")`);
        fn(scrollTop, scrollLeft);
      },
      getLanguage: (hookId) => getEditorHook(hookId).getLanguage?.() ?? null,
      getRenderedTokenColor: (hookId, tokenText, lineNumber) =>
        getEditorHook(hookId).getRenderedTokenColor?.(tokenText, lineNumber) ?? null,
      getTokenTypeAt: (hookId, lineNumber, column) =>
        getEditorHook(hookId).getTokenTypeAt?.(lineNumber, column) ?? null,
      applyEdits: (hookId, edits) => {
        const fn = getEditorHook(hookId).applyEdits;
        if (!fn) missing(`editor.applyEdits("${hookId}")`);
        fn(edits);
      },
    },
    settings: {
      getState: () => getSettingsBridge().getState(),
      getStatus: () => getSettingsBridge().getStatus(),
      save: (settings) => getSettingsBridge().save(settings),
      saveDocument: (document) => getSettingsBridge().saveDocument(document),
      reset: () => getSettingsBridge().reset(),
    },
    graph: {
      getInteractionState: () => getGraphRuntime().getInteractionState?.() ?? null,
      getRuntimeReadiness: () => graphRuntime?.getRuntimeReadiness?.() ?? readRuntimeReadiness(),
      getClickProbeTargets: (scope) => getGraphRuntime().getClickProbeTargets?.(scope) ?? [],
      getHighlightTarget: () => getGraphRuntime().getHighlightTarget?.() ?? null,
      getLastReveal: () => getGraphRuntime().getLastReveal?.() ?? null,
      clearLastReveal: () => getGraphRuntime().clearLastReveal?.(),
      getRowScrollState: (path) => getGraphRuntime().getRowScrollState?.(path) ?? null,
      getPanelRect: () => getGraphRuntime().getPanelRect?.() ?? null,
      getHitResult: (point) => getGraphRuntime().getHitResult?.(point) ?? null,
      getLastGraphData: () => getGraphRuntime().getLastGraphData?.() ?? null,
      revealPath: (path, options) => {
        const fn = getGraphRuntime().revealPath;
        if (!fn) missing('graph.revealPath');
        return fn(path, options);
      },
      activateProbe: (probeId) => {
        const fn = getGraphRuntime().activateProbe;
        if (!fn) missing('graph.activateProbe');
        return fn(probeId);
      },
      commitProbe: (probeId, text) => {
        const fn = getGraphRuntime().commitProbe;
        if (!fn) missing('graph.commitProbe');
        return fn(probeId, text);
      },
      scrollTableToRow: (rowIndex) => {
        const fn = getGraphRuntime().scrollTableToRow;
        if (!fn) missing('graph.scrollTableToRow');
        return fn(rowIndex);
      },
      getStreamState: () => graphStreamState,
      getStreamProgressState: () => getGraphRuntime().getStreamProgressState?.() ?? null,
      buildGraph: () => {
        const fn = getGraphExtras().buildGraph;
        if (!fn) missing('graph.buildGraph');
        return fn();
      },
      get refs() { return getGraphRuntime().refs; },
    },
    worker: {
      callShared: (type, payload, transfer) => getWorkerBridge().callShared(type, payload, transfer),
    },
    preview: {
      generate: (options) => getPreviewBridge().generate(options),
    },
    test: {
      getUrlPresetState: () => urlPresetState,
      resetClipboardWrites: () => {
        clipboardWrites = [];
      },
      pushClipboardWrite: (text) => {
        clipboardWrites.push(text);
      },
      getClipboardWrites: () => [...clipboardWrites],
      resetGraphEditEvents: () => {
        graphEditEvents = [];
      },
      pushGraphEditEvent: (event) => {
        graphEditEvents.push(event);
      },
      getGraphEditEvents: () => [...graphEditEvents],
    },
  };

  window._treease = treease;
  return treease;
}

export function registerTreeaseEditorHook(hookId: string, hook: TreeaseMonacoHook): void {
  if (!ensureWindowTreease()) return;
  editorHooks.set(hookId, hook);
}

export function unregisterTreeaseEditorHook(hookId: string): void {
  editorHooks.delete(hookId);
}

export function registerTreeaseEditorStoreBridge(bridge: TreeaseEditorStoreBridge): void {
  if (!ensureWindowTreease()) return;
  editorStoreBridge = bridge;
}

export function registerTreeaseSettingsBridge(bridge: TreeaseSettingsBridge): void {
  if (!ensureWindowTreease()) return;
  settingsBridge = bridge;
}

export function registerTreeaseGraphRuntime(runtime: TreeaseGraphRuntime): void {
  if (!ensureWindowTreease()) return;
  graphRuntime = runtime;
}

export function clearTreeaseGraphRuntime(): void {
  graphRuntime = null;
}

export function registerTreeaseGraphExtras(extras: TreeaseGraphBridgeExtras): void {
  if (!ensureWindowTreease()) return;
  graphExtras = extras;
}

export function setTreeaseGraphStreamState(state: TreeaseGraphStreamState): void {
  if (!ensureWindowTreease()) return;
  graphStreamState = state;
}

export function updateTreeaseGraphStreamState(mutator: (state: TreeaseGraphStreamState) => void): void {
  if (!ensureWindowTreease()) return;
  const current = graphStreamState ?? { partialSeen: false, finalSeen: false };
  mutator(current);
  graphStreamState = current;
}

export function registerTreeaseWorkerBridge(bridge: TreeaseWorkerBridge): void {
  if (!ensureWindowTreease()) return;
  workerBridge = bridge;
}

export function registerTreeasePreviewBridge(bridge: TreeasePreviewBridge): void {
  if (!ensureWindowTreease()) return;
  previewBridge = bridge;
}

export function setTreeaseUrlPresetState(state: TreeaseUrlPresetState | null): void {
  if (!ensureWindowTreease()) return;
  urlPresetState = state;
}

declare global {
  interface Window {
    _treease?: WindowTreease;
  }
}
