<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import type * as Monaco from 'monaco-editor';
  import type { SnapshotId } from '@core-wasm/index';

  import { createFreshnessScope } from '../../guards/freshness-scope';
  import { buildDocumentJobSettings } from '../../graph-stream/document-job-runner';
  import { buildGraphStreamBuilderConfig } from '../../graph-stream/graph-stream-builder-config';
  import { editorLanguageFallback, type SupportedEditorLanguageId } from '../../monaco/language-support';
  import { getSharedMonacoRuntime } from '../../monaco/editor-runtime';
  import { attachMonacoTestHook } from '../../monaco/test-hook';
  import type { DiffPlan } from '../../graph/diff-plan';
  import { settings } from '../../settings/settings-store';
  import { applyEditorTheme, buildEditorThemeSignature } from '../../settings/ui-settings';
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../wasm/wasm-worker-singleton';
  import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
  import { getActiveTempModelSnapshot } from '../../store/graph-selection-store';
  import { tabTargetStatus, type TabTarget } from '../../store/tab-target';
  import {
    commitSidecarCompareEdit,
    commitSidecarInput,
    captureSidecarTarget,
    isVisibleSidecarTarget,
    readSidecarTempModel,
    updateSidecarCompareLanguage,
    updateSidecarTempModel as updateTargetSidecarTempModel,
  } from '../../store/sidecar-tab-state';
  import {
    editorWorkspace,
    ensureColumnDetailDraftWorkspaceTab,
    getWorkspaceTab,
    removeColumnDetailDraftWorkspaceTab,
    updateWorkspaceTab,
  } from '../../store/workspace-store';
  import { clearWorkspaceSnapshotBinding, getWorkspaceSnapshotId, getWorkspaceState } from '../../store/workspace-store';
  import { monacoChangesToDocumentTextEdits, type MonacoTextChange } from '../../../shared/document-text-edits';
  import { markSidecarRequested, markSidecarSettled } from '../../test-bridge/runtime-readiness';
  import { applyDocumentAnalysisToEditor } from './editor-analysis-apply';
  import { createEditorFullEditController } from './editor-full-edit-controller';
  import { createWorkspaceTabFullEditSink } from './editor-full-edit-sink';
  import { commitEditorTabTextChange } from './editor-tab-edit-commit';
  import { createSidecarExternalSync } from './sidecar-external-sync';
  import { createTreeaseMonacoEditorOptions } from './editor-options';
  import { createEditorPlaceholderController } from './editor-placeholder';
  import { fileDropFeedback } from '../ui/file-drop-feedback';
  import { createContentTransactionEngine, type ContentFormatOptions } from './content-transaction-engine';

  export let tabId = 'tab-sidecar';
  export let tabName = 'Right Editor';
  export let language: SupportedEditorLanguageId = editorLanguageFallback;
  export let sourceText: string | null = null;
  /** Exact semantic-token slice projected from the primary snapshot. */
  export let projectedSemanticTokens: ArrayBuffer | null = null;
  export let projectedSnapshotId: SnapshotId | null = null;
  export let projectedDocumentKey = '';
  export let projectedRevision = 0;
  export let runtimeHookId = 'right-editor';
  export let containerTestId = 'right-text-editor-container';
  export let attachToPane = true;
  export let destroyOnUnmount = false;
  export let lineNumbersMinChars: number | undefined = undefined;
  export let compactGutter = false;
  export let hideLineNumbers = false;
  export let readOnly = false;
  export let placeholderTitle = 'Start typing, or open a file';
  export let onScroll: (payload: { tabId: string; scrollTop: number; scrollLeft: number }) => void = () => {};
  export let onContentChange: (payload: { tabId: string; text: string }) => void = () => {};
  export let onEditorBlur: (text: string) => void = () => {};
  export let onRequestImportFile: (payload: {
    sourceFormat: string;
    targetFormat: string;
    accept: string[];
  }) => void | Promise<void> = async () => {};

  // A Column Detail draft owns a Monaco draft,
  // but its path is part of the primary document. Its writes must therefore
  // return through the graph planner instead of creating a second document.
  const isPrimaryDocumentDraft = () => !attachToPane;

  const themeName = 'tree-sitter-light';
  const editorOptions = {
    ...createTreeaseMonacoEditorOptions(themeName),
    scrollbar: { alwaysConsumeMouseWheel: false },
    overviewRulerBorder: true,
    colorDecorators: true,
    colorDecoratorsActivatedOn: 'clickAndHover' as const,
    'semanticHighlighting.enabled': true,
  };

  let container: HTMLDivElement;
  let monaco: typeof import('monaco-editor') | undefined;
  let editor: Monaco.editor.IStandaloneCodeEditor | null = null;
  let model: Monaco.editor.ITextModel | null = null;
  let cleanupTestHook: (() => void) | null = null;
  let boundRuntimeHookId = '';
  const ownedColumnDetailDraftTabIds = new Set<string>();
  let diffDecorations: Monaco.editor.IEditorDecorationsCollection | null = null;
  let diffBlankZoneIds: string[] = [];
  let runtimeToken = 0;
  let sidecarAnalysisSyncToken = 0;
  let sidecarReadinessRequestId = 0;
  let suppressChange = false;
  let lastModelLength = 0;
  let lastModelText = '';
  let activeLanguage: SupportedEditorLanguageId = language;
  let lastPropLanguage: SupportedEditorLanguageId = language;
  const externalSync = createSidecarExternalSync('');
  let ensureSemanticTokensProvider: (languageId: string) => void = () => {};
  let ensureDocumentColorProvider: (languageId: string) => void = () => {};
  let primeSemanticTokensForDocument: (documentKey: string, semanticTokens: ArrayBuffer) => void = () => {};
  let refreshSemanticTokensForLanguage: (languageId?: string) => void = () => {};
  let clearSemanticTokensForDocument: (documentKey?: string) => void = () => {};
  let lastAppliedThemeSignature = '';
  let detachedDraftText = sourceText ?? '';
  let detachedDraftRevision = 0;
  let detachedDraftBaseline = {
    tabId,
    documentKey: projectedDocumentKey,
    snapshotId: projectedSnapshotId,
    revision: projectedRevision,
    pending: false,
  };
  $: sidecarTab = $editorWorkspace.tabsById[tabId] ?? null;
  $: if (model && projectedSemanticTokens && (!isPrimaryDocumentDraft() || !detachedDraftBaseline.pending)) {
    primeProjectedSemanticTokens();
  }
  $: if (monaco) {
    const signature = buildEditorThemeSignature($settings);
    if (signature !== lastAppliedThemeSignature) {
      lastAppliedThemeSignature = signature;
      applyEditorTheme(monaco, themeName, $settings);
    }
  }
  $: editor?.updateOptions({ readOnly });

  const fullEditSink = createWorkspaceTabFullEditSink(tabId);
  const sidecarContentEngine = createContentTransactionEngine(
    (method, input) => callSharedWasmWorker(method, input as Record<string, unknown>),
  );

  const editorPlaceholder = createEditorPlaceholderController({
    getEditor: () => editor,
    getModel: () => model,
    getMonaco: () => monaco,
    getLanguage: () => activeLanguage,
    getTitle: () => placeholderTitle,
    onRequestImportFile: (payload) => onRequestImportFile(payload),
    onLoadExample: (example, nextLanguage) => showText(example, nextLanguage),
  });

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
    primeInitialSemanticTokens: (documentKey) => {
      primeProjectedSemanticTokens(documentKey);
    },
    setEditorValue,
    setEditorValueForFullEdit: setEditorValue,
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
    if (isPrimaryDocumentDraft()) return `column-navigator-draft:${tabId}`;
    return getWorkspaceTab(tabId)?.documentKey ?? `sidecar:${tabId}:0`;
  }

  function setModelDocumentKey(target: Monaco.editor.ITextModel | null, documentKey: string): void {
    if (!target || !documentKey) return;
    (target as Monaco.editor.ITextModel & { __treeaseDocumentKey?: string }).__treeaseDocumentKey = documentKey;
  }

  function primeProjectedSemanticTokens(documentKey: string | undefined = undefined): void {
    if (!model || !projectedSemanticTokens) return;
    const modelDocumentKey = (model as Monaco.editor.ITextModel & { __treeaseDocumentKey?: string }).__treeaseDocumentKey;
    primeSemanticTokensForDocument(documentKey ?? modelDocumentKey ?? sidecarDocumentKey(), projectedSemanticTokens);
    // This is tied to one accepted snapshot projection, not a generic
    // language reset. Monaco can now read the matching cached token payload.
    refreshSemanticTokensForLanguage(activeLanguage);
  }

  function syncLastModelSnapshot(): string {
    const text = model?.getValue() ?? '';
    lastModelLength = text.length;
    lastModelText = text;
    return text;
  }

  function currentTempModel() {
    if (isPrimaryDocumentDraft()) return getActiveTempModelSnapshot();
    if (attachToPane) {
      const pairedTarget = currentPairedTarget();
      return pairedTarget ? readSidecarTempModel(pairedTarget) ?? getActiveTempModelSnapshot() : getActiveTempModelSnapshot();
    }
    return getWorkspaceTab(tabId)?.tempModel ?? getActiveTempModelSnapshot();
  }

  /** A local edit advances only this sidecar revision; a new generation must never be adopted by an old runtime. */
  function currentPairedTarget(): TabTarget | null {
    const current = getWorkspaceTab(tabId);
    return current?.role === 'sidecar' && current.ownerMainTabId ? captureSidecarTarget(tabId) : null;
  }

  function updateSidecarTempModel(updater: (current: any) => any): void {
    if (isPrimaryDocumentDraft()) return;
    const pairedTarget = currentPairedTarget();
    if (pairedTarget) {
      updateTargetSidecarTempModel(pairedTarget, updater);
      return;
    }
    if (attachToPane) return;
    updateWorkspaceTab(tabId, { tempModel: updater(currentTempModel()) });
  }

  function ensureColumnDetailDraft(sourceText: string): void {
    if (isPrimaryDocumentDraft()) return;
    if (attachToPane) {
      // Paired sidecars are created atomically with their main tab. Creating
      // one from this mounted editor would reintroduce an unowned right pane.
      return;
    }
    ensureColumnDetailDraftWorkspaceTab({
      id: tabId,
      name: tabName,
      sourceText,
    });
    ownedColumnDetailDraftTabIds.add(tabId);
  }

  function setWorkspaceSidecarLanguage(languageId: SupportedEditorLanguageId): void {
    if (isPrimaryDocumentDraft()) return;
    const pairedTarget = currentPairedTarget();
    if (pairedTarget) {
      updateSidecarCompareLanguage(pairedTarget, languageId);
      return;
    }
    if (attachToPane) return;
    updateWorkspaceTab(tabId, { languageId });
  }

  function commitSidecarEditorState(): number {
    if (isPrimaryDocumentDraft()) {
      detachedDraftRevision += 1;
      detachedDraftBaseline = { ...detachedDraftBaseline, pending: true };
      return detachedDraftRevision;
    }
    const pairedTarget = currentPairedTarget();
    if (pairedTarget) {
      return commitSidecarCompareEdit(pairedTarget, {
        languageId: activeLanguage,
        sourceText: model?.getValue() ?? currentTempModel().scratchText,
      }) ?? (getWorkspaceTab(tabId)?.revision ?? 0);
    }
    if (attachToPane) return getWorkspaceTab(tabId)?.revision ?? 0;
    const current = getWorkspaceTab(tabId);
    const revision = (current?.revision ?? 0) + 1;
    updateWorkspaceTab(tabId, {
      languageId: activeLanguage,
      revision,
    });
    return revision;
  }

  function projectionIsNewerThanDetachedDraft(): boolean {
    if (tabId !== detachedDraftBaseline.tabId) return true;
    if (projectedDocumentKey !== detachedDraftBaseline.documentKey) return true;
    return (
      projectedRevision > detachedDraftBaseline.revision &&
      projectedSnapshotId !== detachedDraftBaseline.snapshotId
    );
  }

  function mayApplyDetachedProjection(value: string): boolean {
    if (projectedDocumentKey !== detachedDraftBaseline.documentKey) return true;
    if (!projectionIsNewerThanDetachedDraft()) return false;
    // A queued local edit can receive an earlier committed projection before
    // its own transaction lands. Only the projection that acknowledges the
    // currently visible draft may retire that pending state.
    return !detachedDraftBaseline.pending || value === detachedDraftText;
  }

  function acceptDetachedProjection(value: string): void {
    detachedDraftText = value;
    detachedDraftBaseline = {
      tabId,
      documentKey: projectedDocumentKey,
      snapshotId: projectedSnapshotId,
      revision: projectedRevision,
      pending: false,
    };
  }

  async function refreshDetachedDraftSemanticTokens(
    requestModel: Monaco.editor.ITextModel,
    requestText: string,
    requestLanguage: SupportedEditorLanguageId,
    requestRevision: number,
  ): Promise<void> {
    if (!isPrimaryDocumentDraft()) return;
    const response = await callSharedWasmWorker<{ semanticTokens?: number[] }>('semanticTokens', {
      language: requestLanguage,
      text: requestText,
    });
    if (
      requestModel !== model ||
      !isPrimaryDocumentDraft() ||
      activeLanguage !== requestLanguage ||
      detachedDraftRevision !== requestRevision ||
      requestModel.getValue() !== requestText
    ) {
      return;
    }
    const semanticTokens = response.semanticTokens ?? [];
    primeSemanticTokensForDocument(
      sidecarDocumentKey(),
      new Uint32Array(semanticTokens).buffer,
    );
    refreshSemanticTokensForLanguage(requestLanguage);
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
        const current = getWorkspaceTab(tabId);
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
    if (!freshness.isCurrent()) return;
    primeProjectedSemanticTokens();
  }

  function updateSidecarSourceText(
    value: string,
    options: { clearSnapshot: boolean | undefined } = { clearSnapshot: undefined },
  ): void {
    if (isPrimaryDocumentDraft()) {
      detachedDraftText = value;
      return;
    }
    const pairedTarget = currentPairedTarget();
    // A paired sidecar is persisted exclusively by the sidecar-input sink.
    // In particular, it must never clear a workspace snapshot binding.
    if (pairedTarget || attachToPane) return;
    const documentKey = sidecarDocumentKey();
    const shouldClearSnapshot = options.clearSnapshot ?? true;
    if (!attachToPane) updateWorkspaceTab(tabId, {
      languageId: activeLanguage,
      sourceText: value,
      ...(shouldClearSnapshot ? { snapshotId: null } : {}),
      tempModel: {
        ...currentTempModel(),
        scratchText: value,
      },
    });
    if (shouldClearSnapshot && !isPrimaryDocumentDraft()) {
      clearWorkspaceSnapshotBinding(documentKey);
    }
    if (isPrimaryDocumentDraft()) return;
    clearSemanticTokensForDocument(documentKey);
    refreshSemanticTokensForLanguage(activeLanguage);
  }

  function beginSidecarReadinessRequest(): { requestId: number; documentKey: string } {
    const requestId = ++sidecarReadinessRequestId;
    const documentKey = sidecarDocumentKey();
    markSidecarRequested({
      requestId,
      hookId: runtimeHookId,
      documentKey,
    });
    return { requestId, documentKey };
  }

  function settleSidecarReadinessRequest(request: { requestId: number; documentKey: string }, expectedText: string): void {
    const currentTab = getWorkspaceTab(tabId);
    const sourceTextValue = currentTab?.sourceText ?? '';
    const scratchTextValue = currentTab?.tempModel?.scratchText ?? '';
    const modelTextValue = model?.getValue() ?? sourceTextValue;
    if (sourceTextValue !== expectedText) return;
    if (scratchTextValue !== expectedText) return;
    if (modelTextValue !== expectedText) return;
    markSidecarSettled({
      requestId: request.requestId,
      hookId: runtimeHookId,
      documentKey: request.documentKey,
      revision: currentTab?.revision ?? 0,
    });
  }

  async function prepareSidecarSync(nextLanguage: SupportedEditorLanguageId): Promise<void> {
    await tick();
    await ensureEditor();
    setModelLanguage(nextLanguage);
  }

  function syncModelTextIfNeeded(value: string): void {
    if (model && model.getValue() !== value) {
      setModelValueSilently(model, value, () => {
        syncLastModelSnapshot();
      });
    }
  }

  async function finishSidecarSync(
    request: { requestId: number; documentKey: string },
    value: string,
    nextLanguage: SupportedEditorLanguageId,
    operationTarget: TabTarget | null,
  ): Promise<void> {
    syncModelTextIfNeeded(value);
    externalSync.acceptExternalText(value);
    if (operationTarget) {
      await runPairedSidecarTransaction(operationTarget, value, nextLanguage, true);
    } else {
      await runFullEditForCurrentText('whole-document-replacement', value, nextLanguage);
    }
    // The transaction may commit a canonical formatted form. Read the
    // explicit sidecar entity (never the active tab) to acknowledge exactly
    // the value that reached this channel's sink.
    settleSidecarReadinessRequest(request, getWorkspaceTab(tabId)?.sourceText ?? value);
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
    afterSet: (() => void) | undefined,
  ): boolean {
    if (!target) return false;
    suppressChange = true;
    target.setValue(value);
    afterSet?.();
    editorPlaceholder.update();
    queueMicrotask(() => {
      suppressChange = false;
    });
    return true;
  }

  function shouldPreserveSubmittedJsonString(
    text: string,
    nextLanguage: SupportedEditorLanguageId,
  ): boolean {
    if (nextLanguage !== 'json') return false;
    try {
      return typeof JSON.parse(text) === 'string';
    } catch {
      return false;
    }
  }

  function setEditorValue(value: string): boolean {
    if (!model || model.getValue() === value) return false;
    if (!fullEditController.isImportActive()) return false;
    return setModelValueSilently(model, value, () => {
      updateSidecarSourceText(value);
      externalSync.acceptExternalText(value);
    });
  }

  function sidecarFormattingOptions(): ContentFormatOptions {
    return {
      ...$settings.formatting,
      nest: $settings.parser.enableNest,
    };
  }

  async function runPairedSidecarTransaction(
    operationTarget: TabTarget,
    text: string,
    nextLanguage: SupportedEditorLanguageId,
    format: boolean,
  ): Promise<void> {
    const submittedText = text;
    await sidecarContentEngine.run(
      {
        channel: 'sidecar-input',
        language: nextLanguage,
        text,
        format: format ? sidecarFormattingOptions() : null,
      },
      operationTarget,
      {
        isDocumentCurrent: (candidate) => tabTargetStatus(getWorkspaceState(), candidate) === 'current',
        commit: (candidate, value) => commitSidecarInput(candidate, {
          languageId: value.language,
          sourceText: value.text,
        }),
        isVisibleCurrent: isVisibleSidecarTarget,
        project: (committedTarget, result) => {
          if (!model) return;
          activeLanguage = result.language;
          setModelLanguage(result.language);
          if (model.getValue() === submittedText && result.text !== submittedText) {
            setModelValueSilently(model, result.text, () => {
              syncLastModelSnapshot();
              externalSync.acceptExternalText(result.text);
            });
          }
          if (model.getValue() !== result.text) return;
          setModelDocumentKey(model, committedTarget.documentKey);
          primeSemanticTokensForDocument(
            committedTarget.documentKey,
            new Uint32Array(result.semanticTokens).buffer,
          );
          refreshSemanticTokensForLanguage(result.language);
        },
      },
    );
  }

  async function ensureEditor(): Promise<void> {
    if (editor || !container) return;
    const existingText = isPrimaryDocumentDraft() ? sourceText ?? detachedDraftText : getWorkspaceTab(tabId)?.sourceText ?? '';
    ensureColumnDetailDraft(existingText);
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

    const tab = getWorkspaceTab(tabId);
    externalSync.reset(isPrimaryDocumentDraft() ? existingText : tab?.sourceText ?? '');
    const uri = monaco.Uri.parse(`inmemory://sidecar/${tabId}`);
    model = monaco.editor.createModel(isPrimaryDocumentDraft() ? existingText : tab?.sourceText ?? '', activeLanguage, uri);
    syncLastModelSnapshot();
    setModelDocumentKey(model, tab?.documentKey ?? sidecarDocumentKey());
    primeProjectedSemanticTokens();
    editor = monaco.editor.create(container, {
      model,
      ...editorOptions,
      ...(lineNumbersMinChars == null ? {} : { lineNumbersMinChars }),
      ...(hideLineNumbers ? { lineNumbers: 'off' as const } : {}),
      ...(compactGutter
        ? { glyphMargin: false, folding: false, lineDecorationsWidth: 0, padding: { top: 0, bottom: 0 } }
        : {}),
      readOnly,
    });
    editorPlaceholder.update();
    await tick();
    editorPlaceholder.refresh();
    bindTestHook();
    editor.onDidLayoutChange(() => editorPlaceholder.refresh());
    editor.onDidChangeModelContent((event) => {
      const activeModel = model;
      if (!activeModel) return;
      if (suppressChange) {
        syncLastModelSnapshot();
        return;
      }
      editorPlaceholder.update();
      const previousLength = lastModelLength;
      const previousText = lastModelText;
      const nextText = activeModel.getValue();
      externalSync.recordLocalText(nextText);
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
        if (!isPrimaryDocumentDraft()) {
          const operationTarget = currentPairedTarget();
          if (operationTarget) void runPairedSidecarTransaction(operationTarget, nextText, requestLanguage, true);
          else void runFullEditForCurrentText('whole-document-replacement', nextText, requestLanguage);
        } else {
          commitSidecarEditorState();
          void refreshDetachedDraftSemanticTokens(
            activeModel,
            nextText,
            requestLanguage,
            detachedDraftRevision,
          );
        }
        onContentChange({ tabId, text: nextText });
        return;
      }
      const pairedTarget = !isPrimaryDocumentDraft() ? currentPairedTarget() : null;
      if (pairedTarget) {
        void runPairedSidecarTransaction(pairedTarget, nextText, requestLanguage, false);
        onContentChange({ tabId, text: nextText });
        return;
      }
      updateSidecarSourceText(nextText, { clearSnapshot: false });
      const isFlush = (event as unknown as { isFlush?: boolean }).isFlush ?? false;
      if (!isPrimaryDocumentDraft() && documentKey && changes.length > 0 && !isFlush) {
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
          baseSnapshotId: getWorkspaceSnapshotId(documentKey),
          commitRevision: commitSidecarEditorState,
          settings: buildDocumentJobSettings({
            enableNest: $settings.parser.enableNest,
            formatting: $settings.formatting,
            formatSourceOnClose: false,
          }),
          builderConfig: buildGraphStreamBuilderConfig($settings.viewer.graphViewer),
          isFresh: ({ revision }) => {
            const current = getWorkspaceTab(tabId);
            return (
              activeModel === model &&
              current?.documentKey === documentKey &&
              current.revision === revision &&
              current.sourceText === nextText &&
              activeLanguage === requestLanguage
            );
          },
          applyCommittedSourceText: (sourceTextValue) => {
            const shouldApplyCommittedText = externalSync.shouldApplyExternalText(sourceTextValue, activeModel.getValue());
            if (!shouldApplyCommittedText && sourceTextValue !== activeModel.getValue()) {
              return;
            }
            setModelValueSilently(activeModel, sourceTextValue, () => {
              updateSidecarSourceText(sourceTextValue, { clearSnapshot: false });
              syncLastModelSnapshot();
              externalSync.acceptExternalText(sourceTextValue);
            });
          },
          applyGraphAnalysis: applySidecarGraphAnalysis,
        });
      } else if (!isPrimaryDocumentDraft()) {
        commitSidecarEditorState();
      }
      if (isPrimaryDocumentDraft()) commitSidecarEditorState();
      if (isPrimaryDocumentDraft()) {
        void refreshDetachedDraftSemanticTokens(
          activeModel,
          nextText,
          requestLanguage,
          detachedDraftRevision,
        );
      }
      onContentChange({ tabId, text: nextText });
    });
    editor.onDidScrollChange((event) => {
      onScroll({ tabId, scrollTop: event.scrollTop, scrollLeft: event.scrollLeft });
    });
    editor.onDidFocusEditorText(() => {
      externalSync.focus();
    });
    editor.onDidBlurEditorText(() => {
      externalSync.blur();
      onEditorBlur(model?.getValue() ?? '');
    });
  }

  function bindTestHook(): void {
    if (!editor || !monaco || boundRuntimeHookId === runtimeHookId) return;
    cleanupTestHook?.();
    cleanupTestHook = attachMonacoTestHook(editor, runtimeHookId, monaco.editor.tokenize);
    boundRuntimeHookId = runtimeHookId;
  }

  export async function ensureReady(): Promise<void> {
    await ensureEditor();
  }

  export async function waitForIdle(): Promise<void> {
    await ensureReady();
    while (fullEditController.isImportActive() || fullEditSink.getState().active) {
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
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
    onContentChange({ tabId, text: value });
    ensureColumnDetailDraft(value);
    const operationTarget = currentPairedTarget();
    setWorkspaceSidecarLanguage(nextLanguage);
    const readinessRequest = beginSidecarReadinessRequest();
    await prepareSidecarSync(nextLanguage);
    await finishSidecarSync(readinessRequest, value, nextLanguage, operationTarget);
  }

  async function syncExternalSourceText(value: string, nextLanguage: SupportedEditorLanguageId = activeLanguage): Promise<void> {
    const operationTarget = currentPairedTarget();
    ensureColumnDetailDraft(value);
    setWorkspaceSidecarLanguage(nextLanguage);
    const readinessRequest = beginSidecarReadinessRequest();
    await prepareSidecarSync(nextLanguage);
    if (model && !externalSync.shouldApplyExternalText(value, model.getValue())) {
      settleSidecarReadinessRequest(readinessRequest, value);
      return;
    }
    if (!operationTarget) updateSidecarSourceText(value);
    await finishSidecarSync(readinessRequest, value, nextLanguage, operationTarget);
  }

  async function runFullEditForCurrentText(
    reason: 'whole-document-replacement' | 'language-switch',
    text = model?.getValue() ?? sidecarTab?.sourceText ?? '',
    nextLanguage: SupportedEditorLanguageId = activeLanguage,
  ): Promise<void> {
    if (isPrimaryDocumentDraft()) return;
    if (!text.trim()) return;
    const preserveSubmittedJsonString = shouldPreserveSubmittedJsonString(text, nextLanguage);
    await fullEditController.startFullEditSession({
      language: nextLanguage,
      text,
      reason,
      sourceWritebackPolicy: preserveSubmittedJsonString ? 'submitted' : 'intake',
      formatSourceOnClose: !preserveSubmittedJsonString,
      documentKey: sidecarDocumentKey(),
      isFresh: () => getWorkspaceTab(tabId)?.sourceText === text,
    });
  }

  export function getText(): string {
    return model?.getValue() ?? getWorkspaceTab(tabId)?.sourceText ?? '';
  }

  export function getLanguage(): SupportedEditorLanguageId {
    return activeLanguage;
  }

  export function setScrollPosition(position: { scrollTop: number; scrollLeft: number }): void {
    editor?.setScrollPosition(position);
  }

  export function getViewportAnchor(): { topLine: number; scrollLeft: number } | null {
    if (!editor) return null;
    return { topLine: editor.getVisibleRanges()[0]?.startLineNumber ?? 1, scrollLeft: editor.getScrollLeft() };
  }

  export function restoreViewportAnchor(anchor: { topLine: number; scrollLeft: number }): void {
    if (!editor) return;
    editor.setScrollPosition({ scrollTop: editor.getTopForLineNumber(anchor.topLine), scrollLeft: anchor.scrollLeft });
  }

  $: if (container) {
    void ensureEditor();
  }

  $: if (language !== lastPropLanguage) {
    lastPropLanguage = language;
    const languageChangedExternally = activeLanguage !== language;
    activeLanguage = language;
    sidecarAnalysisSyncToken += 1;
    setWorkspaceSidecarLanguage(activeLanguage);
    if (model && monaco && languageChangedExternally) {
      setModelLanguage(activeLanguage);
      const operationTarget = currentPairedTarget();
      if (operationTarget) {
        void runPairedSidecarTransaction(operationTarget, model.getValue(), activeLanguage, true);
      } else {
        void runFullEditForCurrentText('language-switch', undefined, activeLanguage);
      }
    }
  }

  $: if (model && monaco && activeLanguage && model.getLanguageId() !== activeLanguage) {
    setModelLanguage(activeLanguage);
  }

  $: if (editor && monaco && runtimeHookId) bindTestHook();

  $: if (model && !isPrimaryDocumentDraft() && sidecarTab && !suppressChange && sidecarTab.sourceText !== model.getValue()) {
    if (externalSync.shouldApplyExternalText(sidecarTab.sourceText, model.getValue())) {
      setModelValueSilently(model, sidecarTab.sourceText, () => {
        syncLastModelSnapshot();
        externalSync.acceptExternalText(sidecarTab.sourceText);
      });
    }
  }

  $: if (sourceText != null && (isPrimaryDocumentDraft() ? detachedDraftText : sidecarTab?.sourceText) !== sourceText && !suppressChange) {
    if (!isPrimaryDocumentDraft()) {
      void syncExternalSourceText(sourceText, activeLanguage);
    } else if (tabId !== detachedDraftBaseline.tabId) {
      // A Column Detail path is a distinct read projection even when it comes
      // from the same main snapshot. Its text must replace the previous path's
      // draft directly; external-sync protects edits within one projection only.
      acceptDetachedProjection(sourceText);
      externalSync.reset(sourceText);
      syncModelTextIfNeeded(sourceText);
      setModelDocumentKey(model, sidecarDocumentKey());
      void refreshDetachedDraftSemanticTokens(model, sourceText, activeLanguage, detachedDraftRevision);
    } else if (mayApplyDetachedProjection(sourceText)) {
      acceptDetachedProjection(sourceText);
      void syncExternalSourceText(sourceText, activeLanguage);
    }
  }

  $: if (
    isPrimaryDocumentDraft() &&
    sourceText != null &&
    detachedDraftBaseline.pending &&
    sourceText === detachedDraftText &&
    mayApplyDetachedProjection(sourceText)
  ) {
    acceptDetachedProjection(sourceText);
  }

  onDestroy(() => {
    runtimeToken += 1;
    sidecarAnalysisSyncToken += 1;
    clearDiffPlan();
    editorPlaceholder.dispose();
    cleanupTestHook?.();
    cleanupTestHook = null;
    boundRuntimeHookId = '';
    editor?.dispose();
    editor = null;
    model?.dispose();
    model = null;
    fullEditController.dispose();
    if (destroyOnUnmount) {
      for (const draftTabId of ownedColumnDetailDraftTabIds) {
        removeColumnDetailDraftWorkspaceTab(draftTabId);
      }
    }
  });
</script>

<div
  bind:this={container}
  class="relative min-h-0 min-w-0 flex-1 overflow-hidden"
  data-testid={containerTestId}
  use:fileDropFeedback
>
  <div class="file-drop-feedback-overlay" aria-hidden="true"></div>
</div>

<style>
  .file-drop-feedback-overlay {
    position: absolute;
    z-index: 10;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: rgb(224 243 255 / 88%);
    transition: opacity 120ms ease-out;
  }

  :global(.file-drop-feedback--active) .file-drop-feedback-overlay {
    opacity: 1;
  }
</style>
