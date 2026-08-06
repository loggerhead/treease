import { PathSegTag } from '@core-wasm/index'
import { get } from 'svelte/store';
import type { GraphHighlightTarget } from '../store/graph-selection-store';
import { activeSidecarTempModel } from '../store/active-sidecar-state';
import {
  documentKey,
  emitEditorMutation,
  getDocumentSessionState,
  editorRevision,
} from '../store/document-session-store';
import { getEditorStateSnapshot } from '../store/editor-store';
import { activeFullEditUiState as fullEditUiState } from '../store/active-full-edit-ui-store';
import type { PathSeg } from '../store/tree-path';
import { getWorkspaceState } from '../store/workspace-store';
import { captureActiveSidecarTarget, updateSidecarTempModel } from '../store/sidecar-tab-state';
import { toWasmPathSeg } from '../../shared/brand-bridge';
import type { TreeaseBridgePathSeg, TreeaseMonacoHook } from './types';
import { syncRuntimeReadinessFromEditorState } from './runtime-readiness';
import {
  registerTreeaseEditorHook,
  registerTreeaseEditorStoreBridge,
  unregisterTreeaseEditorHook,
} from './window-treease';

function normalizeBridgePath(path: TreeaseBridgePathSeg[]): PathSeg[] {
  return (path ?? [])
    .map((segment) => {
      if (typeof segment?.tag === 'number') {
        return toWasmPathSeg({
          tag: segment.tag,
          key: typeof segment.key === 'string' ? segment.key : '',
          index: typeof segment.index === 'number' ? segment.index : 0,
        });
      }
      if (typeof segment?.key === 'string') {
        return toWasmPathSeg({ tag: PathSegTag.KEY, key: segment.key, index: 0 });
      }
      if (typeof segment?.index === 'number') {
        return toWasmPathSeg({ tag: PathSegTag.INDEX, key: '', index: segment.index });
      }
      return null;
    })
    .filter((segment): segment is PathSeg => segment !== null);
}

export function registerEditorHook(hookId: string, hook: TreeaseMonacoHook): void {
  registerTreeaseEditorHook(hookId, hook);
}

export function unregisterEditorHook(hookId: string): void {
  unregisterTreeaseEditorHook(hookId);
}

export function installEditorStoreBridge(): void {
  const syncBridgeState = () => {
    const state = getEditorStateSnapshot();
    syncRuntimeReadinessFromEditorState({
      documentKey: state.documentKey,
      editorRevision: state.editorRevision,
      fullEditUiState: state.fullEditUiState,
    });
  };
  syncBridgeState();
  documentKey.subscribe(() => syncBridgeState());
  editorRevision.subscribe(() => syncBridgeState());
  fullEditUiState.subscribe(() => syncBridgeState());
  registerTreeaseEditorStoreBridge({
    // The bridge exposes the rendered right-pane state. It must not read the
    // legacy main-tab TempModel now that graph interaction belongs to sidecar.
    getState: () => ({ ...getEditorStateSnapshot(), tempModel: get(activeSidecarTempModel) }),
    getWorkspace: () => getWorkspaceState(),
    setLanguageId: (value: string) =>
      emitEditorMutation({ type: 'changeLanguage', payload: { languageId: value as any } }),
    setTempGraphSelection: (path: TreeaseBridgePathSeg[], target?: GraphHighlightTarget | 'node') => {
      const normalized = normalizeBridgePath(path);
      const state = getDocumentSessionState();
      const revision = Math.max(state.editorRevision, state.graphAppliedRevision);
      const sidecarTarget = captureActiveSidecarTarget();
      if (sidecarTarget) {
        updateSidecarTempModel(sidecarTarget, (current) => ({
          ...current,
          treePath: normalized,
          graphHighlight: {
            path: normalized,
            target,
            revision,
            source: 'graph',
          },
        }));
      }
      syncBridgeState();
    },
  });
}
