import type * as Monaco from 'monaco-editor';
import { buildEditorTheme } from '../../settings/ui-settings';
import { getSharedMonacoShell, getSharedMonacoLanguageServices, type MonacoShell, type MonacoLanguageServices } from '../../monaco/editor-runtime';

type CreateEditorRuntimeControllerOptions = {
  getSettings: () => any;
  getThemeName: () => string;
  isImportActive: () => boolean;
  callWasmWorker: <T>(method: string, input: unknown) => Promise<T>;
  getWorkerClient: () => Promise<unknown>;
  setMonaco: (value: typeof import('monaco-editor')) => void;
};

export function createEditorRuntimeController(options: CreateEditorRuntimeControllerOptions) {
  let shell: MonacoShell | null = null;

  /**
   * Phase 1: Initialize Monaco editor shell (no WASM needed).
   * After this returns, the editor can be created and rendered.
   */
  async function initShell(): Promise<MonacoShell> {
    shell = await getSharedMonacoShell();
    options.setMonaco(shell.monaco);
    return shell;
  }

  /**
   * Phase 2: Initialize language services (semantic tokens, colors, yq).
   * Requires the WASM worker to be ready.
   */
  async function initLanguageServices(): Promise<MonacoLanguageServices> {
    const langServices = await getSharedMonacoLanguageServices({
      callWasmWorker: options.callWasmWorker,
      getTokenTypes: () => options.callWasmWorker<readonly string[]>('semanticTokensLegend', undefined),
      isImportActive: options.isImportActive,
    });
    return langServices;
  }


  function applyTheme(monaco: typeof import('monaco-editor')): void {
    const themeName = options.getThemeName();
    const theme = buildEditorTheme(options.getSettings());
    monaco.editor.defineTheme(themeName, theme as unknown as Monaco.editor.IStandaloneThemeData);
    monaco.editor.setTheme(themeName);
  }

  function scheduleWorkerWarmup(): void {
    void options
      .getWorkerClient()
      .then(() => undefined)
      .catch((error) => {
        console.error('[editor] worker warmup failed', error);
      });
  }

  return {
    initShell,
    initLanguageServices,
    applyTheme,
    scheduleWorkerWarmup,
  };
}
