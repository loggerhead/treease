// Responsibility: trigger WASM parsing/analysis and manage analysis results and diagnostics.
import type * as Monaco from 'monaco-editor';
import { type TreeNode } from '@core-wasm/index';
import { resolveDocumentAnalysis } from '../../services/DocumentAnalysisResolver';
import { resolveTreePathResult, toByteColumn } from '../../services/TreePathService';
import { resolveEditorPositionTargetResult } from './editor-position-target';
import { applyFailedTreePath, applyResolvedTreePath } from './EditorCore.graph-highlight';
import { createViewRuntimeOperation, type ViewRuntimeOperation } from '../../guards/view-runtime-operation';
import { supportedEditorLanguageSet, type SupportedEditorLanguageId } from '../../monaco/language-support';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { DocumentAnalysisResult, JsonBlockAtPositionResult } from '../../../shared/worker-protocol/protocol';
import { getWorkspaceSnapshotId } from '../../store/workspace-store';
import type { JsonBlockSelection } from '../../store/full-edit-ui-store';
import {
  applyDocumentAnalysisToEditor,
  type EditorAnalysisFreshness,
  type EditorAnalysisLike,
} from './editor-analysis-apply';
import { offsetSemanticTokens } from '../../monaco/semantic-token-offset';
import { buildDocumentJobSettings, runTextDocumentJobForGraph } from '../../graph-stream/document-job-runner';
import {
  isActiveDocumentSemanticPending,
  isActiveDocumentSemanticValid,
  shouldSuppressJsonBlockFallback,
} from '../../store/active-document-authority';
import { resolveReadableSnapshotId } from '../../store/active-document-authority';

type EditorFreshnessScope = EditorAnalysisFreshness;
type CachedAuthoritativeAnalysis = {
  documentKey: string;
  language: SupportedEditorLanguageId;
  revision: number;
  analysis: EditorAnalysisLike;
};

const jsonBlockTokenFormatting = {
  indent: 2,
  smart: true,
  maxLineLength: 100,
  maxInlineComplexity: 1,
  maxArrayInlineItems: 6,
  alignObjectArrays: true,
};

type CursorPathRequestPayload = {
  requestId: number;
  documentKey: string;
  revision: number;
  lineNumber: number;
  column: number;
  syncGraphHighlight: boolean;
};

type CursorPathSettledPayload = Omit<CursorPathRequestPayload, 'syncGraphHighlight'>;

type CreateEditorAnalysisControllerOptions = {
  getMonaco: () => typeof import('monaco-editor') | undefined;
  getEditor: () => Monaco.editor.IStandaloneCodeEditor | null;
  getModel: () => Monaco.editor.ITextModel | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getNestEnabled: () => boolean;
  getEditorRevision: () => number;
  isImportActive: () => boolean;
  getSourceText: () => string;
  getJsonBlockSelection: () => JsonBlockSelection | null;
  setJsonBlockSelection: (selection: JsonBlockSelection | null) => void;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  setTreeState: (value: { tree: TreeNode | null; value: unknown; source: 'editor'; revision: number }) => void;
  primeSemanticTokensForDocument: (documentKey: string, semanticTokens: ArrayBuffer) => void;
  clearSemanticTokensForDocument: (documentKey?: string) => void;
  refreshSemanticTokensForLanguage: (languageId?: string) => void;
  markCursorPathRequested?: (payload: CursorPathRequestPayload) => void;
  markCursorPathSettled?: (payload: CursorPathSettledPayload) => void;
};

export function createEditorAnalysisController(options: CreateEditorAnalysisControllerOptions) {
  let treePathRequestId = 0;
  let treePathOperation: ViewRuntimeOperation | null = null;
  let analysisOperation: ViewRuntimeOperation | null = null;
  let latestAuthoritativeAnalysis: CachedAuthoritativeAnalysis | null = null;

  function currentRevision(): number {
    return options.getEditorRevision();
  }

  function createJsonBlockDocumentKey(
    sourceDocumentKey: string,
    revision: number,
    block: Pick<JsonBlockAtPositionResult, 'startByte' | 'endByte'>,
  ): string {
    return `${sourceDocumentKey}:json-block:${revision}:${block.startByte}:${block.endByte}`;
  }

  function clearDocumentSemanticTokens(documentKey: string, languageId?: string): void {
    options.clearSemanticTokensForDocument(documentKey);
    options.refreshSemanticTokensForLanguage(languageId);
  }

  function getJsonBlockSelectionForDocument(documentKey: string): JsonBlockSelection | null {
    const current = options.getJsonBlockSelection();
    return current?.sourceDocumentKey === documentKey ? current : null;
  }

  function clearJsonBlockSelectionForDocument(documentKey: string, clearSemanticTokens = true): void {
    if (getJsonBlockSelectionForDocument(documentKey)) {
      options.setJsonBlockSelection(null);
      if (clearSemanticTokens) {
        clearDocumentSemanticTokens(documentKey, 'json');
      }
    }
  }

  function hasCurrentJsonBlockSelection(documentKey: string): boolean {
    return getJsonBlockSelectionForDocument(documentKey)?.revision === currentRevision();
  }

  function rememberAuthoritativeAnalysis(
    documentKey: string,
    language: SupportedEditorLanguageId,
    revision: number,
    analysis: EditorAnalysisLike,
  ): void {
    latestAuthoritativeAnalysis = {
      documentKey,
      language,
      revision,
      analysis,
    };
  }

  function clearAuthoritativeAnalysis(documentKey?: string): void {
    if (!documentKey || latestAuthoritativeAnalysis?.documentKey === documentKey) {
      latestAuthoritativeAnalysis = null;
    }
  }

  function getCurrentAuthoritativeAnalysis(
    documentKey: string,
    language: SupportedEditorLanguageId,
  ): EditorAnalysisLike | null {
    const current = latestAuthoritativeAnalysis;
    if (!current) return null;
    if (current.documentKey !== documentKey || current.language !== language) return null;
    if (current.revision !== currentRevision()) return null;
    return current.analysis;
  }

  async function resolveCurrentAuthoritativeAnalysis(
    documentKey: string,
    language: SupportedEditorLanguageId,
    freshness: EditorFreshnessScope,
  ): Promise<EditorAnalysisLike | null | undefined> {
    const cached = getCurrentAuthoritativeAnalysis(documentKey, language);
    if (cached) return cached;
    const resolved = await freshness.step(() =>
      resolveDocumentAnalysis({
        documentKey,
      }),
    );
    if (!resolved) return undefined;
    return resolved.status === 'resolved' ? resolved.analysis : null;
  }

  function isValidWholeDocumentAnalysis(
    documentKey: string,
    analysis: EditorAnalysisLike | null | undefined,
  ): boolean {
    const diagnostics = analysis?.diagnostics ?? [];
    if (isActiveDocumentSemanticValid(documentKey, currentRevision())) return true;
    return getWorkspaceSnapshotId(documentKey) != null && diagnostics.length === 0;
  }

  async function updateJsonBlockSelection(
    requestModel: Monaco.editor.ITextModel,
    position: Monaco.IPosition,
    requestDocumentKey: string,
    requestLanguage: SupportedEditorLanguageId,
    freshness: EditorFreshnessScope,
  ): Promise<void> {
    if (requestLanguage !== 'json') {
      clearJsonBlockSelectionForDocument(requestDocumentKey);
      return;
    }

    if (isActiveDocumentSemanticValid(requestDocumentKey, currentRevision())) {
      clearJsonBlockSelectionForDocument(requestDocumentKey, false);
      return;
    }
    if (shouldSuppressJsonBlockFallback(requestDocumentKey, currentRevision())) {
      clearJsonBlockSelectionForDocument(requestDocumentKey);
      return;
    }
    if (isActiveDocumentSemanticPending(requestDocumentKey, currentRevision())) {
      return;
    }

    const analysis = await resolveCurrentAuthoritativeAnalysis(requestDocumentKey, requestLanguage, freshness);
    if (analysis === undefined) return;
    if (isValidWholeDocumentAnalysis(requestDocumentKey, analysis)) {
      clearJsonBlockSelectionForDocument(requestDocumentKey, false);
      return;
    }

    const row = Math.max(0, position.lineNumber - 1);
    const columnIndex = Math.max(0, position.column - 1);
    const lineText = requestModel.getLineContent(position.lineNumber);
    const column = toByteColumn(lineText, columnIndex);
    const block = await freshness.step(() =>
      callSharedWasmWorker<JsonBlockAtPositionResult>('findJsonBlockAtPosition', {
        documentKey: requestDocumentKey,
        language: requestLanguage,
        text: options.getSourceText(),
        row,
        column,
      }),
    );
    if (!block) return;
    if (!freshness.isCurrent()) return;
    if (!block.found) {
      clearJsonBlockSelectionForDocument(requestDocumentKey);
      return;
    }

    const revision = currentRevision();
    const selection: JsonBlockSelection = {
      sourceDocumentKey: requestDocumentKey,
      blockDocumentKey: createJsonBlockDocumentKey(requestDocumentKey, revision, block),
      revision,
      language: 'json',
      text: block.text,
      startByte: block.startByte,
      endByte: block.endByte,
      startLineNumber: block.startLineNumber,
      startColumn: block.startColumn,
      endLineNumber: block.endLineNumber,
      endColumn: block.endColumn,
    };
    options.setJsonBlockSelection(selection);
    const blockAnalysis = await freshness.step(() =>
      runTextDocumentJobForGraph({
        documentKey: selection.blockDocumentKey,
        language: selection.language,
        text: selection.text,
        settings: buildDocumentJobSettings({
          enableNest: options.getNestEnabled(),
          formatting: jsonBlockTokenFormatting,
          formatSourceOnClose: false,
        }),
        outputAnalysis: true,
        outputGraph: false,
      }),
    );
    if (!blockAnalysis || !freshness.isCurrent()) return;
    if (blockAnalysis.analysis?.semanticTokens instanceof ArrayBuffer) {
      options.primeSemanticTokensForDocument(
        requestDocumentKey,
        offsetSemanticTokens(blockAnalysis.analysis.semanticTokens, selection.startLineNumber, selection.startColumn),
      );
    }
    options.refreshSemanticTokensForLanguage('json');
  }

  function prepareLanguageSwitchAnalysisReset(): void {
    void treePathOperation?.cancel();
    void analysisOperation?.cancel();
    const currentDocumentKey = options.getDocumentKey();
    clearAuthoritativeAnalysis(currentDocumentKey || undefined);
    if (currentDocumentKey) {
      clearDocumentSemanticTokens(currentDocumentKey);
    }
  }

  function clearEditorDiagnostics(requestModel: Monaco.editor.ITextModel): void {
    const monaco = options.getMonaco();
    if (!monaco) return;
    monaco.editor.setModelMarkers(requestModel, 'treease', []);
    options.updateActiveTempModel((current) => ({ ...current, diagnostics: [], error: '' }));
  }

  async function updateTreePath(position: Monaco.IPosition | null, state?: { syncGraphHighlight?: boolean }): Promise<void> {
    if (!position) return;
    if (options.isImportActive()) return;
    const syncGraphHighlight = state?.syncGraphHighlight ?? false;
    const requestLanguage = options.getLanguageId();
    if (!supportedEditorLanguageSet.has(requestLanguage)) {
      options.updateActiveTempModel((current) => applyFailedTreePath(current, syncGraphHighlight));
      return;
    }
    const requestModel = options.getModel();
    if (!requestModel) return;
    const requestDocumentKey = options.getDocumentKey();
    if (!requestDocumentKey) return;
    void treePathOperation?.cancel();
    const operation = createViewRuntimeOperation({
      captured: {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        revision: options.getEditorRevision(),
        model: requestModel,
      },
      getCurrent: () => ({
        documentKey: options.getDocumentKey(),
        languageId: options.getLanguageId(),
        revision: options.getEditorRevision(),
        model: options.getModel(),
      }),
    });
    treePathOperation = operation;
    const requestId = (treePathRequestId += 1);
    const requestRevision = options.getEditorRevision();
    const cursorPathPayload = {
      requestId,
      documentKey: requestDocumentKey,
      revision: requestRevision,
      lineNumber: position.lineNumber,
      column: position.column,
    };
    options.markCursorPathRequested?.({
      ...cursorPathPayload,
      syncGraphHighlight,
    });
    const requestSnapshotId = resolveReadableSnapshotId(requestDocumentKey, requestRevision);
    const hasJsonBlockSelection = getJsonBlockSelectionForDocument(requestDocumentKey) != null;
    const shouldUpdateJsonBlockSelection =
      requestLanguage === 'json' && (syncGraphHighlight || requestSnapshotId == null || hasJsonBlockSelection);
    await operation.run({
      execute: async ({ step }) => {
        if (shouldUpdateJsonBlockSelection) {
          await updateJsonBlockSelection(
            requestModel,
            position,
            requestDocumentKey,
            requestLanguage,
            operation,
          );
        }
        const treePathResult = await step(() =>
          resolveTreePathResult(
            requestModel,
            position,
            requestDocumentKey,
            requestLanguage,
            options.getNestEnabled(),
            requestSnapshotId,
          ),
        );
        if (treePathResult.status !== 'ready') return null;
        const treePath = treePathResult.data;
        const graphHighlightTargetResult = treePath.length
          ? await step(() =>
              resolveEditorPositionTargetResult(
                requestModel,
                position,
                treePath,
                requestDocumentKey,
                requestLanguage,
                options.getNestEnabled(),
              ),
            )
          : undefined;
        if (graphHighlightTargetResult?.status === 'snapshotNotReady') return null;
        return { treePath, graphHighlightTarget: graphHighlightTargetResult?.data ?? undefined };
      },
      land: async (resolved) => {
        if (!resolved) return;
        options.updateActiveTempModel((current) => ({
          ...applyResolvedTreePath(current, {
            treePath: resolved.treePath,
            target: resolved.graphHighlightTarget,
            revision: options.getEditorRevision(),
            syncGraphHighlight,
          }),
        }));
        options.markCursorPathSettled?.(cursorPathPayload);
        if (syncGraphHighlight && !shouldUpdateJsonBlockSelection) {
          await operation.step(() =>
            updateJsonBlockSelection(
              requestModel,
              position,
              requestDocumentKey,
              requestLanguage,
              operation,
            ),
          );
        }
      },
    });
  }

  async function syncStoredAnalysisToEditor(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    requestNest: boolean,
    freshness: EditorFreshnessScope,
    analysis: EditorAnalysisLike | null = null,
  ): Promise<void> {
    await applyDocumentAnalysisToEditor({
      monaco: options.getMonaco(),
      requestModel,
      requestLanguage,
      requestDocumentKey,
      requestNest,
      freshness,
      analysis,
      hasCurrentJsonBlockSelection,
      onResolvedAnalysis: (resolvedAnalysis) => {
        rememberAuthoritativeAnalysis(
          requestDocumentKey,
          requestLanguage,
          currentRevision(),
          resolvedAnalysis,
        );
      },
      updateTempModel: options.updateActiveTempModel,
      primeSemanticTokensForDocument: options.primeSemanticTokensForDocument,
      clearSemanticTokensForDocument: options.clearSemanticTokensForDocument,
      refreshSemanticTokensForLanguage: options.refreshSemanticTokensForLanguage,
    });
  }

  async function applyGraphAnalysis(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
  ): Promise<void> {
    void analysisOperation?.cancel();
    const operation = createViewRuntimeOperation({
      captured: {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        model: requestModel,
        revision,
      },
      getCurrent: () => ({
        documentKey: options.getDocumentKey(),
        languageId: options.getLanguageId(),
        model: options.getModel(),
        revision: currentRevision(),
      }),
    });
    analysisOperation = operation;
    await operation.run({
      execute: async ({ step }) => {
        await step(() =>
          syncStoredAnalysisToEditor(
            requestModel,
            requestLanguage,
            requestDocumentKey,
            options.getNestEnabled(),
            operation,
            analysis,
          ),
        );
      },
      land: async () => {
        options.setTreeState({
          tree: null,
          value: null,
          source: 'editor',
          revision,
        });
        await operation.step(() => updateTreePath(options.getEditor()?.getPosition() ?? null, { syncGraphHighlight: false }));
      },
    });
  }

  return {
    prepareLanguageSwitchAnalysisReset,
    clearEditorDiagnostics,
    updateTreePath,
    applyGraphAnalysis,
  };
}
