// 职责：Editor 文档分析控制器：触发 WASM parse/analysis、管理 analysis 结果与 diagnostics
import type * as Monaco from 'monaco-editor';
import { type TreeNode } from '@core-wasm/index'
import { analyzeDocumentAndStore } from '../../services/EditorDiagnostics';
import { resolveDocumentAnalysis } from '../../services/DocumentAnalysisResolver';
import { resolveTreePathSafe, toByteColumn } from '../../services/TreePathService';
import { resolveEditorPositionTarget } from './editor-position-target';
import { applyFailedTreePath, applyResolvedTreePath } from './EditorCore.graph-highlight';
import { createFreshnessScope } from '../../guards/freshness-scope';
import { supportedEditorLanguageSet, type SupportedEditorLanguageId } from '../../monaco/language-support';
import { offsetSemanticTokens } from '../../monaco/semantic-token-offset';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { DocumentAnalysisResult, JsonBlockAtPositionResult } from '../../../shared/worker-protocol/protocol';
import { bindActiveDocumentSnapshotIfPresent, getActiveDocumentSnapshotId } from '../../services/DocumentSessionService';
import type { JsonBlockSelection } from '../../store/editor-store';
import { applyDocumentAnalysisToEditor, type EditorAnalysisLike } from './editor-analysis-apply';

type EditorFreshnessScope = ReturnType<typeof createFreshnessScope>;
type CachedAuthoritativeAnalysis = {
  documentKey: string;
  language: SupportedEditorLanguageId;
  revision: number;
  analysis: EditorAnalysisLike;
};

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
};

export function createEditorAnalysisController(options: CreateEditorAnalysisControllerOptions) {
  let analysisSyncToken = 0;
  let treePathToken = 0;
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

  function clearJsonBlockSelectionForDocument(documentKey: string, clearSemanticTokens = true): void {
    const current = options.getJsonBlockSelection();
    if (current?.sourceDocumentKey === documentKey) {
      options.setJsonBlockSelection(null);
      if (clearSemanticTokens) {
        clearDocumentSemanticTokens(documentKey, 'json');
      }
    }
  }

  function hasCurrentJsonBlockSelection(documentKey: string): boolean {
    const current = options.getJsonBlockSelection();
    return current?.sourceDocumentKey === documentKey && current.revision === currentRevision();
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

  function isValidWholeDocumentAnalysis(analysis: EditorAnalysisLike | null | undefined): boolean {
    const diagnostics = analysis?.diagnostics ?? [];
    return Boolean(analysis?.tree) && diagnostics.length === 0;
  }

  async function primeJsonBlockSemanticTokens(
    requestDocumentKey: string,
    selection: JsonBlockSelection,
    freshness: ReturnType<typeof createFreshnessScope>,
  ): Promise<void> {
    const blockAnalysis = await freshness.step(() =>
      analyzeDocumentAndStore(
        selection.language,
        selection.text,
        selection.blockDocumentKey,
        options.getNestEnabled(),
      ),
    );
    if (!blockAnalysis) return;
    if (!freshness.isCurrent()) return;

    const shiftedTokens = offsetSemanticTokens(
      blockAnalysis.semanticTokens,
      selection.startLineNumber,
      selection.startColumn,
    );
    options.primeSemanticTokensForDocument(requestDocumentKey, shiftedTokens);
    options.refreshSemanticTokensForLanguage('json');
  }

  async function updateJsonBlockSelection(
    requestModel: Monaco.editor.ITextModel,
    position: Monaco.IPosition,
    requestDocumentKey: string,
    requestLanguage: SupportedEditorLanguageId,
    freshness: ReturnType<typeof createFreshnessScope>,
  ): Promise<void> {
    if (requestLanguage !== 'json') {
      clearJsonBlockSelectionForDocument(requestDocumentKey);
      return;
    }

    const analysis = await resolveCurrentAuthoritativeAnalysis(requestDocumentKey, requestLanguage, freshness);
    if (analysis === undefined) return;
    if (isValidWholeDocumentAnalysis(analysis)) {
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
    await primeJsonBlockSemanticTokens(requestDocumentKey, selection, freshness);
    if (!freshness.isCurrent()) return;
    options.setJsonBlockSelection(selection);
  }

  function prepareLanguageSwitchAnalysisReset(): void {
    analysisSyncToken += 1;
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
    const requestToken = (treePathToken += 1);
    const freshness = createFreshnessScope(
      {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        model: requestModel,
        token: requestToken,
      },
      () => ({
        documentKey: options.getDocumentKey(),
        languageId: options.getLanguageId(),
        model: options.getModel(),
        token: treePathToken,
      }),
    );
    const treePath = await freshness.step(() =>
      resolveTreePathSafe(
        requestModel,
        position,
        requestDocumentKey,
        requestLanguage,
        options.getNestEnabled(),
        getActiveDocumentSnapshotId(requestDocumentKey),
      ),
    );
    if (!treePath) return;
    const graphHighlightTargetResult = treePath.length
      ? await freshness.step(() =>
          resolveEditorPositionTarget(
            requestModel,
            position,
            treePath,
            requestDocumentKey,
            requestLanguage,
            options.getNestEnabled(),
          ),
        )
      : undefined;
    if (!freshness.isCurrent()) return;
    const graphHighlightTarget = graphHighlightTargetResult ?? undefined;
    options.updateActiveTempModel((current) => ({
      ...applyResolvedTreePath(current, {
        treePath,
        target: graphHighlightTarget,
        revision: options.getEditorRevision(),
        syncGraphHighlight,
      }),
    }));
    if (syncGraphHighlight) {
      await updateJsonBlockSelection(
        requestModel,
        position,
        requestDocumentKey,
        requestLanguage,
        freshness,
      );
    }
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

  async function syncAuthoritativeAnalysis(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    requestNest: boolean,
  ): Promise<void> {
    const syncToken = analysisSyncToken + 1;
    analysisSyncToken = syncToken;
    const freshness = createFreshnessScope(
      {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        model: requestModel,
        revision: currentRevision(),
        token: syncToken,
      },
      () => ({
        documentKey: options.getDocumentKey(),
        languageId: options.getLanguageId(),
        model: options.getModel(),
        revision: currentRevision(),
        token: analysisSyncToken,
      }),
    );
    const text = requestModel.getValue();
    if (!requestDocumentKey) {
      clearAuthoritativeAnalysis(requestDocumentKey || undefined);
      if (!freshness.isCurrent()) return;
      clearEditorDiagnostics(requestModel);
      options.setTreeState({
        tree: null,
        value: null,
        source: 'editor',
        revision: currentRevision(),
      });
      options.updateActiveTempModel((current) => ({
        ...current,
        treePath: [],
        graphHighlight: null,
      }));
      await freshness.step(() => updateTreePath(options.getEditor()?.getPosition() ?? null, { syncGraphHighlight: false }));
      return;
    }
    const analysis = await freshness.step(() =>
      analyzeDocumentAndStore(requestLanguage, text, requestDocumentKey, requestNest, {
        onAnalysisDelta: async (delta) => {
          if (!freshness.isCurrent()) return;
          await syncStoredAnalysisToEditor(
            requestModel,
            requestLanguage,
            requestDocumentKey,
            requestNest,
            freshness,
            delta,
          );
        },
      }),
    );
    if (!analysis) {
      clearAuthoritativeAnalysis(requestDocumentKey);
      if (!freshness.isCurrent()) return;
      clearEditorDiagnostics(requestModel);
      options.setTreeState({
        tree: null,
        value: null,
        source: 'editor',
        revision: currentRevision(),
      });
      options.updateActiveTempModel((current) => ({
        ...current,
        treePath: [],
        graphHighlight: null,
      }));
      await freshness.step(() => updateTreePath(options.getEditor()?.getPosition() ?? null, { syncGraphHighlight: false }));
      return;
    }
    bindActiveDocumentSnapshotIfPresent({
      documentKey: requestDocumentKey,
      revision: currentRevision(),
      snapshotId: analysis.snapshotId,
    });
    await syncStoredAnalysisToEditor(
      requestModel,
      requestLanguage,
      requestDocumentKey,
      requestNest,
      freshness,
      analysis,
    );
    await freshness.step(() => updateTreePath(options.getEditor()?.getPosition() ?? null, { syncGraphHighlight: false }));
  }

  async function applyGraphAnalysis(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
  ): Promise<void> {
    const current = analysisSyncToken + 1;
    analysisSyncToken = current;
    const freshness = createFreshnessScope(
      {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        model: requestModel,
        revision,
        token: current,
      },
      () => ({
        documentKey: options.getDocumentKey(),
        languageId: options.getLanguageId(),
        model: options.getModel(),
        revision: currentRevision(),
        token: analysisSyncToken,
      }),
    );
    await syncStoredAnalysisToEditor(
      requestModel,
      requestLanguage,
      requestDocumentKey,
      options.getNestEnabled(),
      freshness,
      analysis,
    );
    if (!freshness.isCurrent()) return;
    options.setTreeState({
      tree: analysis?.tree ? (analysis.tree as TreeNode) : null,
      value: analysis?.value ?? null,
      source: 'editor',
      revision,
    });
    await freshness.step(() => updateTreePath(options.getEditor()?.getPosition() ?? null, { syncGraphHighlight: false }));
  }

  return {
    prepareLanguageSwitchAnalysisReset,
    clearEditorDiagnostics,
    syncAuthoritativeAnalysis,
    updateTreePath,
    applyGraphAnalysis,
  };
}
