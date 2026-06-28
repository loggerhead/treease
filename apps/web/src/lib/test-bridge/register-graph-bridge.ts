import { editorStore } from '../store/editor-store';
import { settingsStore } from '../settings/settings-store';
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import type {
  TreeaseGraphBuildResult,
  TreeaseGraphRuntime,
  TreeaseGraphStreamState,
} from './types';
import {
  clearTreeaseGraphRuntime,
  registerTreeaseGraphExtras,
  registerTreeaseGraphRuntime,
  setTreeaseGraphStreamState,
  updateTreeaseGraphStreamState,
} from './window-treease';

export function installGraphBridge(runtime: TreeaseGraphRuntime): void {
  registerTreeaseGraphRuntime(runtime);
}

export function clearGraphBridge(): void {
  clearTreeaseGraphRuntime();
}

export function installGraphExtrasBridge(): void {
  registerTreeaseGraphExtras({
    buildGraph: async () => {
      const state = editorStore.get();
      const settings = settingsStore.get();
      return callSharedWasmWorker<TreeaseGraphBuildResult>('buildGraph', {
        documentKey: state.documentKey,
        language: state.languageId,
        text: state.sourceText,
        revision: state.editorRevision,
        nest: !!settings.settings.parser.enableNest,
      });
    },
  });
}

export function resetGraphStreamState(): void {
  setTreeaseGraphStreamState({ partialSeen: false, finalSeen: false });
}

export function replaceGraphStreamState(state: TreeaseGraphStreamState): void {
  setTreeaseGraphStreamState(state);
}

export function mutateGraphStreamState(mutator: (state: TreeaseGraphStreamState) => void): void {
  updateTreeaseGraphStreamState(mutator);
}
