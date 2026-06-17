<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import type * as Monaco from 'monaco-editor';

  import { createFreshnessScope } from '../../guards/freshness-scope';
  import { buildDocumentJobSettings } from '../../graph-stream/document-job-runner';
  import { buildGraphStreamBuilderConfig } from '../../graph-stream/graph-stream-builder-config';
  import { editorLanguageFallback, type SupportedEditorLanguageId } from '../../monaco/language-support';
  import { getSharedMonacoRuntime } from '../../monaco/editor-runtime';
  import { attachMonacoTestHook } from '../../monaco/test-hook';
  import type { DiffPlan } from '../../graph/diff-plan';
  import { settings } from '../../settings/settings-store';
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../wasm/wasm-worker-singleton';
  import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
  import { editorStore, editorWorkspace } from '../../store/editor-store';
  import { clearActiveDocumentSnapshot, getActiveDocumentSnapshotId } from '../../services/DocumentSessionService';
  import { monacoChangesToDocumentTextEdits, type MonacoTextChange } from '../../../shared/document-text-edits';
  import { applyDocumentAnalysisToEditor } from './editor-analysis-apply';
  import { createEditorFullEditController } from './editor-full-edit-controller';
  import { createWorkspaceTabFullEditSink } from './editor-full-edit-sink';
  import { commitEditorTabTextChange } from './editor-tab-edit-commit';

  export let tabId = 'tab-sidecar';
  export let language: SupportedEditorLanguageId = editorLanguageFallback;
  export let onScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {};
  export let onContentChange: () => void = () => {};

  let container: HTMLDivElement;
  let monaco: typeof import('monaco-editor') | undefined;
  let editor: Monaco.editor.IStandaloneCodeEditor | null = null;
  let model: Monaco.editor.ITextModel | null = null;
  let cleanupTestHook: (() => void) | null = null;
  let diffDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let diffBlankZoneIds: string[] = [];
  let runtimeToken = 0;
  let sidecarAnalysisSyncToken = 0;
  let suppressChange = false;
  let lastModelLength = 0;
  let lastModelText = '';
  let activeLanguage: SupportedEditorLanguageId = language;
  let lastPropLanguage: SupportedEditorLanguageId = language;
  let ensureSemanticTokensProvider: (languageId: string) => void = () => {};
  let ensureDocumentColorProvider: (languageId: string) => void = () => {};
  let primeSemanticTokensForDocument: (documentKey: string, semanticTokens: ArrayBuffer) => void = () => {};
  let refreshSemanticTokensForLanguage: (languageId?: string) => void = () => {};
  let clearSemanticTokensForDocument: (documentKey?: string) => void = () => {};

  $: sidecarTab = $editorWorkspace.tabsById[tabId] ?? null;

  const SIDECAR_NAME = 'Right Editor';
  const fullEditSink = createWorkspaceTabFullEditSink(tabId);

  const fullEditController = createEditorFullEditController({
    getModel: () => model,
    getEditor: () => editor,
    getMonaco: () => monaco,
    getLanguageId: () => activeLanguage,
    getNestEnabled: () => $settings.parser.enableNest,
    getGraphBuilderConfig: () => buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
    getFullEditUiState: () => fullEditSink.getState(),
    fullEditSink,
    rotateActiveDocumentKey: () => sidecarDocumentKey(),
    setModelDocumentKey,
    setActiveTabDocumentKey: (documentKey) => setModelDocumentKey(model, documentKey),
    clearSemanticTokensForDocument: (documentKey) => clearSemanticTokensForDocument(documentKey),
    setEditorValue,
    setSourceText: (value) => {
      if (!fullEditController.isImportActive()) return;
      updateSidecarSourceText(value);
    },
    setDocumentKey: (documentKey) => setModelDocumentKey(model, documentKey),
    applyImportLanguage: (nextLanguage) => {
      activeLanguage = nextLanguage;
      setWorkspaceSidecarLanguage(nextLanguage);
      setModelLanguage(nextLanguage);
    },
    getFormattingOptions: () => $settings.formatting,
    callWasmWorker: (method, input) => callSharedWasmWorker(method, input as Record<string, any>),
    updateActiveTempModel: updateSidecarTempModel,
    commitEditorState: commitSidecarEditorState,
    applyGraphAnalysis: applySidecarGraphAnalysis,
    triggerGraphSync: () => {},
  });

  function sidecarDocumentKey(): string {
    return editorStore.get().workspace.tabsById[tabId]?.documentKey ?? `sidecar:${tabId}:0`;
  }

  function setModelDocumentKey(target: Monaco.editor.ITextModel | null, documentKey: string): void {
    if (!target || !documentKey) return;
    (target as Monaco.editor.ITextModel & { __treeaseDocumentKey?: string }).__treeaseDocumentKey = documentKey;
  }

  function syncLastModelSnapshot(): string {
    const text = model?.getValue() ?? '';
    lastModelLength = text.length;
    lastModelText = text;
    return text;
  }

  function currentTempModel() {
    return editorStore.get().workspace.tabsById[tabId]?.tempModel ?? editorStore.get().tempModel;
  }

  function updateSidecarTempModel(updater: (current: any) => any): void {
    editorStore.actions.updateWorkspaceTab(tabId, { tempModel: updater(currentTempModel()) });
  }

  function ensureWorkspaceSidecarTab(sourceText: string): void {
    editorStore.actions.ensureSidecarWorkspaceTab({
      id: tabId,
      name: SIDECAR_NAME,
      sourceText,
    });
  }

  function setWorkspaceSidecarLanguage(languageId: SupportedEditorLanguageId): void {
    editorStore.actions.updateWorkspaceTab(tabId, { languageId });
  }

  function commitSidecarEditorState(): number {
    const current = editorStore.get().workspace.tabsById[tabId];
    const revision = (current?.revision ?? 0) + 1;
    editorStore.actions.updateWorkspaceTab(tabId, {
      languageId: activeLanguage,
      revision,
    });
    return revision;
  }

  async function applySidecarGraphAnalysis(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
  ): Promise<void> {
    if (requestModel !== model) return;
    setModelDocumentKey(requestModel, requestDocumentKey);
    ensureSemanticTokensProvider(requestLanguage);
    const token = ++sidecarAnalysisSyncToken;
    const freshness = createFreshnessScope(
      {
        documentKey: requestDocumentKey,
        languageId: requestLanguage,
        model: requestModel,
        revision,
        token,
      },
      () => {
        const current = editorStore.get().workspace.tabsById[tabId];
        return {
          documentKey: sidecarDocumentKey(),
          languageId: activeLanguage,
          model,
          revision: current?.revision ?? -1,
          token: sidecarAnalysisSyncToken,
        };
      },
    );
    await applyDocumentAnalysisToEditor({
      monaco,
      requestModel,
      requestLanguage,
      requestDocumentKey,
      requestNest: $settings.parser.enableNest,
      freshness,
      analysis,
      updateTempModel: updateSidecarTempModel,
      primeSemanticTokensForDocument,
      clearSemanticTokensForDocument,
      refreshSemanticTokensForLanguage,
    });
  }

  function updateSidecarSourceText(value: string, options: { clearSnapshot?: boolean } = {}): void {
    const documentKey = sidecarDocumentKey();
    const shouldClearSnapshot = options.clearSnapshot ?? true;
    editorStore.actions.updateWorkspaceTab(tabId, {
      languageId: activeLanguage,
      sourceText: value,
      ...(shouldClearSnapshot ? { snapshotId: null } : {}),
      tempModel: {
        ...currentTempModel(),
        scratchText: value,
      },
    });
    if (shouldClearSnapshot) {
      clearActiveDocumentSnapshot(documentKey);
    }
    clearSemanticTokensForDocument(documentKey);
    refreshSemanticTokensForLanguage(activeLanguage);
  }

  function setModelLanguage(nextLanguage: SupportedEditorLanguageId): void {
    if (!model || !monaco) return;
    if (model.getLanguageId() === nextLanguage) return;
    monaco.editor.setModelLanguage(model, nextLanguage);
    ensureSemanticTokensProvider(nextLanguage);
    ensureDocumentColorProvider(nextLanguage);
  }

  function setModelValueSilently(
    target: Monaco.editor.ITextModel | null,
    value: string,
    afterSet?: () => void,
  ): boolean {
    if (!target) return false;
    suppressChange = true;
    target.setValue(value);
    afterSet?.();
    queueMicrotask(() => {
      suppressChange = false;
    });
    return true;
  }

  function setEditorValue(value: string): boolean {
    if (!model || model.getValue() === value) return false;
    if (!fullEditController.isImportActive()) return false;
    return setModelValueSilently(model, value, () => {
      updateSidecarSourceText(value);
    });
  }

  async function ensureEditor(): Promise<void> {
    if (editor || !container) return;
    const existingText = editorStore.get().workspace.tabsById[tabId]?.sourceText ?? '';
    ensureWorkspaceSidecarTab(existingText);
    void getSharedWasmWorkerClient().catch(() => {});
    const token = ++runtimeToken;
    const freshness = createFreshnessScope({ token }, () => ({ token: runtimeToken }));
    const runtime = await freshness.step(() =>
      getSharedMonacoRuntime({
        callWasmWorker: callSharedWasmWorker,
        getTokenTypes: () => callSharedWasmWorker<readonly string[]>('semanticTokensLegend'),
        isImportActive: () => fullEditController.isImportActive(),
      }),
    );
    if (!runtime || !freshness.isCurrent() || editor || !container) return;
    monaco = runtime.monaco as typeof import('monaco-editor');
    ensureSemanticTokensProvider = runtime.ensureSemanticTokensProvider;
    ensureDocumentColorProvider = runtime.ensureDocumentColorProvider;
    primeSemanticTokensForDocument = runtime.primeSemanticTokens;
    refreshSemanticTokensForLanguage = runtime.refreshSemanticTokens;
    clearSemanticTokensForDocument = runtime.clearSemanticTokens;
    ensureSemanticTokensProvider(activeLanguage);
    ensureDocumentColorProvider(activeLanguage);

    const tab = editorStore.get().workspace.tabsById[tabId];
    const uri = monaco.Uri.parse(`inmemory://sidecar/${tabId}`);
    model = monaco.editor.createModel(tab?.sourceText ?? '', activeLanguage, uri);
    syncLastModelSnapshot();
    setModelDocumentKey(model, tab?.documentKey ?? sidecarDocumentKey());
    editor = monaco.editor.create(container, {
      model,
      theme: 'tree-sitter-light',
      minimap: { enabled: false },
      automaticLayout: true,
      scrollbar: { alwaysConsumeMouseWheel: false },
      overviewRulerBorder: true,
      colorDecorators: true,
      colorDecoratorsActivatedOn: 'clickAndHover',
      'semanticHighlighting.enabled': true,
    });
    cleanupTestHook = attachMonacoTestHook(editor, 'right-editor', monaco.editor.tokenize);
    editor.onDidChangeModelContent((event) => {
      const activeModel = model;
      if (!activeModel) return;
      if (suppressChange) {
        syncLastModelSnapshot();
        return;
      }
      const previousLength = lastModelLength;
      const previousText = lastModelText;
      const nextText = activeModel.getValue();
      const changes = (event as unknown as { changes?: MonacoTextChange[] }).changes ?? [];
      if (fullEditController.isImportActive()) {
        if (fullEditController.isActiveSessionText(nextText)) {
          syncLastModelSnapshot();
          return;
        }
        fullEditController.cancelImportStream();
      }
      const documentKey = sidecarDocumentKey();
      const requestLanguage = activeLanguage;
      const wholeDocumentReplacement =
        changes.length === 1 && changes[0].rangeOffset === 0 && changes[0].rangeLength === previousLength
          ? changes[0]
          : null;
      lastModelLength = nextText.length;
      lastModelText = nextText;
      if (wholeDocumentReplacement) {
        updateSidecarSourceText(nextText, { clearSnapshot: true });
        void runFullEditForCurrentText('whole-document-replacement', nextText, requestLanguage);
        onContentChange();
        return;
      }
      updateSidecarSourceText(nextText, { clearSnapshot: false });
      const isFlush = (event as unknown as { isFlush?: boolean }).isFlush ?? false;
      if (documentKey && changes.length > 0 && !isFlush) {
        const documentTextEdits = monacoChangesToDocumentTextEdits(
          new TextEncoder().encode(previousText),
          new TextEncoder().encode(nextText),
          changes,
        );
        commitEditorTabTextChange({
          requestModel: activeModel,
          requestLanguage,
          requestDocumentKey: documentKey,
          nextText,
          documentTextEdits,
          baseSnapshotId: getActiveDocumentSnapshotId(documentKey),
          commitRevision: commitSidecarEditorState,
          settings: buildDocumentJobSettings({
            enableNest: $settings.parser.enableNest,
            formatting: $settings.formatting,
            formatSourceOnClose: false,
          }),
          builderConfig: buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
          isFresh: ({ revision }) => {
            const current = editorStore.get().workspace.tabsById[tabId];
            return (
              activeModel === model &&
              current?.documentKey === documentKey &&
              current.revision === revision &&
              current.sourceText === nextText &&
              activeLanguage === requestLanguage
            );
          },
          applyCommittedSourceText: (sourceTextValue) => {
            setModelValueSilently(activeModel, sourceTextValue, () => {
              updateSidecarSourceText(sourceTextValue, { clearSnapshot: false });
              syncLastModelSnapshot();
            });
          },
          bindSnapshot: fullEditSink.bindSnapshot,
          applyGraphAnalysis: applySidecarGraphAnalysis,
        });
      } else {
        commitSidecarEditorState();
      }
      onContentChange();
    });
    editor.onDidScrollChange((event) => {
      onScroll({ scrollTop: event.scrollTop, scrollLeft: event.scrollLeft });
    });
  }

  export async function ensureReady(): Promise<void> {
    await ensureEditor();
  }

  export function getMonaco(): typeof import('monaco-editor') | undefined {
    return monaco;
  }

  export function clearDiffPlan(): void {
    diffDecorations?.clear();
    diffDecorations = null;
    if (editor) {
      editor.changeViewZones((changeAccessor) => {
        for (const id of diffBlankZoneIds) {
          changeAccessor.removeZone(id);
        }
      });
    }
    diffBlankZoneIds = [];
  }

  export function applyDiffPlan(plan: DiffPlan): number {
    if (!editor) return 0;
    clearDiffPlan();
    const highlightCount = plan.decorations.length + plan.fillRanges.length;
    if (plan.decorations.length > 0) {
      diffDecorations = editor.createDecorationsCollection(plan.decorations);
    }
    if (plan.fillRanges.length > 0) {
      editor.changeViewZones((changeAccessor) => {
        diffBlankZoneIds = plan.fillRanges.map((range) =>
          changeAccessor.addZone({
            afterLineNumber: range.startLineNumber - 1,
            heightInLines: range.endLineNumber - range.startLineNumber + 1,
            domNode: (() => {
              const node = document.createElement('div');
              node.className = 'diff-blank-hunk';
              return node;
            })(),
          }),
        );
      });
    }
    if (plan.firstLine) {
      editor.revealLineInCenter(plan.firstLine);
    }
    return highlightCount;
  }

  export async function showText(
    value: string,
    nextLanguage: SupportedEditorLanguageId = language,
  ): Promise<void> {
    activeLanguage = nextLanguage;
    onContentChange();
    ensureWorkspaceSidecarTab(value);
    setWorkspaceSidecarLanguage(nextLanguage);
    updateSidecarSourceText(value);
    await tick();
    await ensureEditor();
    setModelLanguage(nextLanguage);
    await runFullEditForCurrentText('whole-document-replacement', value, nextLanguage);
  }

  async function runFullEditForCurrentText(
    reason: 'whole-document-replacement' | 'language-switch',
    text = model?.getValue() ?? sidecarTab?.sourceText ?? '',
    nextLanguage: SupportedEditorLanguageId = activeLanguage,
  ): Promise<void> {
    if (!text.trim()) return;
    await fullEditController.startFullEditSession({
      language: nextLanguage,
      text,
      reason,
      sourceWritebackPolicy: 'intake',
      documentKey: sidecarDocumentKey(),
      isFresh: () => editorStore.get().workspace.tabsById[tabId]?.sourceText === text,
    });
  }

  export function getText(): string {
    return model?.getValue() ?? editorStore.get().workspace.tabsById[tabId]?.sourceText ?? '';
  }

  export function getLanguage(): SupportedEditorLanguageId {
    return activeLanguage;
  }

  export function setScrollPosition(position: { scrollTop: number; scrollLeft: number }): void {
    editor?.setScrollPosition(position);
  }

  $: if (container) {
    void ensureEditor();
  }

  $: if (language !== lastPropLanguage) {
    lastPropLanguage = language;
    activeLanguage = language;
    sidecarAnalysisSyncToken += 1;
    setWorkspaceSidecarLanguage(activeLanguage);
    if (model && monaco) {
      setModelLanguage(activeLanguage);
      void runFullEditForCurrentText('language-switch', undefined, activeLanguage);
    }
  }

  $: if (model && monaco && activeLanguage && model.getLanguageId() !== activeLanguage) {
    setModelLanguage(activeLanguage);
  }

  $: if (model && sidecarTab && !suppressChange && sidecarTab.sourceText !== model.getValue()) {
    setModelValueSilently(model, sidecarTab.sourceText, () => {
      syncLastModelSnapshot();
    });
  }

  onDestroy(() => {
    runtimeToken += 1;
    sidecarAnalysisSyncToken += 1;
    clearDiffPlan();
    cleanupTestHook?.();
    cleanupTestHook = null;
    editor?.dispose();
    editor = null;
    model?.dispose();
    model = null;
    fullEditController.dispose();
  });
</script>

<div
  bind:this={container}
  class="min-h-0 min-w-0 flex-1 overflow-hidden"
  data-testid="right-text-editor-container"
></div>
