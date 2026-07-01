<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import type * as Monaco from 'monaco-editor';
  import type { DocumentTextEdit } from '@core-wasm/index';
  import { type TreeNode, TreeKind } from '@core-wasm/index';
  import type { DiffPlan } from '../../graph/diff-plan';
  import {
    sourceText,
    compareEditToken,
    editorRevision,
    graphAppliedRevision,
    editorIO,
    editorMutation,
    jsonBlockSelection,
    documentKey as documentKeyStore,
    languageId as languageIdStore,
    activeTempModel,
    editorStore,
    treeState,
    fullEditUiState,
    type EditorMutation,
    type GraphHighlightTarget,
    type JsonBlockSelection,
    type PathSeg,
  } from '../../store/editor-store';
  import { getLanguageExample } from '../../monaco/language-examples';
  import {
    editorLanguageFallback,
    supportedEditorLanguageSet,
    type SupportedEditorLanguageId,
  } from '../../monaco/language-support';
  import { guessLanguage } from '../../guess/guess-language';
  import { settings } from '../../settings/settings-store';
  import { buildDocumentJobSettings } from '../../graph-stream/document-job-runner';
  import { buildGraphStreamBuilderConfig } from '../../graph-stream/graph-stream-builder-config';
  import { createFreshnessScope } from '../../guards/freshness-scope';
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../wasm/wasm-worker-singleton';
  import { attachMonacoTestHook } from '../../monaco/test-hook';
  import { toast } from 'svelte-sonner';
  import { resolvePathSelectionRangeSafe } from '../../services/TreePathService';
  import { bindActiveDocumentSnapshotIfPresent, getActiveDocumentSnapshotId } from '../../services/DocumentSessionService';
  import { resolveEditorRuntimeOverlay, type RuntimeStateEventDetail } from '../../runtime-loading';
  import {
    markCursorPathRequested,
    markCursorPathSettled,
  } from '../../test-bridge/runtime-readiness';
  import { awaitEditorRuntimeStartupDelay } from '../../test-bridge/runtime-startup-delay';

  import TabManager from './TabManager.svelte';
  import EditorDropZone from './EditorDropZone.svelte';
  import { shouldSyncGraphHighlightFromCursorReason } from './EditorCore.graph-highlight';
  import { ensureModelDocumentKey } from './document-key';
  import { registerEditorHoverPreview } from './editor-hover';
  import { createEditorAnalysisController } from './editor-analysis-controller';
  import { createEditorFormatController } from './editor-format-controller';
  import { createEditorFullEditController } from './editor-full-edit-controller';
  import { resolveLanguageSwitchPolicy } from './language-switch-policy';
  import { createEditorRuntimeController } from './editor-runtime-controller';
  import { buildRootScalarHighlightDecorations, resolveRootScalarHighlightKind } from './root-scalar-highlight';
  import { commitEditorTabTextChange } from './editor-tab-edit-commit';
  import { settleWholeDocumentReplacement } from './whole-document-replacement';
  import type { EditorModelWithDocumentKey, EditorTab, TabSummary } from './types';
  import { EDITOR_CONFIG } from '../../config/constants';
  import { monacoChangesToDocumentTextEdits, type MonacoTextChange } from '../../../shared/document-text-edits';

  export let tabSummaries: TabSummary[] = [];
  export let activeTabId = '';
  export let onScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {};
  export let enableRevealSync = true;
  export let synchronizedRuntimeLoading = false;

  type QueuedWholeDocumentReplacement = {
    text: string;
    sourceWritebackPolicy?: 'intake' | 'submitted';
    formatSourceOnClose?: boolean;
    shouldResolveLanguage?: boolean;
    markUserInput?: boolean;
  };

  const dispatch = createEventDispatcher<{ 'runtime-state': RuntimeStateEventDetail }>();

  let monaco: typeof import('monaco-editor');
  let editor: Monaco.editor.IStandaloneCodeEditor | null = null;
  let model: Monaco.editor.ITextModel | null = null;
  let isStoreUpdateSuppressed = false;
  let lastModelLength = 0;
  let lastModelText = '';
  let hoverPreviewDisposable: Monaco.IDisposable | null = null;
  let cleanupSourceEditorTestHook: (() => void) | null = null;
  let storeUnsub: (() => void) | null = null;
  let languageUnsub: (() => void) | null = null;
  let tempModelUnsub: (() => void) | null = null;
  let jsonBlockSelectionUnsub: (() => void) | null = null;

  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback;
  let shouldSuppressLanguageExample = false;
  let lastMutationId = 0;
  let lastExternalTreeSelection: { path: PathSeg[]; target?: GraphHighlightTarget; source: string } | null = null;
  let diffDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let jsonBlockDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let rootScalarDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let diffBlankZoneIds: string[] = [];
  let suppressGraphHighlightSync = 0;
  let suppressTreePathUpdate = 0;
  let wholeDocumentReplacementToken = 0;
  let formattingOptionsValue;
  let suppressNextWholeDocumentAutoGuess = false;
  let queuedWholeDocumentReplacement: QueuedWholeDocumentReplacement | null = null;
  let editorRevisionValue = 0;
  const userInputByTabId = new Map<string, boolean>();
  let refreshSemanticTokensForLanguage: (languageId?: string) => void = () => {};
  let primeSemanticTokensForDocument: (documentKey: string, semanticTokens: ArrayBuffer) => void = () => {};
  let clearSemanticTokensForDocument: (documentKey?: string) => void = () => {};
  let updateDocumentColorViewport: (model: Monaco.editor.ITextModel, visibleRanges: Monaco.Range[]) => void = () => {};
  let refreshVisibleDocumentColors: (model: Monaco.editor.ITextModel) => void = () => {};
  let colorViewportRefreshHandle: ReturnType<typeof setTimeout> | null = null;
  let editorRuntimeReady = false;
  let editorRuntimeError = false;
  let editorRuntimePhase = 'Loading editor runtime...';
  let editorRuntimeToken = 0;
  let lastEditorRuntimeStateSignature = '';
  let jsonBlockSelectionValue: JsonBlockSelection | null = null;
  let editorRuntimeOverlay = resolveEditorRuntimeOverlay({
    editorRuntimeReady: false,
    editorRuntimePhase,
    synchronizedRuntimeLoading,
  });

  function getModelDocumentKey(target: Monaco.editor.ITextModel | null): string {
    return ((target as EditorModelWithDocumentKey | null)?.__treeaseDocumentKey ?? '').trim();
  }

  function setModelDocumentKey(target: Monaco.editor.ITextModel | null, documentKey: string): void {
    if (!target || !documentKey) return;
    (target as EditorModelWithDocumentKey).__treeaseDocumentKey = documentKey;
  }

  function syncLastModelSnapshot(): string {
    const text = model?.getValue() ?? '';
    lastModelLength = text.length;
    lastModelText = text;
    return text;
  }

  function setLanguageIdWithoutExample(nextLanguage: SupportedEditorLanguageId): void {
    shouldSuppressLanguageExample = true;
    languageIdStore.set(nextLanguage);
    shouldSuppressLanguageExample = false;
  }

  function releaseStoreUpdateSuppression(): void {
    queueMicrotask(() => {
      isStoreUpdateSuppressed = false;
    });
  }

  function clearDocumentSemanticState(documentKey: string): void {
    clearSemanticTokensForDocument(documentKey);
    refreshSemanticTokensForLanguage(languageIdValue);
  }

  function applyRootScalarHighlight(
    analysis: import('./editor-analysis-apply').EditorAnalysisLike | null | undefined,
  ): void {
    if (!editor) return;
    rootScalarDecorations ??= editor.createDecorationsCollection();
    const highlightKind = jsonBlockSelectionValue ? null : resolveRootScalarHighlightKind(analysis);
    rootScalarDecorations.set(
      buildRootScalarHighlightDecorations(monaco, model, highlightKind),
    );
  }

  function setActiveEditorIo(): void {
    editorIO.set({
      context: 'editor',
      getModel: () => model,
      getText: () => model?.getValue() ?? '',
      setText: (value: string) => setEditorValue(value),
      applyTextEdits: (edits: DocumentTextEdit[]) => applyDocumentTextEdits(edits),
      getLanguage: () => languageIdValue,
    });
  }

  function suppressNextGraphHighlightSync(): void {
    suppressGraphHighlightSync += 1;
    queueMicrotask(() => {
      suppressGraphHighlightSync = Math.max(0, suppressGraphHighlightSync - 1);
    });
  }

  function suppressNextTreePathUpdate(): void {
    suppressTreePathUpdate += 1;
    queueMicrotask(() => {
      suppressTreePathUpdate = Math.max(0, suppressTreePathUpdate - 1);
    });
  }

  const getNestEnabled = () => $settings.parser.enableNest;
  const updateCurrentTempModel = (updater: (current: any) => any) => activeTempModel.update(updater);
  const clearDocumentSemanticTokens = (documentKey?: string) => clearSemanticTokensForDocument(documentKey);
  const refreshDocumentSemanticTokens = (languageId?: string) => refreshSemanticTokensForLanguage(languageId);
  const callWasmWorkerFromEditor = <T>(method: string, input: unknown) =>
    callSharedWasmWorker<T>(method as any, input);

  function rotateActiveDocumentKey(): string {
    const activeId = tabManager?.getActiveTabId();
    if (activeId) {
      const rotated = tabManager.rotateDocumentKey(activeId);
      if (rotated) {
        documentKeyStore.set(rotated);
        return rotated;
      }
    }
    const fallback = ensureModelDocumentKey(model);
    documentKeyStore.set(fallback);
    return fallback;
  }

  let fullEditUiStateValue = $fullEditUiState;
  $: fullEditUiStateValue = $fullEditUiState;

  const treePathLanguages = supportedEditorLanguageSet;
  const themeName = 'tree-sitter-light';
  const maxTabs = EDITOR_CONFIG.maxTabs;
  const initialCode = getLanguageExample('json');
  $: rootScalarStyle = [
    `--treease-root-scalar-str:${$settings.editor.semanticTypeColors.str}`,
    `--treease-root-scalar-int:${$settings.editor.semanticTypeColors.int}`,
    `--treease-root-scalar-float:${$settings.editor.semanticTypeColors.float}`,
    `--treease-root-scalar-boolean:${$settings.editor.semanticTypeColors.boolean}`,
    `--treease-root-scalar-nil:${$settings.editor.semanticTypeColors.nil}`,
  ].join(';');

  let tabManager: TabManager;
  let dropZone: EditorDropZone;

  function activeTabHasUserInput(language: SupportedEditorLanguageId): boolean {
    const manager = tabManager as TabManager | undefined;
    const activeId = manager?.getActiveTabId();
    if (!activeId) return false;
    const recorded = userInputByTabId.get(activeId);
    if (recorded !== undefined) return recorded;
    return (model?.getValue() ?? '') !== getLanguageExample(language);
  }

  function markActiveTabUserInput(value: boolean): void {
    const manager = tabManager as TabManager | undefined;
    const activeId = manager?.getActiveTabId();
    if (activeId) userInputByTabId.set(activeId, value);
  }

  function guardImportInProgress(): boolean {
    if (!editorFullEditController.isImportActive()) return false;
    toast.info('Import in progress');
    return true;
  }

  const editorAnalysisController = createEditorAnalysisController({
    getMonaco: () => monaco,
    getEditor: () => editor,
    getModel: () => model,
    getDocumentKey,
    getLanguageId: () => languageIdValue,
    getNestEnabled,
    getEditorRevision: () => $editorRevision,
    isImportActive: () => editorFullEditController.isImportActive(),
    getSourceText: () => model?.getValue() ?? '',
    getJsonBlockSelection: () => jsonBlockSelectionValue,
    setJsonBlockSelection: (selection) => jsonBlockSelection.set(selection),
    updateActiveTempModel: updateCurrentTempModel,
    setTreeState: (value) => treeState.set(value),
    applyRootScalarHighlight,
    primeSemanticTokensForDocument: (documentKey, semanticTokens) =>
      primeSemanticTokensForDocument(documentKey, semanticTokens),
    clearSemanticTokensForDocument: clearDocumentSemanticTokens,
    refreshSemanticTokensForLanguage: refreshDocumentSemanticTokens,
    markCursorPathRequested,
    markCursorPathSettled,
  });

  const editorFullEditController = createEditorFullEditController({
    getModel: () => model,
    getEditor: () => editor,
    getMonaco: () => monaco,
    getLanguageId: () => languageIdValue,
    getNestEnabled,
    getGraphBuilderConfig: () => buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
    getFullEditUiState: () => fullEditUiStateValue,
    getFormattingOptions: () => formattingOptionsValue,
    callWasmWorker: callWasmWorkerFromEditor,
    rotateActiveDocumentKey,
    setModelDocumentKey,
    setActiveTabDocumentKey: (documentKey) => {
      const activeId = tabManager?.getActiveTabId();
      if (activeId) tabManager.setTabDocumentKey(activeId, documentKey);
    },
    clearSemanticTokensForDocument: clearDocumentSemanticTokens,
    setEditorValue,
    setSourceText: (value) => sourceText.set(value),
    setDocumentKey: (documentKey) => documentKeyStore.set(documentKey),
    applyImportLanguage: setLanguageIdWithoutExample,
    updateActiveTempModel: updateCurrentTempModel,
    commitEditorState,
    applyGraphAnalysis: (requestModel, requestLanguage, requestDocumentKey, revision, analysis) =>
      editorAnalysisController.applyGraphAnalysis(
        requestModel,
        requestLanguage,
        requestDocumentKey,
        revision,
        analysis,
      ),
    triggerGraphSync: (position) => {
      if (!position) return;
      void editorAnalysisController.updateTreePath(position, { syncGraphHighlight: true });
    },
  });

  const editorFormatController = createEditorFormatController({
    getModel: () => model,
    getLanguageId: () => languageIdValue,
    getFormattingOptions: () => formattingOptionsValue,
    getNestEnabled,
    isImportActive: () => editorFullEditController.isImportActive(),
    callWasmWorker: callWasmWorkerFromEditor,
    replaceWholeDocumentText: (value, kind) =>
      queueWholeDocumentReplacement(value, {
        sourceWritebackPolicy: 'intake',
        formatSourceOnClose: kind === 'sort',
        shouldResolveLanguage: false,
        markUserInput: true,
      }),
    resetEditorCursorToStart,
  });

  const editorRuntimeController = createEditorRuntimeController({
    getSettings: () => $settings,
    getThemeName: () => themeName,
    isImportActive: () => editorFullEditController.isImportActive(),
    callWasmWorker: callWasmWorkerFromEditor,
    getWorkerClient: () => getSharedWasmWorkerClient(),
    setMonaco: (value) => {
      monaco = value;
    },
  });

  const treeKinds = new Set<TreeKind>([TreeKind.SCALAR, TreeKind.SEQUENCE, TreeKind.MAPPING, TreeKind.ALIAS]);

  function isTreeNodeLike(value: unknown): value is TreeNode {
    if (!value || typeof value !== 'object') return false;
    const node = value as { kind?: unknown; semType?: unknown; children?: unknown };
    return (
      typeof node.kind === 'number' &&
      treeKinds.has(node.kind as TreeKind) &&
      typeof node.semType === 'number' &&
      Array.isArray(node.children)
    );
  }

  $: formattingOptionsValue = $settings.formatting;
  $: editorRevisionValue = $editorRevision;
  $: editorRuntimeOverlay = resolveEditorRuntimeOverlay({
    editorRuntimeReady,
    editorRuntimePhase,
    synchronizedRuntimeLoading,
  });
  $: {
    const runtimeState: RuntimeStateEventDetail = {
      ready: editorRuntimeReady,
      loading: !editorRuntimeReady && !editorRuntimeError,
      error: editorRuntimeError,
      phase: editorRuntimePhase,
    };
    const nextSignature = `${runtimeState.ready ? 'ready' : 'not-ready'}|${runtimeState.loading ? 'loading' : 'settled'}|${
      runtimeState.error ? 'error' : 'ok'
    }|${runtimeState.phase ?? ''}`;
    if (nextSignature !== lastEditorRuntimeStateSignature) {
      lastEditorRuntimeStateSignature = nextSignature;
      dispatch('runtime-state', runtimeState);
    }
  }

  $: if ($editorMutation && $editorMutation.id !== lastMutationId) {
    lastMutationId = $editorMutation.id;
    void applyEditorMutation($editorMutation.mutation);
  }

  $: {
    const graphHighlight = $activeTempModel?.graphHighlight ?? null;
    const graphRevealSyncBlocked =
      !enableRevealSync && (graphHighlight?.source === 'graph' || graphHighlight?.source === 'search');
    if (!graphHighlight?.path?.length || graphHighlight.source === 'editor' || graphRevealSyncBlocked) {
      lastExternalTreeSelection = null;
    } else if (graphHighlight !== lastExternalTreeSelection && editor && model) {
      lastExternalTreeSelection = graphHighlight;
      void revealPath(graphHighlight.path, {
        target: graphHighlight.target,
        focus: false,
      }).catch(() => {
        // revealPath already reports the failure via toast + temp model state.
      });
    }
  }

  function setupLanguageSubscription(
    ensureSemanticTokensProvider: (lang: string) => void,
    ensureDocumentColorProvider: (lang: string) => void,
  ) {
    languageUnsub = languageIdStore.subscribe((value) => {
      const nextValue = value || editorLanguageFallback;
      if (!supportedEditorLanguageSet.has(nextValue as SupportedEditorLanguageId)) {
        const message = `Unsupported editor language: ${nextValue}`;
        activeTempModel.update((current) => ({ ...current, error: message }));
        throw new Error(message);
      }
      const previousLanguage = languageIdValue;
      const next = nextValue as SupportedEditorLanguageId;
      const languageChanged = next !== previousLanguage;
      const isManualLanguageSwitch = languageChanged && !shouldSuppressLanguageExample;
      const hadUserInput = activeTabHasUserInput(previousLanguage);
      languageIdValue = next;
      activeTempModel.update((current) => ({ ...current, error: '' }));
      ensureLanguageRegistered(next);
      let shouldDeferTreePath = false;
      if (model && monaco) {
        const languageSwitchSourceText = model.getValue();
        const languageSwitchPolicy = isManualLanguageSwitch
          ? resolveLanguageSwitchPolicy({
              nextLanguage: next,
              hasUserInput: hadUserInput,
              currentText: languageSwitchSourceText,
              nextExampleText: getLanguageExample(next),
            })
          : null;
      shouldDeferTreePath = Boolean(languageSwitchPolicy);
      if (isManualLanguageSwitch) {
        suppressNextWholeDocumentAutoGuess = true;
        wholeDocumentReplacementToken += 1;
        editorAnalysisController.prepareLanguageSwitchAnalysisReset();
      }
        monaco.editor.setModelLanguage(model, next);
        syncColorViewportState('language');
        if (languageSwitchPolicy) {
          const requestModel = model;
          lastModelLength = languageSwitchPolicy.text.length;
          lastModelText = languageSwitchPolicy.text;
          markActiveTabUserInput(languageSwitchPolicy.kind === 'preserve-input');
          void editorFullEditController.startFullEditSession({
            language: languageSwitchPolicy.language,
            text: languageSwitchPolicy.text,
            reason: languageSwitchPolicy.reason,
            isFresh: () => model === requestModel,
          });
        }
      }
      ensureSemanticTokensProvider(next);
      ensureDocumentColorProvider(next);
      const activeId = tabManager?.getActiveTabId();
      if (activeId) {
        tabManager?.updateTabLanguage(activeId, next);
      }
      if (next !== 'json') {
        jsonBlockSelection.set(null);
      }
      if (!treePathLanguages.has(next)) {
        activeTempModel.update((current) => ({ ...current, treePath: [], graphHighlight: null }));
      } else if (activeId && model && !shouldDeferTreePath) {
        void editorAnalysisController.updateTreePath(editor?.getPosition() ?? null, { syncGraphHighlight: false });
      }
    });
  }


  function initFirstTab() {
    const firstTab = tabManager.initTabs();
    model = firstTab.model;
    lastModelLength = model.getValue().length;
    lastModelText = model.getValue();
    const container = dropZone.getContainer();
    editor = monaco.editor.create(container, {
      model,
      theme: themeName,
      minimap: { enabled: false },
      automaticLayout: true,
      scrollbar: { alwaysConsumeMouseWheel: false },
      overviewRulerBorder: true,
      colorDecorators: true,
      colorDecoratorsActivatedOn: 'clickAndHover',
      'semanticHighlighting.enabled': true,
    });
    cleanupSourceEditorTestHook = attachMonacoTestHook(
      {
        getDomNode: () => editor?.getDomNode() ?? null,
        getValue: () => editor?.getValue() ?? '',
        setValue: (value: string) => {
          editor?.setValue(value);
        },
        setValueForTestHook: (value: string) => {
          queueWholeDocumentReplacement(value, {
            sourceWritebackPolicy: 'submitted',
            formatSourceOnClose: false,
            shouldResolveLanguage: false,
            markUserInput: true,
          });
        },
        focus: () => editor?.focus(),
        setPosition: (position) => editor?.setPosition(position),
        revealPositionInCenter: (position) => editor?.revealPositionInCenter(position),
        getScrollTop: () => editor?.getScrollTop() ?? 0,
        getScrollLeft: () => editor?.getScrollLeft() ?? 0,
        setScrollPosition: (position) => editor?.setScrollPosition(position),
        executeEdits: (source, edits) =>
          editor?.executeEdits(source, edits as Monaco.editor.IIdentifiedSingleEditOperation[]),
        onDidChangeModel: (listener) => editor?.onDidChangeModel(listener) ?? { dispose: () => {} },
        getModel: () => editor?.getModel() ?? null,
      },
      'source-editor',
      monaco.editor.tokenize,
    );
    rootScalarDecorations = editor.createDecorationsCollection();
    ensureLanguageRegistered(languageIdValue);
    monaco.editor.setModelLanguage(model, languageIdValue);
    editorStore.actions.initWorkspaceFromPrimaryTab({ id: firstTab.id, name: firstTab.name });
    setActiveTab(firstTab, 'initial-example');
    syncColorViewportState('init');
  }

  function clearColorViewportRefresh(): void {
    if (colorViewportRefreshHandle == null) return;
    clearTimeout(colorViewportRefreshHandle);
    colorViewportRefreshHandle = null;
  }

  function syncColorViewportState(_reason: 'init' | 'scroll' | 'content' | 'language' | 'model'): void {
    if (!editor || !model) return;
    const activeModel = model;
    updateDocumentColorViewport(activeModel, editor.getVisibleRanges());
    if (editorFullEditController.isImportActive()) return;
    clearColorViewportRefresh();
    colorViewportRefreshHandle = setTimeout(() => {
      colorViewportRefreshHandle = null;
      if (model !== activeModel) return;
      refreshVisibleDocumentColors(activeModel);
    }, 200);
  }

  function bindEditorEvents() {
    if (!editor || !model) return;
    editor.onDidFocusEditorWidget(() => {
      if (!model) return;
      setActiveEditorIo();
    });

    editor.onDidChangeModelContent((event) => {
      const activeModel = model;
      if (!activeModel) return;
      const previousLength = lastModelLength;
      const previousText = lastModelText;
      const nextText = activeModel.getValue();
      applyRootScalarHighlight(null);
      syncColorViewportState('content');
      const changes = (event as unknown as { changes?: MonacoTextChange[] }).changes ?? [];
      isStoreUpdateSuppressed = true;
      notifyCompareEdit();
      if (editorFullEditController.isImportActive()) {
        if (editorFullEditController.isActiveSessionText(nextText)) {
          syncLastModelSnapshot();
          releaseStoreUpdateSuppression();
          return;
        }
        editorFullEditController.cancelImportStream();
      }
      if (diffDecorations || diffBlankZoneIds.length > 0) {
        clearDiffPlan();
      }
      const previousDocumentKey = getDocumentKey();
      if (previousDocumentKey) jsonBlockSelection.set(null);
      if (previousDocumentKey) {
        clearDocumentSemanticState(previousDocumentKey);
      }
      const shouldSkipWholeDocumentAutoGuess = suppressNextWholeDocumentAutoGuess;
      suppressNextWholeDocumentAutoGuess = false;
      const wholeDocumentReplacement =
        changes.length === 1 && changes[0].rangeOffset === 0 && changes[0].rangeLength === previousLength
          ? changes[0]
          : null;
      lastModelLength = nextText.length;
      lastModelText = nextText;
      const shouldRotateDocumentKey = Boolean(wholeDocumentReplacement);
      if (shouldRotateDocumentKey) {
        rotateActiveDocumentKey();
        if (editorFullEditController.suppressNextWholeDocumentIntake()) {
          releaseStoreUpdateSuppression();
          return;
        }
        const queuedReplacement =
          queuedWholeDocumentReplacement && queuedWholeDocumentReplacement.text === nextText
            ? queuedWholeDocumentReplacement
            : null;
        queuedWholeDocumentReplacement = null;
        activeTempModel.update((current) => ({
          ...current,
          treePath: [],
          graphHighlight: null,
        }));
        sourceText.set(nextText);
        const documentKeyValue = getDocumentKey();
        const requestModel = activeModel;
        const currentLanguage = languageIdValue;
        const preparedRevision = editorRevisionValue;
        const replacementToken = ++wholeDocumentReplacementToken;
        const sourceWritebackPolicy = queuedReplacement?.sourceWritebackPolicy ?? 'intake';
        const formatSourceOnClose = queuedReplacement?.formatSourceOnClose ?? true;
        const shouldResolveLanguage =
          queuedReplacement?.shouldResolveLanguage ??
          (!shouldSkipWholeDocumentAutoGuess && wholeDocumentReplacement.text.trim().length >= 8);
        const shouldMarkUserInput = queuedReplacement?.markUserInput ?? true;
        editorStore.actions.prepareFullEditStream({
          documentKey: documentKeyValue,
          revision: preparedRevision,
          language: currentLanguage,
          transportKind: 'memory',
          reason: 'whole-document-replacement',
        });
        const replacementFreshness = createFreshnessScope(
          {
            token: replacementToken,
            model: requestModel,
            documentKey: documentKeyValue,
          },
          () => ({
            token: wholeDocumentReplacementToken,
            model,
            documentKey: getDocumentKey(),
          }),
        );
        const isReplacementCurrent = replacementFreshness.isCurrent;
        void settleWholeDocumentReplacement({
          text: nextText,
          currentLanguage,
          shouldResolveLanguage,
          resolveLanguage: resolveWholeDocumentReplacementLanguage,
          onResolveLanguageError: (error) => {
            console.error('[editor] whole-document language detection failed', error);
          },
          isStillCurrent: isReplacementCurrent,
          onDetectedLanguage: (language) => {
            if (language !== currentLanguage) toast.success(`Detected ${language.toUpperCase()} input`);
          },
          commitWholeDocumentReplacement: async (language) => {
            if (shouldMarkUserInput) markActiveTabUserInput(true);
            await editorFullEditController.startFullEditSession({
              language,
              text: nextText,
              reason: 'whole-document-replacement',
              sourceWritebackPolicy,
              formatSourceOnClose,
              documentKey: documentKeyValue,
              isFresh: isReplacementCurrent,
            });
          },
        })
          .catch((error) => {
            if (!isReplacementCurrent()) return;
            const message = error instanceof Error ? error.message : String(error);
            console.error('[editor] whole-document replacement failed', error);
            activeTempModel.update((current) => ({ ...current, error: message }));
            toast.error('Graph rebuild failed');
          })
          .finally(() => {
            editorStore.actions.cancelPreparedFullEditStream({
              documentKey: documentKeyValue,
              revision: preparedRevision,
              reason: 'whole-document-replacement',
            });
          });
        releaseStoreUpdateSuppression();
        return;
      }
      sourceText.set(nextText);
      const documentKeyValue = getDocumentKey();
      const isFlush = (event as unknown as { isFlush?: boolean }).isFlush ?? false;
      if (documentKeyValue && changes.length > 0 && !isFlush) {
        const documentTextEdits = monacoChangesToDocumentTextEdits(
          new TextEncoder().encode(previousText),
          new TextEncoder().encode(nextText),
          changes,
        );
        markActiveTabUserInput(true);
        commitDocumentChanges(activeModel, languageIdValue, documentKeyValue, nextText, documentTextEdits);
      } else {
        commitEditorState();
      }
      releaseStoreUpdateSuppression();
    });

    const updateCursorStatus = (
      position: Monaco.IPosition | null,
      selection: Monaco.Selection | null,
      syncGraphHighlight: boolean,
    ) => {
      if (!model || !position) return;
      const selectionLength = selection ? model.getValueInRange(selection).length : 0;
      activeTempModel.update((current) => ({
        ...current,
        cursor: `Ln ${position.lineNumber}, Col ${position.column}`,
        selectionLength,
      }));
      if (suppressTreePathUpdate > 0) return;
      void editorAnalysisController.updateTreePath(position, { syncGraphHighlight });
    };

    editor.onDidChangeCursorPosition((event) => {
      const isFocused = typeof editor?.hasTextFocus === 'function' ? editor.hasTextFocus() : true;
      updateCursorStatus(
        event.position,
        editor?.getSelection() ?? null,
        suppressGraphHighlightSync === 0 &&
          (shouldSyncGraphHighlightFromCursorReason(event.reason) || (event.reason === 0 && isFocused)),
      );
    });
    editor.onDidChangeCursorSelection((event) => {
      const position = editor?.getPosition() ?? event.selection.getPosition();
      const isFocused = typeof editor?.hasTextFocus === 'function' ? editor.hasTextFocus() : true;
      updateCursorStatus(
        position,
        event.selection,
        suppressGraphHighlightSync === 0 &&
          (shouldSyncGraphHighlightFromCursorReason(event.reason) || (event.reason === 0 && isFocused)),
      );
    });
    editor.onDidScrollChange((event) => {
      onScroll({ scrollTop: event.scrollTop, scrollLeft: event.scrollLeft });
      syncColorViewportState('scroll');
    });
  }

  function bindStoreSubscriptions() {
    storeUnsub = sourceText.subscribe((value) => {
      if (!model || isStoreUpdateSuppressed) return;
      if (editorStore.get().fullEditUiState.active) return;
      if (value !== model.getValue()) {
        model.setValue(value);
      }
    });
    tempModelUnsub = activeTempModel.subscribe((value) => {
      const activeId = tabManager?.getActiveTabId();
      if (!activeId) return;
      const current = tabManager?.getTempModel(activeId);
      if (!current || current !== value) {
        tabManager?.setTempModel(activeId, value);
      }
    });
    jsonBlockSelectionUnsub = jsonBlockSelection.subscribe((value) => {
      jsonBlockSelectionValue = value;
      applyJsonBlockDecoration(value);
      if (value) {
        applyRootScalarHighlight(null);
      }
    });
  }

  function commitEditorState(): number {
    if (!model) return 0;
    const documentKeyValue = getDocumentKey();
    if (documentKeyValue) documentKeyStore.set(documentKeyValue);
    let nextRevision = 0;
    editorRevision.update((value) => {
      nextRevision = value + 1;
      return nextRevision;
    });
    return nextRevision;
  }

  function commitDocumentChanges(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    nextText: string,
    documentTextEdits: DocumentTextEdit[],
  ): number {
    return commitEditorTabTextChange({
      requestModel,
      requestLanguage,
      requestDocumentKey,
      nextText,
      documentTextEdits,
      baseSnapshotId: getActiveDocumentSnapshotId(requestDocumentKey),
      commitRevision: commitEditorState,
      settings: buildDocumentJobSettings({
        enableNest: $settings.parser.enableNest,
        formatting: $settings.formatting,
        formatSourceOnClose: false,
      }),
      builderConfig: buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
      isFresh: () =>
        requestModel === model &&
        requestDocumentKey === getDocumentKey() &&
        requestLanguage === languageIdValue,
      applyCommittedSourceText: (sourceTextValue) => requestModel.setValue(sourceTextValue),
      bindSnapshot: bindActiveDocumentSnapshotIfPresent,
      applyGraphAnalysis: (modelValue, languageValue, documentKeyValue, revisionValue, analysis) =>
        editorAnalysisController.applyGraphAnalysis(
          modelValue,
          languageValue,
          documentKeyValue,
          revisionValue,
          analysis,
        ),
    });
  }

  function ensureLanguageRegistered(languageId: string) {
    if (!monaco) return;
    const registered = monaco.languages.getLanguages().some((lang) => lang.id === languageId);
    if (!registered) {
      monaco.languages.register({ id: languageId });
    }
  }

  function notifyCompareEdit(): void {
    compareEditToken.update((value) => value + 1);
  }

  function setEditorValue(value: string): boolean {
    if (!model) return false;
    const previousValue = model.getValue();
    if (value === previousValue) {
      return false;
    }
    if (editor) {
      editor.setValue(value);
    } else {
      model.setValue(value);
    }
    return true;
  }

  function queueWholeDocumentReplacement(
    value: string,
    options: Omit<QueuedWholeDocumentReplacement, 'text'> = {},
  ): boolean {
    queuedWholeDocumentReplacement = {
      text: value,
      ...options,
    };
    const changed = setEditorValue(value);
    if (!changed) {
      queuedWholeDocumentReplacement = null;
    }
    return changed;
  }

  function clearJsonBlockDecoration(): void {
    jsonBlockDecorations?.clear();
    jsonBlockDecorations = null;
  }

  function applyJsonBlockDecoration(selection: JsonBlockSelection | null): void {
    clearJsonBlockDecoration();
    if (!editor || !model || !monaco || !selection) return;
    if (selection.sourceDocumentKey !== getDocumentKey()) return;
    if (selection.revision !== editorRevisionValue) return;
    jsonBlockDecorations = editor.createDecorationsCollection([
      {
        range: new monaco.Range(
          selection.startLineNumber,
          selection.startColumn,
          selection.endLineNumber,
          selection.endColumn,
        ),
        options: {
          inlineClassName: 'treease-json-block-highlight',
          isWholeLine: false,
        },
      },
    ]);
  }

  function utf8ByteLengthForCodePoint(codePoint: number): number {
    if (codePoint <= 0x7f) return 1;
    if (codePoint <= 0x7ff) return 2;
    if (codePoint <= 0xffff) return 3;
    return 4;
  }

  function utf16OffsetForUtf8ByteOffset(text: string, byteOffset: number): number {
    if (byteOffset <= 0) return 0;
    let bytes = 0;
    let offset = 0;
    while (offset < text.length) {
      if (bytes >= byteOffset) return offset;
      const codePoint = text.codePointAt(offset);
      if (codePoint == null) return offset;
      const codeUnitLength = codePoint > 0xffff ? 2 : 1;
      const nextBytes = bytes + utf8ByteLengthForCodePoint(codePoint);
      if (nextBytes > byteOffset) return offset;
      bytes = nextBytes;
      offset += codeUnitLength;
    }
    return text.length;
  }

  function applyDocumentTextEdits(edits: DocumentTextEdit[]): boolean {
    if (!editor || !model || !monaco || edits.length === 0) return false;
    const activeModel = model;
    const activeText = activeModel.getValue();
    const operations = edits.map((edit) => {
      const start = activeModel.getPositionAt(utf16OffsetForUtf8ByteOffset(activeText, edit.startByte));
      const end = activeModel.getPositionAt(utf16OffsetForUtf8ByteOffset(activeText, edit.oldEndByte));
      return {
        range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column),
        text: edit.text,
        forceMoveMarkers: true,
      };
    });
    const applied = editor.executeEdits('graph-value-edit', operations);
    return applied;
  }

  function resetEditorCursorToStart(): void {
    if (!editor || !monaco) return;
    suppressNextGraphHighlightSync();
    editor.setPosition({ lineNumber: 1, column: 1 });
    editor.setSelection(new monaco.Selection(1, 1, 1, 1));
    editor.setScrollPosition({ scrollTop: 0, scrollLeft: 0 });
  }

  function workspacePayloadForTab(tab: EditorTab) {
    const workspaceTab = editorStore.get().workspace.tabsById[tab.id];
    const isActiveEditorTab = tabManager?.getActiveTabId() === tab.id;
    const snapshotId = workspaceTab
      ? workspaceTab.snapshotId
      : isActiveEditorTab
        ? getActiveDocumentSnapshotId(tab.documentKey)
        : null;
    const fullEditState = workspaceTab
      ? workspaceTab.fullEditUiState
      : isActiveEditorTab
        ? fullEditUiStateValue
        : undefined;
    return {
      id: tab.id,
      name: tab.name,
      documentKey: tab.documentKey,
      languageId: tab.languageId,
      sourceText: tab.model.getValue(),
      revision: workspaceTab?.revision ?? (isActiveEditorTab ? editorRevisionValue : 0),
      graphAppliedRevision: workspaceTab?.graphAppliedRevision ?? (isActiveEditorTab ? $graphAppliedRevision : 0),
      snapshotId,
      tempModel: tabManager.getTempModel(tab.id) ?? workspaceTab?.tempModel ?? $activeTempModel,
      ...(fullEditState ? { fullEditUiState: fullEditState } : {}),
    };
  }

  function syncTabBindings() {
    tabSummaries = editorStore.actions.getWorkspaceTabSummaries();
    activeTabId = editorStore.get().workspace.activeTabId;
  }

  function setActiveTab(tab: EditorTab, reason: 'initial-example' | 'tab-reactivate' = 'tab-reactivate') {
    if (!editor) return;
    tabManager.setActiveTabId(tab.id);
    if (!userInputByTabId.has(tab.id)) {
      userInputByTabId.set(tab.id, false);
    }
    jsonBlockSelection.set(null);
    clearJsonBlockDecoration();
    model = tab.model;
    setModelDocumentKey(model, tab.documentKey);
    editor.setModel(tab.model);
    syncColorViewportState('model');
    lastModelLength = tab.model.getValue().length;
    lastModelText = tab.model.getValue();
    editorStore.actions.activateWorkspaceTabFromEditor(workspacePayloadForTab(tab));
    setLanguageIdWithoutExample(tab.languageId);
    setActiveEditorIo();
    const tempModel = tabManager.getTempModel(tab.id) ?? {
      diffInputText: '',
      scratchText: '',
      commandQuery: '',
      status: 'Ready',
      error: '',
      cursor: 'Ln 1, Col 1',
      selectionLength: 0,
      treePath: [],
      graphHighlight: null,
      diagnostics: [],
    };
    tabManager.setTempModel(tab.id, tempModel);
    activeTempModel.set(tempModel);
    syncTabBindings();
    isStoreUpdateSuppressed = true;
    const nextText = tab.model.getValue();
    const requestModel = tab.model;
    void editorFullEditController.startFullEditSession({
      language: tab.languageId,
      text: nextText,
      reason,
      isFresh: () => model === requestModel && requestModel.getValue() === nextText,
    });
    releaseStoreUpdateSuppression();
  }

  export function addTab() {
    if (guardImportInProgress()) return;
    if (!monaco) return;
    const tab = tabManager.addTab(languageIdValue, getLanguageExample(languageIdValue));
    if (tab) {
      userInputByTabId.set(tab.id, false);
      editorStore.actions.addWorkspaceTabFromEditor(workspacePayloadForTab(tab));
      setActiveTab(tab);
      return;
    }
    syncTabBindings();
  }

  export function closeTab(id: string) {
    if (guardImportInProgress()) return;
    userInputByTabId.delete(id);
    const nextTab = tabManager.closeTab(id, languageIdValue, getLanguageExample(languageIdValue));
    if (nextTab && !userInputByTabId.has(nextTab.id)) {
      userInputByTabId.set(nextTab.id, false);
    }
    if (nextTab) {
      editorStore.actions.closeWorkspaceTabFromEditor(id, workspacePayloadForTab(nextTab));
      setActiveTab(nextTab);
      return;
    }
    editorStore.actions.closeWorkspaceTabFromEditor(id);
    syncTabBindings();
  }

  export function activateTab(id: string) {
    if (guardImportInProgress()) return;
    const tab = tabManager.activateTab(id);
    if (tab) setActiveTab(tab);
  }

  export function formatActive() {
    return editorFormatController.formatActive();
  }

  export function minifyActive() {
    return editorFormatController.minifyActive();
  }

  export function sortActive() {
    return editorFormatController.sortActive();
  }

  export async function exportAs(targetFormat: string) {
    if (!model) return null;
    const text = model.getValue();
    if (!text.trim()) return '';
    return callSharedWasmWorker<string>('convert', {
      sourceLanguage: languageIdValue,
      targetFormat,
      text,
      options: formattingOptionsValue,
    });
  }

  export function getActiveText() {
    return model?.getValue() ?? '';
  }

  export function getActiveLanguage() {
    return languageIdValue;
  }

  export async function escapeActive(): Promise<void> {
    if (!model) return;
    const text = model.getValue();
    if (!text.trim()) {
      toast.info('No content to escape');
      return;
    }
    try {
      const result = JSON.stringify(text);
      queueWholeDocumentReplacement(result, {
        sourceWritebackPolicy: 'submitted',
        formatSourceOnClose: false,
        shouldResolveLanguage: false,
        markUserInput: true,
      });
      resetEditorCursorToStart();
      toast.success('Escape completed');
    } catch (error) {
      toast.error('Escape failed');
      console.error('[editor] escape failed', error);
    }
  }

  export async function unescapeActive(): Promise<void> {
    if (!model) return;
    const text = model.getValue();
    if (!text.trim()) {
      toast.info('No content to unescape');
      return;
    }
    try {
      const parsed = JSON.parse(text);
      let result: string;
      if (typeof parsed === 'string') {
        result = parsed;
      } else {
        result = JSON.stringify(parsed, null, 2);
      }
      queueWholeDocumentReplacement(result, {
        sourceWritebackPolicy: 'submitted',
        formatSourceOnClose: false,
        shouldResolveLanguage: false,
        markUserInput: true,
      });
      resetEditorCursorToStart();
      toast.success('Unescape completed');
    } catch (error) {
      toast.error('Unescape failed: invalid JSON');
      console.error('[editor] unescape failed', error);
    }
  }

  export function cancelImportStream(): void {
    editorFullEditController.cancelImportStream();
  }

  export async function importStream(
    file: File,
    sourceLanguage: string,
    targetLanguage: SupportedEditorLanguageId = languageIdValue,
  ): Promise<void> {
    markActiveTabUserInput(true);
    await editorFullEditController.importStream(file, sourceLanguage, 'import-file', targetLanguage);
  }

  export async function importAs(targetFormat: string, text: string, sourceFormat: string) {
    if (!model || !monaco) return;
    const nextLanguage = supportedEditorLanguageSet.has(sourceFormat as SupportedEditorLanguageId)
      ? (sourceFormat as SupportedEditorLanguageId)
      : (targetFormat as SupportedEditorLanguageId);
    if (!supportedEditorLanguageSet.has(nextLanguage)) {
      const message = `Unsupported editor language: ${nextLanguage}`;
      activeTempModel.update((current) => ({ ...current, error: message }));
      toast.error(message);
      throw new Error(message);
    }
    suppressNextWholeDocumentAutoGuess = true;
    markActiveTabUserInput(true);
    setLanguageIdWithoutExample(nextLanguage);
    setEditorValue(text);
  }

  export function revealError(startLineNumber: number, startColumn: number) {
    if (!editor || !model || !monaco) return;
    suppressNextGraphHighlightSync();
    editor.setPosition({ lineNumber: startLineNumber, column: startColumn });
    editor.setSelection(
      new monaco.Selection(startLineNumber, startColumn, startLineNumber, Math.max(startColumn + 1, startColumn)),
    );
    editor.revealPositionInCenter({ lineNumber: startLineNumber, column: startColumn });
    editor.focus();
  }

  export function revealLine(lineNumber: number, column = 1) {
    if (!editor || !model || !monaco) return;
    const safeLineNumber = Math.max(1, Math.min(lineNumber, model.getLineCount()));
    const lineLength = model.getLineContent(safeLineNumber).length;
    const safeColumn = Math.max(1, Math.min(column, Math.max(1, lineLength + 1)));
    suppressNextGraphHighlightSync();
    editor.setPosition({ lineNumber: safeLineNumber, column: safeColumn });
    editor.setSelection(new monaco.Selection(safeLineNumber, safeColumn, safeLineNumber, safeColumn));
    editor.revealPositionInCenter({ lineNumber: safeLineNumber, column: safeColumn });
    editor.focus();
  }

  export async function revealPath(path: PathSeg[], options?: { target?: 'key' | 'value' | 'node'; focus?: boolean }) {
    if (!editor || !model || !monaco) return;
    if (!path || path.length === 0) return;
    const documentKeyValue = getDocumentKey();
    if (!documentKeyValue) return;
    const selectionRange = await resolvePathSelectionRangeSafe(
      model,
      path,
      documentKeyValue,
      languageIdValue,
      options?.target ?? 'node',
      $settings.parser.enableNest,
      getActiveDocumentSnapshotId(documentKeyValue),
    );
    if (!selectionRange) {
      const message = `Failed to reveal path ${JSON.stringify(path)}`;
      // Clear graphHighlight to prevent the reactive subscription from
      // re-entering — the error update would otherwise keep the reference
      // check alive (H1 !== H2) and loop forever.
      activeTempModel.update((current) => ({ ...current, error: message, graphHighlight: null }));
      toast.error('Reveal failed');
      throw new Error(message);
    }
    suppressNextGraphHighlightSync();
    suppressNextTreePathUpdate();
    editor.setSelection(
      new monaco.Selection(
        selectionRange.start.lineNumber,
        selectionRange.start.column,
        selectionRange.end.lineNumber,
        selectionRange.end.column,
      ),
    );
    editor.revealPositionInCenter(selectionRange.start);
    if (options?.focus !== false) editor.focus();
  }

  export function getScrollPosition() {
    if (!editor) return null;
    return { scrollTop: editor.getScrollTop(), scrollLeft: editor.getScrollLeft() };
  }

  export function setScrollPosition(position: { scrollTop: number; scrollLeft: number }) {
    if (!editor) return;
    editor.setScrollPosition(position);
  }

  function clearDiffPlan() {
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

  export function applyDiffPlan(plan: DiffPlan) {
    if (!editor) return;
    clearDiffPlan();
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
  }

  async function handleDrop(event: DragEvent): Promise<void> {
    if ((event.dataTransfer?.files?.length ?? 0) > 0) {
      markActiveTabUserInput(true);
    }
    await editorFullEditController.handleDrop(event);
  }

  async function resolveWholeDocumentReplacementLanguage(
    text: string,
    currentLanguage: SupportedEditorLanguageId,
  ): Promise<SupportedEditorLanguageId> {
    const guessed = await guessLanguage(text);
    return guessed ?? currentLanguage;
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
  }

  function getDocumentKey() {
    if (!model) return '';
    const documentKey = getModelDocumentKey(model);
    if (documentKey) return documentKey;
    const fallback = ensureModelDocumentKey(model);
    return fallback;
  }


  onMount(async () => {
    const runtimeToken = ++editorRuntimeToken;
    const freshness = createFreshnessScope({ token: runtimeToken }, () => ({ token: editorRuntimeToken }));
    editorRuntimeReady = false;
    editorRuntimeError = false;
    editorRuntimePhase = 'Loading editor runtime...';

    try {
      // ── Phase 1: Monaco shell — editor interactive without WASM ──
      const shell = await freshness.step(() => editorRuntimeController.initShell());
      if (!shell) return;

      editorRuntimePhase = 'Starting language services...';
      editorRuntimeController.applyTheme(shell.monaco);
      editorRuntimeController.scheduleWorkerWarmup();

      editorRuntimePhase = 'Preparing sample document...';
      initFirstTab();
      bindEditorEvents();
      await awaitEditorRuntimeStartupDelay();

      if (freshness.isCurrent()) {
        editorRuntimeReady = true;
        editorRuntimePhase = '';
      }

      // ── Phase 2: Language services — async, after WASM loads ──
      const langServices = await editorRuntimeController.initLanguageServices();
      if (!freshness.isCurrent()) return;

      const ensureSemanticTokensProvider = langServices.ensureSemanticTokensProvider;
      refreshSemanticTokensForLanguage = langServices.refreshSemanticTokens;
      primeSemanticTokensForDocument = langServices.primeSemanticTokens;
      clearSemanticTokensForDocument = langServices.clearSemanticTokens;
      const ensureDocumentColorProvider = langServices.ensureDocumentColorProvider;
      updateDocumentColorViewport = langServices.updateDocumentColorViewport;
      refreshVisibleDocumentColors = langServices.refreshVisibleDocumentColors;

      // Wire up language services for already-open editor
      ensureSemanticTokensProvider(languageIdValue);
      ensureDocumentColorProvider(languageIdValue);
      setupLanguageSubscription(ensureSemanticTokensProvider, ensureDocumentColorProvider);

      if (editor && monaco) {
        hoverPreviewDisposable = registerEditorHoverPreview({
          monaco,
          editor,
          getTreeState: () => $treeState,
          getRevision: () => editorRevisionValue,
          getDocumentKey,
          getLanguageId: () => languageIdValue,
          getNestEnabled: () => $settings.parser.enableNest,
          isImportActive: () => editorFullEditController.isImportActive(),
        });
      }
      bindStoreSubscriptions();
    } catch (error) {
      if (freshness.isCurrent()) {
        editorRuntimeError = true;
        editorRuntimePhase = 'Editor failed to load. Please refresh and try again.';
      }
      throw error;
    }
  });

  onDestroy(() => {
    editorRuntimeToken += 1;
    clearColorViewportRefresh();
    editorIO.set(null);
    editorFullEditController.dispose();
    cleanupSourceEditorTestHook?.();
    cleanupSourceEditorTestHook = null;
    if (editor) {
      editor.dispose();
      editor = null;
    }
    if (model) {
      model.dispose();
      model = null;
    }
    tabManager?.disposeAll();
    hoverPreviewDisposable?.dispose();
    hoverPreviewDisposable = null;
    rootScalarDecorations?.clear();
    rootScalarDecorations = null;
    storeUnsub?.();
    storeUnsub = null;
    languageUnsub?.();
    languageUnsub = null;
    tempModelUnsub?.();
    tempModelUnsub = null;
    jsonBlockSelectionUnsub?.();
    jsonBlockSelectionUnsub = null;

    clearJsonBlockDecoration();
  });

  async function applyEditorMutation(mutation: EditorMutation): Promise<void> {
    if (!model) return;
    if (mutation.type === 'applyValueEdit') {
      const { path, preferKey, value } = mutation.payload;
      try {
        const node = isTreeNodeLike(value)
          ? value
          : await callSharedWasmWorker<TreeNode>('valueToTreeNode', {
              value,
            });
        const nextText = await callSharedWasmWorker<string>('applyValueEdit', {
          language: languageIdValue,
          text: model.getValue(),
          path,
          preferKey,
          value: node,
        });
        if (typeof nextText === 'string' && nextText !== model.getValue()) {
          setEditorValue(nextText);
        }
        return;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        activeTempModel.update((current) => ({ ...current, error: message }));
      }
      return;
    }
    if (mutation.type === 'replaceSourceText') {
      setEditorValue(mutation.payload.text);
    }
  }

  $: if (monaco && $settings) {
    editorRuntimeController.applyTheme(monaco);
  }

  $: if (tabManager) {
    tabSummaries = editorStore.actions.getWorkspaceTabSummaries();
    activeTabId = editorStore.get().workspace.activeTabId;
  }
</script>

<div class="contents" style={rootScalarStyle}>
  <EditorDropZone
    bind:this={dropZone}
    onDrop={handleDrop}
    onDragOver={handleDragOver}
    loading={editorRuntimeOverlay.loading}
    loadingPhase={editorRuntimeOverlay.phase}
  />
  <TabManager bind:this={tabManager} {monaco} {maxTabs} initialLanguageId={languageIdValue} {initialCode} />
</div>

<style>
  :global(.treease-root-scalar-str) {
    color: var(--treease-root-scalar-str) !important;
  }

  :global(.treease-root-scalar-int) {
    color: var(--treease-root-scalar-int) !important;
  }

  :global(.treease-root-scalar-float) {
    color: var(--treease-root-scalar-float) !important;
  }

  :global(.treease-root-scalar-boolean) {
    color: var(--treease-root-scalar-boolean) !important;
  }

  :global(.treease-root-scalar-nil) {
    color: var(--treease-root-scalar-nil) !important;
  }
</style>
