import { PathSegTag } from '@core-wasm/index'
import { activeTempModel, editorStore, type GraphHighlightTarget } from '../store/editor-store';
import type { PathSeg } from '../store/tree-path';
import { toWasmPathSeg } from '../../shared/brand-bridge';
import type { TreeaseBridgePathSeg, TreeaseMonacoHook } from './types';
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
  registerTreeaseEditorStoreBridge({
    getState: () => editorStore.get(),
    getWorkspace: () => editorStore.get().workspace,
    setLanguageId: (value: string) => editorStore.actions.setLanguageId(value as any),
    setTempGraphSelection: (path: TreeaseBridgePathSeg[], target?: GraphHighlightTarget | 'node') => {
      const normalized = normalizeBridgePath(path);
      const state = editorStore.get();
      const revision = Math.max(state.editorRevision, state.graphAppliedRevision);
      activeTempModel.update((current) => ({
        ...current,
        treePath: normalized,
        graphHighlight: {
          path: normalized,
          target,
          revision,
          source: 'graph',
        },
      }));
    },
  });
}
