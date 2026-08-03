import type * as Monaco from 'monaco-editor';
import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
import type { FreshnessScope } from '../../guards/freshness-scope';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import {
  readStoredDiagnosticsResult,
  type DiagnosticsResult,
  type WasmError,
} from '../../services/EditorDiagnostics';
import { resolveDocumentAnalysis } from '../../services/DocumentAnalysisResolver';

export type EditorAnalysisLike = Partial<
  Pick<
    DocumentAnalysisResult,
    'diagnostics' | 'semanticTokens' | 'tree' | 'value' | 'documentKey' | 'semanticTokenVersion' | 'sourceByteLength' | 'language'
  >
>;

export type EditorAnalysisFreshness = Pick<FreshnessScope, 'isCurrent' | 'step'>;

type ApplyDocumentAnalysisToEditorOptions = {
  monaco: typeof import('monaco-editor') | undefined;
  requestModel: Monaco.editor.ITextModel;
  requestLanguage: SupportedEditorLanguageId;
  requestDocumentKey: string;
  requestNest: boolean;
  freshness: EditorAnalysisFreshness;
  analysis?: EditorAnalysisLike | null;
  hasCurrentJsonBlockSelection?: (documentKey: string) => boolean;
  onResolvedAnalysis?: (analysis: EditorAnalysisLike) => void;
  updateTempModel: (updater: (current: any) => any) => void;
  primeSemanticTokensForDocument: (documentKey: string, semanticTokens: ArrayBuffer) => void;
  clearSemanticTokensForDocument: (documentKey?: string) => void;
  refreshSemanticTokensForLanguage: (languageId?: string) => void;
};

function shouldPrimeWholeDocumentSemanticTokens(
  language: SupportedEditorLanguageId,
  diagnostics: readonly unknown[],
): boolean {
  return !(language === 'json' && diagnostics.length > 0);
}

async function applyStoredDiagnosticsToEditor(params: {
  monaco: typeof import('monaco-editor');
  requestModel: Monaco.editor.ITextModel;
  requestLanguage: SupportedEditorLanguageId;
  requestDocumentKey: string;
  requestNest: boolean;
  freshness: EditorAnalysisFreshness;
  updateTempModel: (updater: (current: any) => any) => void;
  preloadedErrors?: WasmError[] | null;
}): Promise<DiagnosticsResult | null> {
  const result: DiagnosticsResult | null = await params.freshness.step(() =>
    readStoredDiagnosticsResult(
      params.monaco,
      params.requestModel,
      params.requestLanguage,
      params.requestDocumentKey,
      params.requestNest,
      params.preloadedErrors ?? null,
    ),
  );
  if (!result || !params.freshness.isCurrent()) return null;
  params.monaco.editor.setModelMarkers(params.requestModel, 'treease', result.markers);
  params.updateTempModel((current) => ({
    ...current,
    diagnostics: result.diagnostics,
    error: result.error,
    ...(result.diagnostics.length > 0 ? { treePath: [], graphHighlight: null } : {}),
  }));
  return result;
}

function syncWholeDocumentSemanticTokens(
  options: ApplyDocumentAnalysisToEditorOptions,
  analysis: EditorAnalysisLike,
): void {
  const diagnostics = analysis.diagnostics ?? [];
  if (
    analysis.semanticTokens instanceof ArrayBuffer &&
    shouldPrimeWholeDocumentSemanticTokens(options.requestLanguage, diagnostics)
  ) {
    options.primeSemanticTokensForDocument(options.requestDocumentKey, analysis.semanticTokens);
    return;
  }
  if (
    options.requestLanguage === 'json' &&
    diagnostics.length > 0 &&
    !(options.hasCurrentJsonBlockSelection?.(options.requestDocumentKey) ?? false)
  ) {
    options.clearSemanticTokensForDocument(options.requestDocumentKey);
  }
}

export async function applyDocumentAnalysisToEditor(
  options: ApplyDocumentAnalysisToEditorOptions,
): Promise<EditorAnalysisLike | null> {
  const resolved = await options.freshness.step(() =>
    resolveDocumentAnalysis({
      documentKey: options.requestDocumentKey,
      preloadedAnalysis: options.analysis,
    }),
  );
  if (!resolved || resolved.status !== 'resolved') return null;
  if (!options.freshness.isCurrent()) return null;
  options.onResolvedAnalysis?.(resolved.analysis);
  syncWholeDocumentSemanticTokens(options, resolved.analysis);

  const monaco = options.monaco;
  if (monaco) {
    await applyStoredDiagnosticsToEditor({
      monaco,
      requestModel: options.requestModel,
      requestLanguage: options.requestLanguage,
      requestDocumentKey: options.requestDocumentKey,
      requestNest: options.requestNest,
      freshness: options.freshness,
      updateTempModel: options.updateTempModel,
      preloadedErrors: (resolved.analysis.diagnostics ?? []) as WasmError[],
    });
    if (!options.freshness.isCurrent()) return null;
  }
  options.refreshSemanticTokensForLanguage(options.requestLanguage);
  return resolved.analysis;
}
