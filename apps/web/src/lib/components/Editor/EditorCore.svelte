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
  import { treeState } from '../../store/graph-selection-store';
  import { captureActiveSidecarTarget, captureSidecarTarget, updateSidecarTempModel } from '../../store/sidecar-tab-state';
  import {
    initialFullEditUiState,
    jsonBlockSelection,
    type JsonBlockSelection,
  } from '../../store/full-edit-ui-store';
  import { activeFullEditUiState as fullEditUiState, getActiveFullEditUiState } from '../../store/active-full-edit-ui-store';
  import {
    getWorkspaceRawState,
    getWorkspaceState,
    setWorkspaceState,
    transitionWorkspaceTabDocument,
    updateWorkspaceTab,
  } from '../../store/workspace-store';
  import type { PathSeg } from '../../store/tree-path';
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
  import { getActiveDocumentText, resolveCommitBaseSnapshotId } from '../../store/active-document-authority';
  import { getWorkspaceSnapshotId } from '../../store/workspace-store';
  import { resolvePathSelectionRangeResult } from '../../services/TreePathService';
  import { resolveEditorRuntimeOverlay, type RuntimeStateEventDetail } from '../../runtime-loading';
  import {
    markCursorPathRequested,
    markCursorPathSettled,
  } from '../../test-bridge/runtime-readiness';

  import EditorDropZone from './EditorDropZone.svelte';
  import { shouldSyncGraphHighlightFromCursorReason } from './EditorCore.graph-highlight';
  import { ensureModelDocumentKey } from './document-key';
  import { registerEditorHoverPreview } from './editor-hover';
  import { createEditorAnalysisController } from './editor-analysis-controller';
  import { createEditorFormatController, type FormatCommandTarget } from './editor-format-controller';
  import { createEditorFullEditController } from './editor-full-edit-controller';
  import { createWorkspaceTabFullEditSink } from './editor-full-edit-sink';
  import { createEditorRuntimeController } from './editor-runtime-controller';
  import { createEditorPlaceholderController } from './editor-placeholder';
  import { commitEditorTabTextChange } from './editor-tab-edit-commit';
  import { settleWholeDocumentReplacement } from './whole-document-replacement';
  import type { EditorModelWithDocumentKey } from './types';
  import { EditorTabRuntime } from './editor-tab-runtime';
  import {
    activateWorkspaceTabTransition,
    closeWorkspaceTabTransition,
    createWorkspaceTabTransition,
    type EditorWorkspaceTab,
  } from '../../store/editor-workspace';
  import { EDITOR_CONFIG } from '../../config/constants';
  import { monacoChangesToDocumentTextEdits, type MonacoTextChange } from '../../../shared/document-text-edits';
  import { createTreeaseMonacoEditorOptions } from './editor-options';
  import type { DocumentOrigin } from '../../document-origin';
  import type { SharedWorkspaceMutationTarget } from '../../share/share-workspace-lifecycle';

  export let onScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {};
  export let onNavigation: (path: PathSeg[], target: 'key' | 'value' | 'node') => void = () => {};
  export let synchronizedRuntimeLoading = false;
  export let runBidirectionalEdit: <T>(source: string, execute: () => Promise<T>, reason?: string) => Promise<T> = async (_source, execute) => execute();
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => Promise<void> = async () => {};
  export let onDirectDraftMutation: (target: SharedWorkspaceMutationTarget) => void = () => {};
  export let ensureSharedWorkspacePromoted: (target: SharedWorkspaceMutationTarget) => Promise<boolean> = async () => true;

  type WholeDocumentReplacementOptions = {
    sourceWritebackPolicy?: 'intake' | 'submitted';
    effectiveEnableNest?: boolean;
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
  let jsonBlockSelectionUnsub: (() => void) | null = null;

  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback;
  let lastMutationId = 0;
  let diffDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let jsonBlockDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let diffBlankZoneIds: string[] = [];
  let suppressGraphHighlightSync = 0;
  let userSelectionGesture = false;
  let suppressTreePathUpdate = 0;
  let unfocusedExternalRevealSelection = false;
  let wholeDocumentReplacementToken = 0;
  let formattingOptionsValue;
  let suppressNextWholeDocumentAutoGuess = false;
  const programmaticWholeDocumentReplacementByTabId = new Map<
    string,
    { model: Monaco.editor.ITextModel; text: string }
  >();
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

  function sharedMutationTarget(
    tabId: string,
    targetModel: Monaco.editor.ITextModel,
    documentKey: string,
  ): SharedWorkspaceMutationTarget {
    return {
      tabId,
      documentKey,
      readDocumentKey: () => getWorkspaceState().tabsById[tabId]?.documentKey ?? '',
      readText: () => targetModel.getValue(),
      isCurrent: () => {
        const tab = getWorkspaceState().tabsById[tabId];
        return (
          !targetModel.isDisposed() &&
          tabRuntime?.get(tabId) === targetModel &&
          Boolean(tab)
        );
      },
    };
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
    editor.focus();
    const position = editor.getPosition();
    if (position && monaco) {
      editor.setSelection(new monaco.Selection(position.lineNumber, position.column, position.lineNumber, position.column));
      updateCurrentTempModel((current) => ({ ...current, selectionLength: 0 }));
      queueMicrotask(() => updateCurrentTempModel((current) => ({ ...current, selectionLength: 0 })));
    }
  }

  const getNestEnabled = () => $settings.parser.enableNest;
  /** Main-editor interaction projects UI-only selection state to its paired sidecar. */
  function updateCurrentTempModel(updater: (current: any) => any): void {
    const target = captureActiveSidecarTarget();
    if (target) updateSidecarTempModel(target, updater);
  }

  function updateMainTabSidecarTempModel(tabId: string, updater: (current: any) => any): void {
    const main = getWorkspaceState().tabsById[tabId];
    const target = main?.sidecarTabId ? captureSidecarTarget(main.sidecarTabId) : null;
    if (target) updateSidecarTempModel(target, updater);
  }
  function clearDocumentSemanticTokens(documentKey: string | undefined): void {
    clearSemanticTokensForDocument(documentKey);
  }

  function refreshDocumentSemanticTokens(languageId: string | undefined): void {
    refreshSemanticTokensForLanguage(languageId);
  }
  const callWasmWorkerFromEditor = <T>(method: string, input: unknown) =>
    callSharedWasmWorker<T>(method as any, input);

  let fullEditUiStateValue = $fullEditUiState;
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

  const editorPlaceholder = createEditorPlaceholderController({
    getEditor: () => editor,
    getModel: () => model,
    getMonaco: () => monaco,
    getLanguage: () => languageIdValue,
    onRequestImportFile: (payload) => onRequestImportFile(payload),
    onLoadExample: (example) => {
      setActiveTabOrigin('example');
      queueWholeDocumentReplacement(example, {
        sourceWritebackPolicy: 'intake',
        formatSourceOnClose: false,
        shouldResolveLanguage: false,
        markUserInput: false,
      });
      editor?.focus();
    },
  });

  let tabRuntime: EditorTabRuntime;
  const fullEditControllersByTabId = new Map<string, ReturnType<typeof createEditorFullEditController>>();
  let tabSequence = 1;
  let dropZone: EditorDropZone;

  function isTabActive(tabId: string, target: Monaco.editor.ITextModel | null = null): boolean {
    return getWorkspaceState().activeTabId === tabId && (!target || (model === target && editor?.getModel() === target));
  }

  function prepareTabFullEditUi(tabId: string, payload: {
    documentKey: string;
    revision: number;
    language: SupportedEditorLanguageId;
    reason: 'whole-document-replacement';
  }): void {
    updateWorkspaceTab(tabId, {
      fullEditUiState: {
        ...initialFullEditUiState,
        active: true,
        documentKey: payload.documentKey,
        revision: payload.revision,
        language: payload.language,
        phase: 'preparing',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: payload.reason,
      },
    });
  }

  function cancelPreparedTabFullEditUi(tabId: string, documentKey: string, revision: number): void {
    const current = getWorkspaceState().tabsById[tabId]?.fullEditUiState;
    if (
      !current?.active ||
      current.phase !== 'preparing' ||
      current.documentKey !== documentKey ||
      current.revision !== revision
    ) {
      return;
    }
    updateWorkspaceTab(tabId, { fullEditUiState: initialFullEditUiState });
  }

  function updateTabSourceText(tabId: string, value: string): void {
    updateWorkspaceTab(tabId, { sourceText: value });
    if (isTabActive(tabId)) sourceText.set(value);
  }

  function transitionTabDocumentForOperation(
    tabId: string,
    documentKey: string,
    language: SupportedEditorLanguageId,
  ): number {
    const workspace = getWorkspaceState();
    const tab = workspace.tabsById[tabId];
    const targetModel = tabRuntime?.get(tabId) ?? null;
    if (!tab || tab.role === 'sidecar' || tab.role === 'column-detail-draft') return 0;
    const revision = tab.revision + 1;
    const transitioned = transitionWorkspaceTabDocument({
      tabId,
      expected: { documentKey: tab.documentKey, languageId: tab.languageId, revision: tab.revision },
      next: {
        documentKey,
        languageId: language,
        revision,
        sourceText: targetModel?.getValue() ?? tab.sourceText,
      },
    });
    if (!transitioned) return 0;
    setModelDocumentKey(targetModel, documentKey);
    if (isTabActive(tabId, targetModel)) documentKeyStore.set(documentKey);
    return revision;
  }

  function setTabOperationLanguage(tabId: string, language: SupportedEditorLanguageId): void {
    const workspace = getWorkspaceState();
    const tab = workspace.tabsById[tabId];
    const targetModel = tabRuntime?.get(tabId) ?? null;
    if (!tab || tab.role === 'sidecar' || tab.role === 'column-detail-draft' || tab.languageId === language) return;
    if (isTabActive(tabId, targetModel)) {
      languageIdStore.set(language);
    } else {
      transitionWorkspaceTabDocument({
        tabId,
        expected: { documentKey: tab.documentKey, languageId: tab.languageId, revision: tab.revision },
        next: { documentKey: tab.documentKey, languageId: language, revision: tab.revision, sourceText: tab.sourceText },
      });
      if (monaco && targetModel) monaco.editor.setModelLanguage(targetModel, language);
    }
  }

  function getFullEditController(tabId: string) {
    const existing = fullEditControllersByTabId.get(tabId);
    if (existing) return existing;
    const fullEditSink = createWorkspaceTabFullEditSink(tabId);
    const controller = createEditorFullEditController({
      getModel: () => tabRuntime?.get(tabId) ?? null,
      getEditor: () => {
        const target = tabRuntime?.get(tabId) ?? null;
        return isTabActive(tabId, target) ? editor : null;
      },
      getMonaco: () => monaco,
      getLanguageId: () => getWorkspaceState().tabsById[tabId]?.languageId ?? editorLanguageFallback,
      getNestEnabled,
      getGraphBuilderConfig: () => buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
      getFullEditUiState: fullEditSink.getState,
      isDocumentCurrent: (target) => {
        const tab = getWorkspaceState().tabsById[tabId];
        return (
          tabRuntime?.get(tabId) === target.model &&
          tab?.documentKey === target.documentKey &&
          tab.languageId === target.language &&
          tab.revision === target.revision
        );
      },
      fullEditSink,
      rotateActiveDocumentKey: () => {
        const tab = getWorkspaceState().tabsById[tabId];
        return tab ? `${tab.documentKey}:${Date.now()}` : '';
      },
      setModelDocumentKey,
      setActiveTabDocumentKey: (documentKey, language) => {
        transitionTabDocumentForOperation(tabId, documentKey, language);
      },
      clearSemanticTokensForDocument: clearDocumentSemanticTokens,
      setEditorValue: (value) => {
        const target = tabRuntime?.get(tabId) ?? null;
        if (!target || target.getValue() === value) return false;
        target.setValue(value);
        updateTabSourceText(tabId, value);
        return true;
      },
      setEditorValueForFullEdit: (value) => {
        const target = tabRuntime?.get(tabId) ?? null;
        if (!target) return false;
        const changed = target.getValue() !== value;
        if (isTabActive(tabId, target)) isStoreUpdateSuppressed = true;
        if (changed) target.setValue(value);
        updateTabSourceText(tabId, value);
        if (isTabActive(tabId, target)) {
          syncLastModelSnapshot();
          releaseStoreUpdateSuppression();
        }
        return changed;
      },
      setSourceText: (value) => updateTabSourceText(tabId, value),
      setDocumentKey: (documentKey) => {
        const target = tabRuntime?.get(tabId) ?? null;
        setModelDocumentKey(target, documentKey);
        if (isTabActive(tabId, target)) documentKeyStore.set(documentKey);
      },
      applyImportLanguage: (language) => setTabOperationLanguage(tabId, language),
      getFormattingOptions: () => formattingOptionsValue,
      callWasmWorker: callWasmWorkerFromEditor,
      updateActiveTempModel: (updater) => {
        updateMainTabSidecarTempModel(tabId, updater);
      },
      commitEditorState: () => getWorkspaceState().tabsById[tabId]?.revision ?? 0,
      applyGraphAnalysis: async (requestModel, requestLanguage, requestDocumentKey, revision, analysis) => {
        if (!isTabActive(tabId, requestModel)) return;
        await editorAnalysisController.applyGraphAnalysis(requestModel, requestLanguage, requestDocumentKey, revision, analysis);
      },
      triggerGraphSync: (position) => {
        const target = tabRuntime?.get(tabId) ?? null;
        if (!isTabActive(tabId, target) || !position) return;
        void editorAnalysisController.updateTreePath(position, {
          syncGraphHighlight: true,
        });
      },
      runBidirectionalEdit,
      beforeDocumentMutation: ({ model: targetModel }) => {
        const tab = getWorkspaceState().tabsById[tabId];
        if (!tab || tabRuntime?.get(tabId) !== targetModel) return Promise.resolve(false);
        return ensureSharedWorkspacePromoted(sharedMutationTarget(tabId, targetModel, tab.documentKey));
      },
    });
    fullEditControllersByTabId.set(tabId, controller);
    return controller;
  }

  function getActiveFullEditController() {
    const tabId = getWorkspaceState().activeTabId;
    return tabId ? getFullEditController(tabId) : null;
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
      editorPlaceholder.update();
    }
  }

  const editorAnalysisController = createEditorAnalysisController({
    getMonaco: () => monaco,
    getEditor: () => editor,
    getModel: () => model,
    getDocumentKey,
    getLanguageId: () => languageIdValue,
    getNestEnabled,
    getEditorRevision: () => $editorRevision,
    isImportActive: () => getActiveFullEditController()?.isImportActive() ?? false,
    getSourceText: () => model?.getValue() ?? '',
    getJsonBlockSelection: () => jsonBlockSelectionValue,
    setJsonBlockSelection: (selection) => jsonBlockSelection.set(selection),
    updateActiveTempModel: updateCurrentTempModel,
    publishNavigation: onNavigation,
    setTreeState: (value) => treeState.set(value),
    primeSemanticTokensForDocument: (documentKey, semanticTokens) =>
      primeSemanticTokensForDocument(documentKey, semanticTokens),
    clearSemanticTokensForDocument: clearDocumentSemanticTokens,
    refreshSemanticTokensForLanguage: refreshDocumentSemanticTokens,
    markCursorPathRequested,
    markCursorPathSettled,
  });

  const editorFormatController = createEditorFormatController({
    getActiveTarget: () => {
      const tabId = getWorkspaceState().activeTabId;
      const tab = getWorkspaceState().tabsById[tabId];
      const target = tabRuntime?.get(tabId) ?? null;
      if (!tab || !target) return null;
      return { tabId, model: target, documentKey: tab.documentKey, revision: tab.revision, languageId: tab.languageId };
    },
    getFormattingOptions: () => formattingOptionsValue,
    getNestEnabled,
    isImportActive: (target) => getFullEditController(target.tabId).isImportActive(),
    isTargetCurrent: (target) => {
      const tab = getWorkspaceState().tabsById[target.tabId];
      return (
        tabRuntime?.get(target.tabId) === target.model &&
        tab?.documentKey === target.documentKey &&
        tab.languageId === target.languageId &&
        tab.revision === target.revision
      );
    },
    isTargetVisible: (target) => isTabActive(target.tabId, target.model),
    callWasmWorker: callWasmWorkerFromEditor,
    replaceWholeDocumentText: (target, value, kind) =>
      replaceWholeDocumentTextForTarget(target, value, {
        sourceWritebackPolicy: 'intake',
        formatSourceOnClose: kind === 'sort',
        shouldResolveLanguage: false,
        markUserInput: true,
      }),
    resetEditorCursorToStart: () => resetEditorCursorToStart(),
  });

  const editorRuntimeController = createEditorRuntimeController({
    getSettings: () => $settings,
    getThemeName: () => themeName,
    isImportActive: () => getActiveFullEditController()?.isImportActive() ?? false,
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
    editorRuntimeError,
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

  function setupLanguageSubscription(
    ensureSemanticTokensProvider: (lang: string) => void,
    ensureDocumentColorProvider: (lang: string) => void,
  ) {
    // This subscription projects committed language state into Monaco/UI only.
    // Language commands and imports own their full-edit transaction upstream.
    languageUnsub = languageIdStore.subscribe((value) => {
      const nextValue = value || editorLanguageFallback;
      if (!supportedEditorLanguageSet.has(nextValue as SupportedEditorLanguageId)) {
        const message = `Unsupported editor language: ${nextValue}`;
        updateCurrentTempModel((current) => ({ ...current, error: message }));
        throw new Error(message);
      }
      const next = nextValue as SupportedEditorLanguageId;
      languageIdValue = next;
      updateCurrentTempModel((current) => ({ ...current, error: '' }));
      ensureLanguageRegistered(next);
      ensureSemanticTokensProvider(next);
      ensureDocumentColorProvider(next);
      if (model && monaco) {
        monaco.editor.setModelLanguage(model, next);
        syncColorViewportState('language');
      }
      const activeId = getWorkspaceState().activeTabId;
      if (activeId) updateWorkspaceTab(activeId, { languageId: next });
      if (next !== 'json') {
        jsonBlockSelection.set(null);
      }
      if (!treePathLanguages.has(next)) {
        updateCurrentTempModel((current) => ({ ...current, treePath: [], graphHighlight: null }));
      } else if (activeId && model) {
        void editorAnalysisController.updateTreePath(editor?.getPosition() ?? null, {
          syncGraphHighlight: false,
        });
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
    editorPlaceholder.update();
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
        getVisibleStartLine: () => editor?.getVisibleRanges()[0]?.startLineNumber ?? 1,
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
    if (getActiveFullEditController()?.isImportActive()) return;
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
      // Monaco sometimes reports a real pointer selection with reason=NotSet.
      // The pointer gesture is the proof; focus alone is never sufficient.
      userSelectionGesture = true;
      if (!unfocusedExternalRevealSelection || !editor || !model || !monaco) return;
      unfocusedExternalRevealSelection = false;
      const position = event.target.position ?? editor.getPosition();
      if (!position) return;
      if (!model || !monaco) return;
      editor.setSelection(new monaco.Selection(position.lineNumber, position.column, position.lineNumber, position.column));
      editor.setPosition(position);
    });

    editor.onDidChangeModelContent((event) => {
      const activeModel = model;
      if (!activeModel) return;
      editorPlaceholder.update();
      const previousLength = lastModelLength;
      const previousText = lastModelText;
      const nextText = activeModel.getValue();
      const changes = (event as unknown as { changes?: MonacoTextChange[] }).changes ?? [];
      const isFlush = (event as unknown as { isFlush?: boolean }).isFlush ?? false;
      syncColorViewportState('content');
      isStoreUpdateSuppressed = true;
      notifyCompareEdit();
      const programmaticReplacement = programmaticWholeDocumentReplacementByTabId.get(getWorkspaceState().activeTabId);
      if (programmaticReplacement?.model === activeModel && programmaticReplacement.text === nextText) {
        programmaticWholeDocumentReplacementByTabId.delete(getWorkspaceState().activeTabId);
        syncLastModelSnapshot();
        releaseStoreUpdateSuppression();
        return;
      }
      const activeFullEditController = getActiveFullEditController();
      if (activeFullEditController?.isImportActive()) {
        if (activeFullEditController.isActiveSessionText(nextText)) {
          syncLastModelSnapshot();
          releaseStoreUpdateSuppression();
          return;
        }
        activeFullEditController.cancelImportStream();
      }
      const mutationTabId = getWorkspaceState().activeTabId;
      const mutationDocumentKey = getWorkspaceState().tabsById[mutationTabId]?.documentKey ?? '';
      if (changes.length > 0 && !isFlush && nextText !== previousText && mutationDocumentKey) {
        onDirectDraftMutation(sharedMutationTarget(mutationTabId, activeModel, mutationDocumentKey));
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
        const targetTabId = getWorkspaceState().activeTabId;
        const targetTab = getWorkspaceState().tabsById[targetTabId];
        const targetDocumentKey = targetTab ? `${targetTab.documentKey}:${Date.now()}` : '';
        const transitionedRevision = targetTab
          ? transitionTabDocumentForOperation(targetTabId, targetDocumentKey, targetTab.languageId)
          : 0;
        if (!transitionedRevision) {
          releaseStoreUpdateSuppression();
          return;
        }
        if (activeFullEditController?.suppressNextWholeDocumentIntake()) {
          releaseStoreUpdateSuppression();
          return;
        }
        updateCurrentTempModel((current) => ({
          ...current,
          treePath: [],
          graphHighlight: null,
        }));
        sourceText.set(nextText);
        const documentKeyValue = getDocumentKey();
        const requestModel = activeModel;
        const currentLanguage = languageIdValue;
        const preparedRevision = transitionedRevision;
        const replacementToken = ++wholeDocumentReplacementToken;
        const sourceWritebackPolicy = 'intake';
        const formatSourceOnClose = true;
        const shouldResolveLanguage =
          !shouldSkipWholeDocumentAutoGuess && wholeDocumentReplacement.text.trim().length >= 8;
        const shouldMarkUserInput = true;
        const skipUsageMetering = false;
        prepareTabFullEditUi(targetTabId, {
          documentKey: documentKeyValue,
          revision: preparedRevision,
          language: currentLanguage,
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
            model: tabRuntime?.get(targetTabId) ?? null,
            documentKey: getWorkspaceState().tabsById[targetTabId]?.documentKey ?? '',
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
            if (language !== currentLanguage && isTabActive(targetTabId, requestModel)) {
              toast.success(`Detected ${language.toUpperCase()} input`);
            }
          },
          commitWholeDocumentReplacement: async (language) => {
            if (shouldMarkUserInput) {
              userInputByTabId.set(targetTabId, true);
              updateWorkspaceTab(targetTabId, { origin: 'user' });
            }
            await getFullEditController(targetTabId).startFullEditSession({
              language,
              text: nextText,
              reason: 'whole-document-replacement',
              sourceWritebackPolicy,
              formatSourceOnClose,
              documentKey: documentKeyValue,
              documentTransitioned: true,
              isFresh: isReplacementCurrent,
              skipUsageMetering,
            });
          },
        })
          .catch((error) => {
            if (!isReplacementCurrent()) return;
            const message = error instanceof Error ? error.message : String(error);
            console.error('[editor] whole-document replacement failed', error);
            updateMainTabSidecarTempModel(targetTabId, (current) => ({ ...current, error: message }));
            if (isTabActive(targetTabId, requestModel)) toast.error('Graph rebuild failed');
          })
          .finally(() => {
            cancelPreparedTabFullEditUi(targetTabId, documentKeyValue, preparedRevision);
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
      updateCurrentTempModel((current) => ({
        ...current,
        cursor: `Ln ${position.lineNumber}, Col ${position.column}`,
        selectionLength,
      }));
      if (suppressTreePathUpdate > 0) return;
      void editorAnalysisController.updateTreePath(position, {
        syncGraphHighlight,
      });
    };

    editor.onDidChangeCursorPosition((event) => {
      updateCursorStatus(
        event.position,
        editor?.getSelection() ?? null,
        suppressGraphHighlightSync === 0 && (shouldSyncGraphHighlightFromCursorReason(event.reason) || userSelectionGesture),
      );
      userSelectionGesture = false;
    });
    editor.onDidChangeCursorSelection((event) => {
      const position = editor?.getPosition() ?? event.selection.getPosition();
      updateCursorStatus(
        position,
        event.selection,
        suppressGraphHighlightSync === 0 && (shouldSyncGraphHighlightFromCursorReason(event.reason) || userSelectionGesture),
      );
      userSelectionGesture = false;
    });
    editor.onDidScrollChange((event) => {
      onScroll({ scrollTop: event.scrollTop, scrollLeft: event.scrollLeft });
      syncColorViewportState('scroll');
    });
  }

  function bindStoreSubscriptions() {
    storeUnsub = sourceText.subscribe((value) => {
      if (!model || isStoreUpdateSuppressed) return;
      if (getActiveFullEditUiState().active) return;
      if (value !== model.getValue()) {
        model.setValue(value);
      }
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
      onError: (error) => {
        if (isExpectedDocumentTaskTermination(error)) {
          console.debug('[editor] text commit ended before landing', error);
          return;
        }
        reportEditorDocumentTaskError(error, getWorkspaceState().activeTabId, requestModel);
      },
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
    options: WholeDocumentReplacementOptions = {},
  ): boolean {
    const tabId = getWorkspaceState().activeTabId;
    const tab = getWorkspaceState().tabsById[tabId];
    const target = tabRuntime?.get(tabId) ?? null;
    if (!tab || !target) return setEditorValue(value);
    if (target.getValue() === value) return false;
    void replaceWholeDocumentTextForTarget(
      { tabId, model: target, documentKey: tab.documentKey, revision: tab.revision, languageId: tab.languageId },
      value,
      options,
    );
    return true;
  }

  async function replaceWholeDocumentTextForTarget(
    target: FormatCommandTarget,
    value: string,
    options: WholeDocumentReplacementOptions,
  ): Promise<boolean> {
    const tab = getWorkspaceState().tabsById[target.tabId];
    if (
      !tab ||
      tabRuntime?.get(target.tabId) !== target.model ||
      tab.documentKey !== target.documentKey ||
      tab.languageId !== target.languageId ||
      tab.revision !== target.revision
    ) {
      return false;
    }

    const isVisible = isTabActive(target.tabId, target.model);
    if (!(await ensureSharedWorkspacePromoted(sharedMutationTarget(target.tabId, target.model, target.documentKey)))) {
      return false;
    }
    const currentTab = getWorkspaceState().tabsById[target.tabId];
    if (
      !currentTab ||
      tabRuntime?.get(target.tabId) !== target.model ||
      currentTab.documentKey !== target.documentKey ||
      currentTab.languageId !== target.languageId ||
      currentTab.revision !== target.revision
    ) {
      return false;
    }
    programmaticWholeDocumentReplacementByTabId.set(target.tabId, { model: target.model, text: value });
    target.model.setValue(value);
    if (!isVisible) programmaticWholeDocumentReplacementByTabId.delete(target.tabId);

    const nextDocumentKey = `${target.documentKey}:${Date.now()}`;
    const revision = await getFullEditController(target.tabId).startFullEditSession({
      language: target.languageId,
      text: value,
      reason: 'whole-document-replacement',
      sourceWritebackPolicy: options.sourceWritebackPolicy,
      effectiveEnableNest: options.effectiveEnableNest,
      formatSourceOnClose: options.formatSourceOnClose,
      documentKey: nextDocumentKey,
      isFresh: () => tabRuntime?.get(target.tabId) === target.model && !target.model.isDisposed(),
      skipUsageMetering: options.skipUsageMetering,
    });
    if (revision > 0 && options.markUserInput) {
      userInputByTabId.set(target.tabId, true);
      updateWorkspaceTab(target.tabId, { origin: 'user' });
    }
    return revision > 0;
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
    editorPlaceholder.update();
    syncColorViewportState('model');
    lastModelLength = model.getValue().length;
    lastModelText = model.getValue();
    languageIdStore.set(tab.languageId);
    setActiveEditorIo();
    return { model, text };
  }

  function isExpectedDocumentTaskTermination(error: unknown): boolean {
    const message = error instanceof Error ? error.message : String(error);
    return /stale|cancel|disposed|dispose|tab.+(switch|change)|no longer fresh/i.test(message);
  }

  function reportEditorDocumentTaskError(
    error: unknown,
    tabId: string,
    requestModel: Monaco.editor.ITextModel,
  ): void {
    if (!isExpectedDocumentTaskTermination(error)) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('[editor] document task failed', { tabId, message, error });
      updateMainTabSidecarTempModel(tabId, (current) => ({ ...current, error: message }));
      if (isTabActive(tabId, requestModel)) toast.error('Unable to update the document view. You can keep editing and try again.');
      return;
    }
    console.debug('[editor] document task ended before landing', error);
  }

  async function startInstalledActiveTab(
    tab: EditorWorkspaceTab,
    installed: InstalledActiveTab,
    reason: 'initial-example' | 'tab-reactivate',
    options: { awaitSnapshotReady?: boolean; editorReadOnly?: boolean } = {},
  ): Promise<boolean> {
    const requestModel = installed.model;
    const controller = getFullEditController(tab.id);
    if (controller.isImportActive()) {
      releaseStoreUpdateSuppression();
      return true;
    }
    const fullEditRequest = {
      language: tab.languageId,
      text: installed.text,
      reason,
      // Re-activating a resident tab is a view change, not a document
      // replacement. Rotating its document key here also resets its paired
      // sidecar, which makes right-pane state appear to leak between tabs.
      documentKey: tab.documentKey,
      documentTransitioned: true,
      editorReadOnly: options.editorReadOnly ?? false,
      // Tab activation changes only the visible projection. The operation is
      // still current while its resident model belongs to this tab, even when
      // another tab is active.
      isFresh: () => tabRuntime?.get(tab.id) === requestModel && !requestModel.isDisposed(),
    };
    try {
      if (options.awaitSnapshotReady) {
        // Retained for callers that explicitly need a terminal outcome. Startup
        // never uses this path: SnapshotReady is not Editor readiness.
        const outcome = await controller.runFullEditSessionToTerminal(fullEditRequest);
        if (outcome.status !== 'completed' || outcome.snapshotId == null) {
          reportEditorDocumentTaskError(
            new Error(`Document task ended without SnapshotReady: ${outcome.status}`),
            tab.id,
            requestModel,
          );
          return false;
        }
      } else {
        void controller.startFullEditSession(fullEditRequest).catch((error) => {
          reportEditorDocumentTaskError(error, tab.id, requestModel);
        });
      }
      return true;
    } catch (error) {
      reportEditorDocumentTaskError(error, tab.id, requestModel);
      return false;
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
    // Installing the Monaco model is synchronous. Document intake is deliberately
    // started independently so a parse/Snapshot failure cannot block editing.
    return startInstalledActiveTab(tab, installed, reason, options);
  }

  export function addTab() {
    if (!monaco) return;
    const id = `tab-${Date.now()}-${tabSequence++}`;
    const transition = createWorkspaceTabTransition(getWorkspaceRawState(), { id, name: 'Untitled', documentKey: `${id}:0`, languageId: languageIdValue, sourceText: '', origin: 'user' });
    const tab = transition?.workspace.tabsById[id];
    if (tab && transition) {
      // Install the model before publishing the new active workspace tab.
      const installed = installActiveTab(tab);
      if (!installed) return;
      setWorkspaceState(transition.workspace);
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
    if (!monaco) return null;
    const id = `tab-${Date.now()}-${tabSequence++}`;
    const transition = createWorkspaceTabTransition(getWorkspaceRawState(), { id, name: payload.name, documentKey: `${id}:0`, languageId: payload.languageId, sourceText: payload.text, origin: payload.origin ?? 'import', fileLinkedDocument: payload.fileLinkedDocument, savedText: payload.fileLinkedDocument ? payload.text : undefined });
    const tab = transition?.workspace.tabsById[id];
    if (!tab) return null;
    userInputByTabId.set(tab.id, true);
    const installed = installActiveTab(tab);
    if (!installed) return null;
    setWorkspaceState(transition.workspace);
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
    languageIdStore.set(payload.languageId);
    queueWholeDocumentReplacement(payload.text, { skipUsageMetering: payload.skipUsageMetering });
    setActiveTabOrigin(payload.origin ?? 'import');
  }

  export async function replaceDocumentFromFile(payload: { tabId: string; text: string; languageId: SupportedEditorLanguageId }): Promise<boolean> {
    const tab = getWorkspaceState().tabsById[payload.tabId];
    if (!tab || !monaco || !tabRuntime) return false;
    setTabOperationLanguage(payload.tabId, payload.languageId);
    const current = getWorkspaceState().tabsById[payload.tabId];
    const targetModel = current ? tabRuntime.getOrCreate(current) : null;
    if (!current || !targetModel) return false;
    return replaceWholeDocumentTextForTarget(
      {
        tabId: payload.tabId,
        model: targetModel,
        documentKey: current.documentKey,
        revision: current.revision,
        languageId: current.languageId,
      },
      payload.text,
      {
        sourceWritebackPolicy: 'intake',
        formatSourceOnClose: true,
        markUserInput: true,
      },
    );
  }

  export function renameDocument(tabId: string, name: string): void {
    updateWorkspaceTab(tabId, { name });
  }

  export function closeTab(id: string) {
    const workspace = getWorkspaceRawState();
    const wasActive = workspace.activeTabId === id || workspace.primaryTabId === id || workspace.paneTabIds.left === id;
    const blankId = `tab-${Date.now()}-${tabSequence++}`;
    const transition = closeWorkspaceTabTransition(workspace, id, { id: blankId, documentKey: `${blankId}:0`, name: 'Untitled', languageId: languageIdValue });
    if (!transition) return;
    // Invalidate the closed tab synchronously before its model leaves the
    // runtime. The controller owns its Job/RAF/conversion cleanup.
    fullEditControllersByTabId.get(id)?.dispose();
    fullEditControllersByTabId.delete(id);
    userInputByTabId.delete(id);
    const nextTab = transition.workspace.tabsById[transition.effect.tabId];
    if (!nextTab) return;
    if (!wasActive) {
      setWorkspaceState(transition.workspace);
      if (transition.effect.disposeTabId) tabRuntime.dispose(transition.effect.disposeTabId);
      return;
    }
    // Install successor before releasing the removed model; editorIO must never observe a disposed active document.
    const installed = installActiveTab(nextTab);
    if (!installed) return;
    setWorkspaceState(transition.workspace);
    if (transition.effect.kind === 'activate-new-blank') commitEditorState();
    void startInstalledActiveTab(nextTab, installed, 'tab-reactivate');
    if (transition.effect.disposeTabId) tabRuntime.dispose(transition.effect.disposeTabId);
  }

  export function activateTab(id: string) {
    const transition = activateWorkspaceTabTransition(getWorkspaceRawState(), id);
    const tab = transition?.workspace.tabsById[id];
    if (tab && transition) {
      const installed = installActiveTab(tab);
      if (!installed) return;
      setWorkspaceState(transition.workspace);
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

  export async function changeLanguage(nextLanguage: SupportedEditorLanguageId): Promise<boolean> {
    const tabId = getWorkspaceState().activeTabId;
    const tab = getWorkspaceState().tabsById[tabId];
    const targetModel = tabRuntime?.get(tabId) ?? null;
    if (!tab || !targetModel || tab.languageId === nextLanguage) return false;
    if (!supportedEditorLanguageSet.has(nextLanguage)) {
      throw new Error(`Unsupported editor language: ${nextLanguage}`);
    }

    wholeDocumentReplacementToken += 1;
    editorAnalysisController.prepareLanguageSwitchAnalysisReset();
    const revision = await getFullEditController(tabId).startFullEditSession({
      language: nextLanguage,
      text: targetModel.getValue(),
      reason: 'language-switch',
      sourceWritebackPolicy: 'intake',
      formatSourceOnClose: true,
      isFresh: () => tabRuntime?.get(tabId) === targetModel && !targetModel.isDisposed(),
    });
    return revision > 0;
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
        effectiveEnableNest: false,
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
        effectiveEnableNest: false,
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
    getActiveFullEditController()?.cancelImportStream();
  }

  export async function importStream(
    file: File,
    sourceLanguage: string,
    targetLanguage: SupportedEditorLanguageId = languageIdValue,
  ): Promise<void> {
    markActiveTabUserInput(true);
    await getActiveFullEditController()?.importStream(file, sourceLanguage, 'import-file', targetLanguage);
  }

  export async function importAs(targetFormat: string, text: string, sourceFormat: string) {
    if (!model || !monaco) return;
    const nextLanguage = supportedEditorLanguageSet.has(sourceFormat as SupportedEditorLanguageId)
      ? (sourceFormat as SupportedEditorLanguageId)
      : (targetFormat as SupportedEditorLanguageId);
    if (!supportedEditorLanguageSet.has(nextLanguage)) {
      const message = `Unsupported editor language: ${nextLanguage}`;
      updateCurrentTempModel((current) => ({ ...current, error: message }));
      toast.error(message);
      throw new Error(message);
    }
    suppressNextWholeDocumentAutoGuess = true;
    suppressNextTreePathUpdate();
    markActiveTabUserInput(true);
    languageIdStore.set(nextLanguage);
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
    options: {
      target?: 'key' | 'value' | 'node';
      focus?: boolean;
      isCurrent?: () => boolean;
    } | undefined,
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
    if (options?.isCurrent && !options.isCurrent()) return false;
    const selectionRange = selectionRangeResult.data;
    if (!selectionRange) {
      const message = `Failed to reveal path ${JSON.stringify(path)}`;
      // Clear graphHighlight to prevent the reactive subscription from
      // re-entering — the error update would otherwise keep the reference
      // check alive (H1 !== H2) and loop forever.
      updateCurrentTempModel((current) => ({ ...current, error: message, graphHighlight: null }));
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
    try {
      await getActiveFullEditController()?.handleDrop(event);
    } catch (error) {
      if (model) reportEditorDocumentTaskError(error, getWorkspaceState().activeTabId, model);
    }
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


  async function initializeEditorRuntime(): Promise<void> {
    const runtimeToken = ++editorRuntimeToken;
    const freshness = createFreshnessScope({ token: runtimeToken }, () => ({ token: editorRuntimeToken }));
    editorRuntimeReady = false;
    editorRuntimeError = false;
    editorRuntimePhase = 'Loading editor runtime...';

    try {
      // Monaco/model/store/interaction are the only Editor readiness gates.
      const shell = await freshness.step(() => editorRuntimeController.initShell());
      if (!shell || !freshness.isCurrent()) return;

      editorRuntimeController.applyTheme(shell.monaco);
      editorRuntimeController.scheduleWorkerWarmup();
      const firstTab = initFirstTab();
      bindEditorEvents();
      bindStoreSubscriptions();
      void setActiveTab(firstTab, 'initial-example').catch((error) => {
        if (model) reportEditorDocumentTaskError(error, firstTab.id, model);
      });

      if (editor && monaco) {
        editor.updateOptions({ readOnly: false });
        hoverPreviewDisposable?.dispose();
        hoverPreviewDisposable = registerEditorHoverPreview({
          monaco,
          editor,
          getTreeState: () => $treeState,
          getRevision: () => editorRevisionValue,
          getDocumentKey,
          getLanguageId: () => languageIdValue,
          getNestEnabled: () => $settings.parser.enableNest,
          isImportActive: () => getActiveFullEditController()?.isImportActive() ?? false,
        });
      }

      // Publish readiness before language services or the first Snapshot settle.
      if (freshness.isCurrent()) {
        editorRuntimeReady = true;
        editorRuntimePhase = '';
      }

      void (async () => {
        try {
          const langServices = await editorRuntimeController.initLanguageServices();
          if (!freshness.isCurrent()) return;

          const ensureSemanticTokensProvider = langServices.ensureSemanticTokensProvider;
          refreshSemanticTokensForLanguage = langServices.refreshSemanticTokens;
          primeSemanticTokensForDocument = langServices.primeSemanticTokens;
          clearSemanticTokensForDocument = langServices.clearSemanticTokens;
          const ensureDocumentColorProvider = langServices.ensureDocumentColorProvider;
          updateDocumentColorViewport = langServices.updateDocumentColorViewport;
          refreshVisibleDocumentColors = langServices.refreshVisibleDocumentColors;
          ensureSemanticTokensProvider(languageIdValue);
          ensureDocumentColorProvider(languageIdValue);
          setupLanguageSubscription(ensureSemanticTokensProvider, ensureDocumentColorProvider);
        } catch (error) {
          // Syntax services are optional; Monaco remains editable.
          console.error('[editor] optional language services failed', error);
          updateCurrentTempModel((current) => ({
            ...current,
            error: error instanceof Error ? error.message : String(error),
          }));
        }
      })();
    } catch (error) {
      if (freshness.isCurrent()) {
        editorRuntimeError = true;
        editorRuntimePhase = 'Editor failed to load. Please retry.';
        console.error('[editor] runtime initialization failed', error);
      }
      // Never rethrow from the Svelte lifecycle: this local state is the
      // boundary and Graph/page-shell failures remain independent.
    }
  }

  function retryEditorRuntime(): void {
    if (editorRuntimeReady) return;
    void initializeEditorRuntime();
  }

  onMount(() => {
    void initializeEditorRuntime();
  });

  onDestroy(() => {
    editorRuntimeToken += 1;
    clearColorViewportRefresh();
    editorIO.set(null);
    for (const controller of fullEditControllersByTabId.values()) controller.dispose();
    fullEditControllersByTabId.clear();
    cleanupSourceEditorTestHook?.();
    cleanupSourceEditorTestHook = null;
    editorPlaceholder.dispose();
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
    jsonBlockSelectionUnsub?.();
    jsonBlockSelectionUnsub = null;

    clearJsonBlockDecoration();
  });

  async function applyEditorMutation(mutation: EditorMutation): Promise<void> {
    if (!model) return;
    if (mutation.type === 'replaceSourceText') {
      setEditorValue(mutation.payload.text);
      return;
    }
    await changeLanguage(mutation.payload.languageId);
  }

  $: if (monaco && $settings) {
    editorRuntimeController.applyTheme(monaco);
  }

  export async function ensureReady(): Promise<void> {
    while (!editor || !model || (!editorRuntimeReady && !editorRuntimeError)) {
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
  }

  export async function waitForIdle(): Promise<void> {
    await ensureReady();
    while ((getActiveFullEditController()?.isImportActive() ?? false) || fullEditUiStateValue.active) {
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
    error={editorRuntimeError}
    onRetry={retryEditorRuntime}
  />
</div>
