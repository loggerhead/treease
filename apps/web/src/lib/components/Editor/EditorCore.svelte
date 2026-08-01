<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import type * as Monaco from 'monaco-editor';
  import type { DocumentTextEdit } from '@core-wasm/index';
  import type { DiffPlan } from '../../graph/diff-plan';
  import {
    compareEditToken,
    documentKey as documentKeyStore,
    editorIO,
    getDocumentSessionState,
    editorMutation,
    editorRevision,
    graphAppliedRevision,
    languageId as languageIdStore,
    setEditorIO,
    sourceText,
    type EditorMutation,
  } from '../../store/document-session-store';
  import { activeTempModel, treeState, type GraphHighlightTarget } from '../../store/graph-selection-store';
  import {
    cancelPreparedFullEditStream,
    fullEditUiState,
    getFullEditUiStateSnapshot,
    jsonBlockSelection,
    prepareFullEditStream,
    type JsonBlockSelection,
  } from '../../store/full-edit-ui-store';
  import {
    getWorkspaceRawState,
    getWorkspaceState,
    setWorkspaceState,
    updateWorkspaceTab,
  } from '../../store/workspace-store';
  import type { PathSeg } from '../../store/tree-path';
  import { getLanguageExample } from '../../monaco/language-examples';
  import {
    editorLanguageFallback,
    importFormatOptions,
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
  import { getActiveDocumentText, resolveCommitBaseSnapshotId } from '../../store/active-document-authority';
  import { getWorkspaceSnapshotId } from '../../store/workspace-store';
  import { resolvePathSelectionRangeResult } from '../../services/TreePathService';
  import { resolveEditorRuntimeOverlay, type RuntimeStateEventDetail } from '../../runtime-loading';
  import {
    markCursorPathRequested,
    markCursorPathSettled,
  } from '../../test-bridge/runtime-readiness';
  import { awaitEditorRuntimeStartupDelay } from '../../test-bridge/runtime-startup-delay';

  import EditorDropZone from './EditorDropZone.svelte';
  import { shouldSyncGraphHighlightFromCursorReason } from './EditorCore.graph-highlight';
  import { ensureModelDocumentKey } from './document-key';
  import { registerEditorHoverPreview } from './editor-hover';
  import { createEditorAnalysisController } from './editor-analysis-controller';
  import { createEditorFormatController } from './editor-format-controller';
  import { createEditorFullEditController } from './editor-full-edit-controller';
  import { resolveLanguageSwitchPolicy } from './language-switch-policy';
  import { createEditorRuntimeController } from './editor-runtime-controller';
  import { commitEditorTabTextChange } from './editor-tab-edit-commit';
  import { settleWholeDocumentReplacement } from './whole-document-replacement';
  import type { EditorModelWithDocumentKey } from './types';
  import { EditorTabRuntime } from './editor-tab-runtime';
  import {
    activateWorkspaceTabTransition,
    closeWorkspaceTabTransition,
    createWorkspaceTabTransition,
    syncSidecarLanguageFromPrimary,
    type EditorWorkspaceTab,
  } from '../../store/editor-workspace';
  import { EDITOR_CONFIG } from '../../config/constants';
  import { monacoChangesToDocumentTextEdits, type MonacoTextChange } from '../../../shared/document-text-edits';
  import { serializePath } from '../../../shared/document-anchor-utils';
  import { createTreeaseMonacoEditorOptions } from './editor-options';
  import type { DocumentOrigin } from '../../document-origin';

  export let onScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {};
  export let enableRevealSync = true;
  export let synchronizedRuntimeLoading = false;
  export let runBidirectionalEdit: <T>(source: string, execute: () => Promise<T>, reason?: string) => Promise<T> = async (_source, execute) => execute();
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => Promise<void> = async () => {};

  type QueuedWholeDocumentReplacement = {
    text: string;
    sourceWritebackPolicy?: 'intake' | 'submitted';
    formatSourceOnClose?: boolean;
    shouldResolveLanguage?: boolean;
    markUserInput?: boolean;
    skipUsageMetering?: boolean;
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
  let placeholderWidget: Monaco.editor.IContentWidget | null = null;
  let placeholderVisible = false;

  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback;
  let shouldSuppressLanguageExample = false;
  let lastMutationId = 0;
  let lastExternalTreeSelectionSignature = '';
  let diffDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let jsonBlockDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let diffBlankZoneIds: string[] = [];
  let suppressGraphHighlightSync = 0;
  let suppressTreePathUpdate = 0;
  let unfocusedExternalRevealSelection = false;
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
  const LARGE_TEXT_EDIT_USAGE_THRESHOLD_BYTES = 256 * 1024;
  type EditorUsageRunner = (source: string, execute: () => Promise<unknown>) => Promise<unknown>;
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

  function changesCoverWholeDocument(changes: MonacoTextChange[], previousLength: number): boolean {
    if (!changes.length) return false;
    const ordered = [...changes].sort((left, right) => left.rangeOffset - right.rangeOffset);
    if (ordered[0].rangeOffset !== 0) return false;
    let coveredEnd = 0;
    for (const change of ordered) {
      if (change.rangeOffset > coveredEnd) return false;
      coveredEnd = Math.max(coveredEnd, change.rangeOffset + change.rangeLength);
    }
    return coveredEnd >= previousLength;
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

  function setActiveEditorIo(): void {
    setEditorIO({
      context: 'editor',
      getModel: () => model,
      getText: () => model?.getValue() ?? getDocumentSessionState().sourceText,
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

  function isRangeSelection(range: { start: Monaco.IPosition; end: Monaco.IPosition }): boolean {
    return range.start.lineNumber !== range.end.lineNumber || range.start.column !== range.end.column;
  }

  function handleEditorPointerDownCapture(): void {
    if (!unfocusedExternalRevealSelection || !editor) return;
    if (editor.hasTextFocus()) return;
    editor.focus();
  }

  function buildExternalTreeSelectionSignature(graphHighlight: {
    path: PathSeg[];
    target?: GraphHighlightTarget;
    revision: number;
    source: string;
  }): string {
    return `${graphHighlight.source}|${graphHighlight.target ?? 'auto'}|${graphHighlight.revision}|${serializePath(graphHighlight.path)}`;
  }

  const getNestEnabled = () => $settings.parser.enableNest;
  const updateCurrentTempModel = (updater: (current: any) => any) => activeTempModel.update(updater);
  function clearDocumentSemanticTokens(documentKey: string | undefined): void {
    clearSemanticTokensForDocument(documentKey);
  }

  function refreshDocumentSemanticTokens(languageId: string | undefined): void {
    refreshSemanticTokensForLanguage(languageId);
  }
  const callWasmWorkerFromEditor = <T>(method: string, input: unknown) =>
    callSharedWasmWorker<T>(method as any, input);

  function rotateActiveDocumentKey(): string {
    const activeId = getWorkspaceState().activeTabId;
    const tab = getWorkspaceState().tabsById[activeId];
    const activeModel = tabRuntime?.get(activeId);
    if (tab && activeModel) {
      const rotated = `${tab.documentKey}:${Date.now()}`;
      activeModel.__treeaseDocumentKey = rotated;
      updateWorkspaceTab(activeId, { documentKey: rotated });
      documentKeyStore.set(rotated);
      return rotated;
    }
    const fallback = ensureModelDocumentKey(model);
    documentKeyStore.set(fallback);
    return fallback;
  }

  let fullEditUiStateValue = $fullEditUiState;
  let queuedProgrammaticSourceText: string | null = null;
  $: fullEditUiStateValue = $fullEditUiState;

  const treePathLanguages = supportedEditorLanguageSet;
  const themeName = 'tree-sitter-light';
  const editorOptions = {
    ...createTreeaseMonacoEditorOptions(themeName),
    scrollbar: { alwaysConsumeMouseWheel: false },
    overviewRulerBorder: true,
    colorDecorators: true,
    colorDecoratorsActivatedOn: 'clickAndHover' as const,
    'semanticHighlighting.enabled': true,
    readOnly: true,
  };
  const maxTabs = EDITOR_CONFIG.maxTabs;
  const initialCode = getLanguageExample('json');

  function updateEditorPlaceholder(): void {
    if (!editor || !model) return;
    const shouldShow = model.getValue().trim() === '';

    if (!shouldShow) {
      if (placeholderWidget) {
        editor.removeContentWidget(placeholderWidget);
        placeholderWidget = null;
      }
      placeholderVisible = false;
      return;
    }

    if (placeholderWidget) {
      placeholderVisible = true;
      return;
    }

    placeholderWidget = {
        getId: () => 'treease-editor-placeholder',
        getDomNode: () => {
          const root = document.createElement('div');
          root.className = 'treease-editor-placeholder';

          const title = document.createElement('div');
          title.textContent = 'Start typing, or open a file';
          root.appendChild(title);

          const openFileRow = document.createElement('div');
          openFileRow.className = 'treease-editor-placeholder__row';
          const openFile = document.createElement('button');
          openFile.type = 'button';
          openFile.className = 'treease-editor-placeholder__link';
          openFile.textContent = 'Choose a file or drag one into this editor';
          openFile.addEventListener('click', (event) => {
            event.preventDefault();
            event.stopPropagation();
            void onRequestImportFile({
              sourceFormat: languageIdValue,
              targetFormat: languageIdValue,
              accept: importFormatOptions.find((option) => option.id === languageIdValue)?.extensions ?? [],
            });
          });
          openFileRow.appendChild(openFile);
          root.appendChild(openFileRow);

          const exampleRow = document.createElement('div');
          exampleRow.className = 'treease-editor-placeholder__row';
          const loadExample = document.createElement('button');
          loadExample.type = 'button';
          loadExample.className = 'treease-editor-placeholder__link';
          loadExample.textContent = 'Load an example file';
          loadExample.addEventListener('click', (event) => {
            event.preventDefault();
            event.stopPropagation();
            const example = getLanguageExample(languageIdValue);
            if (!example) return;
            setActiveTabOrigin('example');
            queueWholeDocumentReplacement(example, {
              sourceWritebackPolicy: 'intake',
              formatSourceOnClose: false,
              shouldResolveLanguage: false,
              markUserInput: false,
            });
            editor?.focus();
          });
          exampleRow.appendChild(loadExample);
          root.appendChild(exampleRow);
          return root;
        },
        getPosition: () => ({
          position: { lineNumber: 1, column: 1 },
          preference: [monaco.editor.ContentWidgetPositionPreference.EXACT],
        }),
        suppressMouseDown: true,
    };
    editor.addContentWidget(placeholderWidget);

    placeholderVisible = true;
  }

  let tabRuntime: EditorTabRuntime;
  let tabSequence = 1;
  let dropZone: EditorDropZone;

  function activeTabHasUserInput(language: SupportedEditorLanguageId): boolean {
    const activeId = getWorkspaceState().activeTabId;
    if (!activeId) return false;
    const recorded = userInputByTabId.get(activeId);
    if (recorded !== undefined) return recorded;
    return (model?.getValue() ?? '') !== getLanguageExample(language);
  }

  function markActiveTabUserInput(value: boolean): void {
    const activeId = getWorkspaceState().activeTabId;
    if (activeId) {
      userInputByTabId.set(activeId, value);
      if (value) setActiveTabOrigin('user');
    }
  }

  function setActiveTabOrigin(origin: DocumentOrigin): void {
    const activeId = getWorkspaceState().activeTabId;
    if (activeId) {
      userInputByTabId.set(activeId, origin !== 'example');
      updateWorkspaceTab(activeId, { origin });
      updateEditorPlaceholder();
    }
  }

  function guardImportInProgress(): boolean {
    return editorFullEditController.isImportActive();
  }

  function showImportBlockedToast(): void {
    toast.info('Import in progress');
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
      const activeId = getWorkspaceState().activeTabId;
      if (activeId) {
        const activeModel = tabRuntime?.get(activeId);
        if (activeModel) activeModel.__treeaseDocumentKey = documentKey;
        updateWorkspaceTab(activeId, { documentKey });
      }
    },
    clearSemanticTokensForDocument: clearDocumentSemanticTokens,
    setEditorValue,
    setEditorValueForFullEdit,
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
    runBidirectionalEdit,
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
      lastExternalTreeSelectionSignature = '';
    } else if (editor && model) {
      const signature = buildExternalTreeSelectionSignature(graphHighlight);
      if (signature !== lastExternalTreeSelectionSignature) {
        lastExternalTreeSelectionSignature = signature;
        void revealPath(graphHighlight.path, {
          target: graphHighlight.target,
          focus: false,
        }).catch(() => {
          // revealPath already reports the failure via toast + temp model state.
        });
      }
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
        const languageSwitchSourceText = getActiveDocumentText();
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
      const activeId = getWorkspaceState().activeTabId;
      if (activeId) updateWorkspaceTab(activeId, { languageId: next });
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
    const workspace = getWorkspaceState();
    const firstTab = workspace.tabsById[workspace.activeTabId];
    if (!firstTab) throw new Error('Workspace must have an active left tab before editor initialization.');
    tabRuntime = new EditorTabRuntime(monaco);
    model = tabRuntime.getOrCreate(firstTab);
    lastModelLength = model.getValue().length;
    lastModelText = model.getValue();
    const container = dropZone.getContainer();
    editor = monaco.editor.create(container, {
      model,
      ...editorOptions,
    });
    updateEditorPlaceholder();
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
        getMarkers: () => (model ? monaco.editor.getModelMarkers({ resource: model.uri }) : []),
        getModel: () => editor?.getModel() ?? null,
      },
      'source-editor',
      monaco.editor.tokenize,
    );
    ensureLanguageRegistered(languageIdValue);
    monaco.editor.setModelLanguage(model, languageIdValue);
    syncColorViewportState('init');
    return firstTab;
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

    editor.onMouseDown((event) => {
      if (!unfocusedExternalRevealSelection || !editor || !model || !monaco) return;
      unfocusedExternalRevealSelection = false;
      const position = event.target.position;
      if (!position) return;
      queueMicrotask(() => {
        if (!editor || !model || !monaco) return;
        editor.setSelection(new monaco.Selection(position.lineNumber, position.column, position.lineNumber, position.column));
        editor.setPosition(position);
      });
    });

    editor.onDidChangeModelContent((event) => {
      const activeModel = model;
      if (!activeModel) return;
      updateEditorPlaceholder();
      const previousLength = lastModelLength;
      const previousText = lastModelText;
      const nextText = activeModel.getValue();
      const changes = (event as unknown as { changes?: MonacoTextChange[] }).changes ?? [];
      const isFlush = (event as unknown as { isFlush?: boolean }).isFlush ?? false;
      syncColorViewportState('content');
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
      if (previousDocumentKey) clearDocumentSemanticState(previousDocumentKey);
      const shouldSkipWholeDocumentAutoGuess = suppressNextWholeDocumentAutoGuess;
      suppressNextWholeDocumentAutoGuess = false;
      const wholeDocumentReplacement = changesCoverWholeDocument(changes, previousLength)
        ? { ...changes[0], rangeOffset: 0, rangeLength: previousLength, text: nextText }
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
        const skipUsageMetering = queuedReplacement?.skipUsageMetering ?? false;
        prepareFullEditStream({
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
              skipUsageMetering,
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
            cancelPreparedFullEditStream({
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
      if (documentKeyValue && changes.length > 0 && !isFlush) {
        const documentTextEdits = monacoChangesToDocumentTextEdits(
          new TextEncoder().encode(previousText),
          new TextEncoder().encode(nextText),
          changes,
        );
        markActiveTabUserInput(true);
        const insertedByteLength = changes.reduce(
          (total, change) => total + new TextEncoder().encode(change.text ?? '').byteLength,
          0,
        );
        commitActiveTabEdits(
          activeModel,
          languageIdValue,
          documentKeyValue,
          nextText,
          documentTextEdits,
          insertedByteLength >= LARGE_TEXT_EDIT_USAGE_THRESHOLD_BYTES
            ? (source, execute) => runBidirectionalEdit(source, execute, 'whole-document-replacement')
            : undefined,
        );
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
      if (getFullEditUiStateSnapshot().active) return;
      if (value !== model.getValue()) {
        model.setValue(value);
      }
    });
    tempModelUnsub = activeTempModel.subscribe((value) => {
      const activeId = getWorkspaceState().activeTabId;
      if (activeId) updateWorkspaceTab(activeId, { tempModel: value });
    });
    jsonBlockSelectionUnsub = jsonBlockSelection.subscribe((value) => {
      jsonBlockSelectionValue = value;
      applyJsonBlockDecoration(value);
      if (value) {
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

  function commitActiveTabEdits(
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    nextText: string,
    documentTextEdits: DocumentTextEdit[],
    runUsage: EditorUsageRunner | undefined = undefined,
  ): number {
    return commitEditorTabTextChange({
      requestModel,
      requestLanguage,
      requestDocumentKey,
      nextText,
      documentTextEdits,
      runUsage,
      baseSnapshotId: resolveCommitBaseSnapshotId(requestDocumentKey),
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
      applyCommittedSourceText: (sourceTextValue) => {
        if (sourceTextValue !== getActiveDocumentText()) {
          sourceText.set(sourceTextValue)
        }
      },
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
    const previousValue = getActiveDocumentText();
    if (value === previousValue) {
      return false;
    }
    if (fullEditUiStateValue.active) {
      queuedProgrammaticSourceText = value;
      return true;
    }
    if (model) {
      model.setValue(value);
      return true;
    }
    sourceText.set(value);
    return true;
  }

  function setEditorValueForFullEdit(value: string): boolean {
    if (!model) return false;
    const previousValue = model.getValue();
    isStoreUpdateSuppressed = true;
    if (previousValue !== value) {
      model.setValue(value);
      syncLastModelSnapshot();
    }
    if (getDocumentSessionState().sourceText !== value) {
      sourceText.set(value);
    }
    releaseStoreUpdateSuppression();
    return previousValue !== value;
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
    return editor.executeEdits('graph-value-edit', operations);
  }

  function resetEditorCursorToStart(): void {
    if (!editor || !monaco) return;
    suppressNextGraphHighlightSync();
    editor.setPosition({ lineNumber: 1, column: 1 });
    editor.setSelection(new monaco.Selection(1, 1, 1, 1));
    editor.setScrollPosition({ scrollTop: 0, scrollLeft: 0 });
  }

  type InstalledActiveTab = { model: Monaco.editor.ITextModel; text: string };

  function installActiveTab(tab: EditorWorkspaceTab): InstalledActiveTab | null {
    if (!editor) return null;
    if (!userInputByTabId.has(tab.id)) {
      userInputByTabId.set(tab.id, false);
    }
    jsonBlockSelection.set(null);
    clearJsonBlockDecoration();
    model = tabRuntime.getOrCreate(tab);
    setModelDocumentKey(model, tab.documentKey);
    editor.setModel(model);
    // The installed Monaco model and authority session must change together.
    // Otherwise a later session-store emission can overwrite this tab with the
    // text from the previously active model.
    isStoreUpdateSuppressed = true;
    const text = model.getValue();
    sourceText.set(text);
    documentKeyStore.set(tab.documentKey);
    placeholderVisible = false;
    updateEditorPlaceholder();
    syncColorViewportState('model');
    lastModelLength = model.getValue().length;
    lastModelText = model.getValue();
    setLanguageIdWithoutExample(tab.languageId);
    setActiveEditorIo();
    activeTempModel.set(tab.tempModel);
    return { model, text };
  }

  async function startInstalledActiveTab(
    tab: EditorWorkspaceTab,
    installed: InstalledActiveTab,
    reason: 'initial-example' | 'tab-reactivate',
    options: { awaitSnapshotReady?: boolean; editorReadOnly?: boolean } = {},
  ): Promise<boolean> {
    const requestModel = installed.model;
    const fullEditRequest = {
      language: tab.languageId,
      text: installed.text,
      reason,
      editorReadOnly: options.editorReadOnly ?? false,
      isFresh: () => model === requestModel && requestModel.getValue() === installed.text,
    };
    try {
      if (options.awaitSnapshotReady) {
        const outcome = await editorFullEditController.runFullEditSessionToTerminal(fullEditRequest);
        if (outcome.status !== 'completed' || outcome.snapshotId == null) {
          editor.updateOptions({ readOnly: true });
          throw new Error(`Initial document did not produce SnapshotReady: ${outcome.status}`);
        }
      } else {
        void editorFullEditController.startFullEditSession(fullEditRequest);
      }
      return true;
    } finally {
      releaseStoreUpdateSuppression();
    }
  }

  async function setActiveTab(
    tab: EditorWorkspaceTab,
    reason: 'initial-example' | 'tab-reactivate' = 'tab-reactivate',
    options: { awaitSnapshotReady?: boolean; editorReadOnly?: boolean } = {},
  ): Promise<boolean> {
    const installed = installActiveTab(tab);
    if (!installed) return false;
    if (reason === 'initial-example') commitEditorState();
    return startInstalledActiveTab(tab, installed, reason, options);
  }

  export function addTab() {
    if (guardImportInProgress()) {
      showImportBlockedToast();
      return;
    }
    if (!monaco) return;
    const id = `tab-${Date.now()}-${tabSequence++}`;
    const transition = createWorkspaceTabTransition(getWorkspaceRawState(), { id, name: `Untitled ${tabSequence}`, documentKey: `${id}:0`, languageId: languageIdValue, sourceText: '', origin: 'user' });
    const tab = transition?.workspace.tabsById[id];
    if (tab && transition) {
      // Install the model before publishing the new active workspace tab.
      const installed = installActiveTab(tab);
      if (!installed) return;
      setWorkspaceState(syncSidecarLanguageFromPrimary(transition.workspace));
      commitEditorState();
      void startInstalledActiveTab(tab, installed, 'tab-reactivate');
    }
  }

  export function openDocument(payload: {
    name: string;
    text: string;
    languageId: SupportedEditorLanguageId;
    origin?: DocumentOrigin;
    fileLinkedDocument?: { grantId: string; name: string };
  }): string | null {
    if (guardImportInProgress()) {
      showImportBlockedToast();
      return null;
    }
    if (!monaco) return null;
    const id = `tab-${Date.now()}-${tabSequence++}`;
    const transition = createWorkspaceTabTransition(getWorkspaceRawState(), { id, name: payload.name, documentKey: `${id}:0`, languageId: payload.languageId, sourceText: payload.text, origin: payload.origin ?? 'import', fileLinkedDocument: payload.fileLinkedDocument, savedText: payload.fileLinkedDocument ? payload.text : undefined });
    const tab = transition?.workspace.tabsById[id];
    if (!tab) return null;
    userInputByTabId.set(tab.id, true);
    const installed = installActiveTab(tab);
    if (!installed) return null;
    setWorkspaceState(syncSidecarLanguageFromPrimary(transition.workspace));
    commitEditorState();
    void startInstalledActiveTab(tab, installed, 'tab-reactivate');
    return tab.id;
  }

  export async function replaceActiveFromFile(payload: {
    text: string;
    languageId: SupportedEditorLanguageId;
    origin?: DocumentOrigin;
    skipUsageMetering?: boolean;
  }): Promise<void> {
    if (!model || !monaco) return;
    suppressNextWholeDocumentAutoGuess = true;
    setLanguageIdWithoutExample(payload.languageId);
    queueWholeDocumentReplacement(payload.text, { skipUsageMetering: payload.skipUsageMetering });
    setActiveTabOrigin(payload.origin ?? 'import');
  }

  export function replaceDocumentFromFile(payload: { tabId: string; text: string; languageId: SupportedEditorLanguageId }): void {
    const tab = getWorkspaceState().tabsById[payload.tabId];
    if (!tab || !monaco) return;
    const targetModel = tabRuntime.getOrCreate(tab);
    monaco.editor.setModelLanguage(targetModel, payload.languageId);
    targetModel.setValue(payload.text);
    userInputByTabId.set(tab.id, true);
    updateWorkspaceTab(tab.id, { origin: 'import', languageId: payload.languageId, sourceText: payload.text });
  }

  export function renameDocument(tabId: string, name: string): void {
    updateWorkspaceTab(tabId, { name });
  }

  export function closeTab(id: string) {
    if (guardImportInProgress()) {
      showImportBlockedToast();
      return;
    }
    const workspace = getWorkspaceRawState();
    const wasActive = workspace.activeTabId === id || workspace.primaryTabId === id || workspace.paneTabIds.left === id;
    const blankId = `tab-${Date.now()}-${tabSequence++}`;
    const transition = closeWorkspaceTabTransition(workspace, id, { id: blankId, documentKey: `${blankId}:0`, name: `Untitled ${tabSequence}`, languageId: languageIdValue });
    if (!transition) return;
    userInputByTabId.delete(id);
    const nextTab = transition.workspace.tabsById[transition.effect.tabId];
    if (!nextTab) return;
    if (!wasActive) {
      setWorkspaceState(syncSidecarLanguageFromPrimary(transition.workspace));
      if (transition.effect.disposeTabId) tabRuntime.dispose(transition.effect.disposeTabId);
      return;
    }
    // Install successor before releasing the removed model; editorIO must never observe a disposed active document.
    const installed = installActiveTab(nextTab);
    if (!installed) return;
    setWorkspaceState(syncSidecarLanguageFromPrimary(transition.workspace));
    if (transition.effect.kind === 'activate-new-blank') commitEditorState();
    void startInstalledActiveTab(nextTab, installed, 'tab-reactivate');
    if (transition.effect.disposeTabId) tabRuntime.dispose(transition.effect.disposeTabId);
  }

  export function activateTab(id: string) {
    if (guardImportInProgress()) return;
    const transition = activateWorkspaceTabTransition(getWorkspaceRawState(), id);
    const tab = transition?.workspace.tabsById[id];
    if (tab && transition) {
      const installed = installActiveTab(tab);
      if (!installed) return;
      setWorkspaceState(syncSidecarLanguageFromPrimary(transition.workspace));
      void startInstalledActiveTab(tab, installed, 'tab-reactivate');
    }
  }

  export function formatActive() {
    return editorFormatController.formatActive();
  }

  export function minifyActive() {
    return editorFormatController.minifyActive();
  }

  export function compactActive() {
    return editorFormatController.compactActive();
  }

  export function sortActive() {
    return editorFormatController.sortActive();
  }

  export async function exportAs(targetFormat: string) {
    const text = getActiveDocumentText();
    if (!text.trim()) return '';
    return callSharedWasmWorker<string>('convert', {
      sourceLanguage: languageIdValue,
      targetFormat,
      text,
      options: formattingOptionsValue,
    });
  }

  export function getActiveText() {
    return getActiveDocumentText();
  }

  export function getActiveLanguage() {
    return languageIdValue;
  }

  export async function escapeActive(): Promise<void> {
    const text = getActiveDocumentText();
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
    const text = getActiveDocumentText();
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

  export async function revealPath(
    path: PathSeg[],
    options: { target?: 'key' | 'value' | 'node'; focus?: boolean } | undefined,
  ) {
    if (!editor || !model || !monaco) return;
    if (!path || path.length === 0) return;
    const documentKeyValue = getDocumentKey();
    if (!documentKeyValue) return;
    const selectionRangeResult = await resolvePathSelectionRangeResult(
      model,
      path,
      documentKeyValue,
      languageIdValue,
      options?.target ?? 'node',
      $settings.parser.enableNest,
      getWorkspaceSnapshotId(documentKeyValue),
    );
    if (selectionRangeResult.status !== 'ready') return false;
    const selectionRange = selectionRangeResult.data;
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
    const shouldFocusEditor = options?.focus !== false;
    editor.setSelection(
      new monaco.Selection(
        selectionRange.start.lineNumber,
        selectionRange.start.column,
        selectionRange.end.lineNumber,
        selectionRange.end.column,
      ),
    );
    // Navigating between fields should not force Monaco to rebuild the visible
    // semantic-token viewport when the target is already visible.  The
    // unconditional center reveal causes a transient loss of semantic
    // decorations on every navigation change.
    editor.revealRangeInCenterIfOutsideViewport(
      new monaco.Range(
        selectionRange.start.lineNumber,
        selectionRange.start.column,
        selectionRange.end.lineNumber,
        selectionRange.end.column,
      ),
    );
    if (shouldFocusEditor) {
      unfocusedExternalRevealSelection = false;
      editor.focus();
    } else {
      unfocusedExternalRevealSelection = isRangeSelection(selectionRange);
    }
  }

  export function getScrollPosition() {
    if (!editor) return null;
    return { scrollTop: editor.getScrollTop(), scrollLeft: editor.getScrollLeft() };
  }

  export function setScrollPosition(position: { scrollTop: number; scrollLeft: number }) {
    if (!editor) return;
    editor.setScrollPosition(position);
  }

  export function getViewportAnchor(): { topLine: number; scrollLeft: number } | null {
    if (!editor) return null;
    return { topLine: editor.getVisibleRanges()[0]?.startLineNumber ?? 1, scrollLeft: editor.getScrollLeft() };
  }

  export function restoreViewportAnchor(anchor: { topLine: number; scrollLeft: number }): void {
    if (!editor) return;
    editor.setScrollPosition({ scrollTop: editor.getTopForLineNumber(anchor.topLine), scrollLeft: anchor.scrollLeft });
  }

  export function getSelection(): { startLine: number; startColumn: number; endLine: number; endColumn: number } | null {
    const selection = editor?.getSelection();
    return selection ? { startLine: selection.startLineNumber, startColumn: selection.startColumn, endLine: selection.endLineNumber, endColumn: selection.endColumn } : null;
  }

  export function restoreSelection(selection: { startLine: number; startColumn: number; endLine: number; endColumn: number }): void {
    if (!editor || !monaco) return;
    editor.setSelection(new monaco.Selection(selection.startLine, selection.startColumn, selection.endLine, selection.endColumn));
    editor.revealPositionInCenter({ lineNumber: selection.startLine, column: selection.startColumn });
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

  export async function handleFileDrop(event: DragEvent): Promise<void> {
    await handleDrop(event);
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

      const firstTab = initFirstTab();
      bindEditorEvents();

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

      editorRuntimePhase = 'Preparing sample document...';
      await setActiveTab(firstTab, 'initial-example', {
        awaitSnapshotReady: true,
        editorReadOnly: true,
      });

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
      await awaitEditorRuntimeStartupDelay();

      if (freshness.isCurrent()) {
        editorRuntimeReady = true;
        editorRuntimePhase = '';
      }
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
    if (editor && placeholderWidget) {
      editor.removeContentWidget(placeholderWidget);
      placeholderWidget = null;
    }
    if (editor) {
      editor.dispose();
      editor = null;
    }
    if (model) {
      model.dispose();
      model = null;
    }
    tabRuntime?.disposeAll();
    hoverPreviewDisposable?.dispose();
    hoverPreviewDisposable = null;
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
    if (mutation.type === 'replaceSourceText') {
      setEditorValue(mutation.payload.text);
    }
  }

  $: if (monaco && $settings) {
    editorRuntimeController.applyTheme(monaco);
  }

  $: if (!fullEditUiStateValue.active && queuedProgrammaticSourceText !== null) {
    const nextValue = queuedProgrammaticSourceText;
    queuedProgrammaticSourceText = null;
    if (nextValue !== getDocumentSessionState().sourceText) {
      sourceText.set(nextValue);
    }
  }

  export async function ensureReady(): Promise<void> {
    while (!editor || !model || !editorRuntimeReady) {
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
  }

  export async function waitForIdle(): Promise<void> {
    await ensureReady();
    while (editorFullEditController.isImportActive() || fullEditUiStateValue.active) {
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
  }
</script>

<div class="contents">
  <EditorDropZone
    bind:this={dropZone}
    onDrop={handleDrop}
    onDragOver={handleDragOver}
    onPointerDownCapture={handleEditorPointerDownCapture}
    loading={editorRuntimeOverlay.loading}
    loadingPhase={editorRuntimeOverlay.phase}
  />
</div>

<style>
  :global(.treease-editor-placeholder) {
    color: #7b8794;
    font: 13px/20px ui-monospace, SFMono-Regular, Menlo, monospace;
    pointer-events: auto;
    white-space: nowrap;
  }

  :global(.treease-editor-placeholder__link) {
    padding: 1px 3px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 2px;
  }

  :global(.treease-editor-placeholder__row) {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }

  :global(.treease-editor-placeholder__row::before) {
    width: 6px;
    height: 6px;
    flex: 0 0 6px;
    border-radius: 999px;
    background: var(--accent);
    content: '';
  }

  :global(.treease-editor-placeholder__link:hover),
  :global(.treease-editor-placeholder__link:focus-visible) {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
</style>
