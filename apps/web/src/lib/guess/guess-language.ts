import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import type { SupportedEditorLanguageId } from '../monaco/language-support';

type DiagnosticsError = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

type DiagnosticsProvider = (language: SupportedEditorLanguageId, text: string) => Promise<DiagnosticsError[]>;

export async function guessLanguage(
  input: string,
  _diagnosticsProvider?: DiagnosticsProvider,
): Promise<SupportedEditorLanguageId | null> {
  return callSharedWasmWorker<SupportedEditorLanguageId | null>('guessLanguage', { text: input });
}
