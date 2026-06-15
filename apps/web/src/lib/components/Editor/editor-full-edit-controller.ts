// 职责：Editor full-edit 控制器：管理 full-edit session 生命周期、stream attach/commit
import type { BuilderConfig, SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { toast } from 'svelte-sonner';

import { runIntakeJob, type IntakeResult } from '../../services/DocumentIntake';
import {
  clearFullEditDocumentJobSession,
  startReadableDocumentJobSessionForGraph,
  type FullEditDocumentJobSession,
} from '../../graph-stream/full-edit-document-job-session';
import { createFreshnessScope } from '../../guards/freshness-scope';

import { readImportSourceSample, resolveImportSourceFormat } from '../../import/resolve-import-source';
import { IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD, IMPORT_FILE_CHUNK_BYTE_SIZE } from '../../import/import-config';
import { selectGraphStreamChunkSize } from '../../graph-stream/chunk-size-policy';
import { buildDocumentJobSettings } from '../../graph-stream/document-job-runner';
import {
  editorLanguageFallback,
  supportedEditorLanguageSet,
  type SupportedEditorLanguageId,
} from '../../monaco/language-support';
import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
import { createPrimaryFullEditSink, type FullEditSink } from './editor-full-edit-sink';

type FullEditReason =
  | 'initial-example'
  | 'language-example'
  | 'language-switch'
  | 'whole-document-replacement'
  | 'tab-reactivate'
  | 'import-file'
  | 'drop-file';
type FullEditTransportKind = 'file' | 'memory';
type SourceWritebackPolicy = 'intake' | 'submitted';
type ImportIntakeOverride = {
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
  sourceText?: string | null;
};
type ImportIntakeResult = IntakeResult | ImportIntakeOverride;

function isIntakeResult(result: ImportIntakeResult): result is IntakeResult {
  return 'status' in result;
}

type FullEditSession = {
  active: boolean;
  documentKey: string;
  revision: number;
  language: SupportedEditorLanguageId;
  reason: FullEditReason;
  transportKind: FullEditTransportKind;
  sourceWritebackPolicy: SourceWritebackPolicy;
  decoder: TextDecoder;
  inputByteLength: number;
  streamSeq: number;
  chunkCount: number;
  textFlushCount: number;
  textFlushedChars: number;
  visibleText: string;
  pendingText: string;
  pendingBytes: number;
  flushHandle: number | null;
  hasVisibleFlush: boolean;
  ownerKey: string;
  sessionId: string;
  graphJobSession: FullEditDocumentJobSession | null;
};

type CreateEditorFullEditControllerOptions = {
  getModel: () => Monaco.editor.ITextModel | null;
  getEditor: () => Monaco.editor.IStandaloneCodeEditor | null;
  getMonaco: () => typeof import('monaco-editor') | undefined;
  getLanguageId: () => SupportedEditorLanguageId;
  getNestEnabled: () => boolean;
  getGraphBuilderConfig: () => BuilderConfig;
  getFullEditUiState: () => any;
  fullEditSink?: FullEditSink;
  rotateActiveDocumentKey: () => string;
  setModelDocumentKey: (target: Monaco.editor.ITextModel | null, documentKey: string) => void;
  setActiveTabDocumentKey?: (documentKey: string) => void;
  clearSemanticTokensForDocument: (documentKey?: string) => void;
  setEditorValue: (value: string) => boolean;
  setSourceText: (value: string) => void;
  setDocumentKey: (documentKey: string) => void;
  applyImportLanguage: (languageId: SupportedEditorLanguageId) => void;
  getFormattingOptions?: () => unknown;
  callWasmWorker?: <T>(method: string, input: unknown) => Promise<T>;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  commitEditorState: () => number;
  applyGraphAnalysis: (
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
  ) => Promise<void>;
  triggerGraphSync: (position: Monaco.IPosition) => void;
};

export function createEditorFullEditController(options: CreateEditorFullEditControllerOptions) {
  let importSession: FullEditSession | null = null;
  let importOnlyToken = 0;
  let suppressNextFormatSource = false;
  let suppressNextWholeDocumentIntake = false;
  const fullEditSink = options.fullEditSink ?? createPrimaryFullEditSink();

  const defaultFormattingOptions = {
    indent: 2,
    smart: true,
    formatSourceOnClose: true,
    maxLineLength: 100,
    maxInlineComplexity: 1,
    maxArrayInlineItems: 6,
    alignObjectArrays: true,
  };

  function documentJobSettingsFor() {
    const formatSourceOnClose = !suppressNextFormatSource;
    suppressNextFormatSource = false;
    return buildDocumentJobSettings({
      enableNest: options.getNestEnabled(),
      formatting: {
        ...defaultFormattingOptions,
        ...(options.getFormattingOptions?.() as Partial<typeof defaultFormattingOptions> | null | undefined),
      },
      formatSourceOnClose,
    });
  }

  function isImportActive(): boolean {
    return importSession?.active ?? false;
  }

  function isActiveSessionText(value: string): boolean {
    const session = importSession;
    if (!session?.active) return false;
    return value === session.visibleText;
  }

  function isEditorLanguage(language: string): language is SupportedEditorLanguageId {
    return supportedEditorLanguageSet.has(language as SupportedEditorLanguageId);
  }

  function appendImportTextToModel(text: string): boolean {
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!model || !monaco || !text) return false;
    const lineCount = model.getLineCount();
    const lastLineLength = model.getLineMaxColumn(lineCount);
    const range = new monaco.Range(lineCount, lastLineLength, lineCount, lastLineLength);
    model.pushEditOperations(
      null,
      [
        {
          range,
          text,
          forceMoveMarkers: true,
        },
      ],
      () => null,
    );
    return true;
  }

  function normalizeImportTextForModel(text: string): string {
    return text.replace(/\r\n?/g, '\n');
  }

  function setImportModelEolToLf(): void {
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!model || !monaco) return;
    const lf = monaco.editor?.EndOfLineSequence?.LF;
    if (lf === undefined || typeof model.pushEOL !== 'function') return;
    model.pushEOL(lf);
  }

  function applyIntakeSuccessEffects(params: {
    documentKey: string;
    language: SupportedEditorLanguageId;
    revision: number;
    snapshotId: SnapshotId | null;
    analysis: DocumentAnalysisResult | null;
    sourceText?: string | null;
  }): Promise<void> | void {
    // setSourceText is safe here: the sourceText subscriber guards on
    // fullEditUiState.active (EditorCore.svelte:591), which is true during
    // an active full-edit session, preventing the subscriber from firing
    // model.setValue and triggering a new session loop.
    if (typeof params.sourceText === 'string') {
      options.setSourceText(params.sourceText);
    }
    fullEditSink.bindSnapshot({
      documentKey: params.documentKey,
      revision: params.revision,
      snapshotId: params.snapshotId,
    });
    if (params.analysis == null) return;
    const activeModel = options.getModel();
    if (!activeModel) return;
    return options.applyGraphAnalysis(
      activeModel,
      params.language,
      params.documentKey,
      params.revision,
      params.analysis,
    );
  }

  function detachImportGraphJobSession(session: FullEditSession): void {
    const graphJobSession = session.graphJobSession;
    if (!graphJobSession) return;
    clearFullEditDocumentJobSession(session.sessionId, graphJobSession);
    session.graphJobSession = null;
  }

  function cancelImportGraphJobSession(session: FullEditSession): void {
    const graphJobSession = session.graphJobSession;
    if (!graphJobSession) return;
    void graphJobSession.cancel();
    clearFullEditDocumentJobSession(session.sessionId, graphJobSession);
    session.graphJobSession = null;
  }

  function settleImportSessionUi(session: FullEditSession, mode: 'finish' | 'cancel'): void {
    if (importSession?.sessionId === session.sessionId) {
      importSession = null;
    }
    options.getEditor()?.updateOptions({ readOnly: false });
    if (mode === 'cancel') {
      fullEditSink.cancel({ sessionId: session.sessionId, ownerKey: session.ownerKey });
      return;
    }
    fullEditSink.finish({ sessionId: session.sessionId, ownerKey: session.ownerKey });
  }


  function cancelImportTextFlush(session: FullEditSession | null): void {
    if (!session || session.flushHandle == null) return;
    cancelAnimationFrame(session.flushHandle);
    session.flushHandle = null;
  }

  function flushPendingText(sessionId: string, state?: { force?: boolean }): boolean {
    const session = importSession;
    if (!session?.active || session.sessionId !== sessionId) return false;
    cancelImportTextFlush(session);
    if (!session.pendingText) {
      session.pendingBytes = 0;
      return false;
    }
    const force = state?.force ?? false;
    let pendingText = session.pendingText;
    if (!force && pendingText.endsWith('\r')) {
      session.pendingText = '\r';
      pendingText = pendingText.slice(0, -1);
    } else {
      session.pendingText = '';
    }
    session.pendingBytes = 0;
    if (!pendingText) return false;
    const previousVisibleText = session.visibleText;
    session.visibleText = `${previousVisibleText}${normalizeImportTextForModel(pendingText)}`;
    const appended = appendImportTextToModel(pendingText);
    if (!appended) {
      session.visibleText = previousVisibleText;
      return false;
    }
    session.hasVisibleFlush = true;
    session.textFlushCount += 1;
    session.textFlushedChars += pendingText.length;
    options.setSourceText(session.visibleText);
    return true;
  }

  function scheduleImportTextFlush(sessionId: string): void {
    const session = importSession;
    if (!session?.active || session.sessionId !== sessionId) return;
    if (session.flushHandle != null) return;
    session.flushHandle = requestAnimationFrame(() => {
      const activeSession = importSession;
      if (!activeSession?.active || activeSession.sessionId !== sessionId) return;
      activeSession.flushHandle = null;
      flushPendingText(sessionId);
    });
  }

  function bufferImportText(sessionId: string, text: string, byteLength: number): void {
    const session = importSession;
    if (!session?.active || session.sessionId !== sessionId || !text) return;
    session.pendingText += text;
    session.pendingBytes += byteLength;
    if (!session.hasVisibleFlush || session.pendingBytes >= IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD) {
      flushPendingText(sessionId);
      return;
    }
    scheduleImportTextFlush(sessionId);
  }

  function appendMemoryFullEditChunkMeta(session: FullEditSession, inputByteLength: number, modelVersionId: number): void {
    fullEditSink.appendChunkMeta({
      sessionId: session.sessionId,
      ownerKey: session.ownerKey,
      streamSeq: session.streamSeq,
      inputByteLength,
      modelVersionId,
    });
  }

  function feedMemoryFullEditBytes(session: FullEditSession, bytes: Uint8Array, modelVersionId: number): void {
    if (bytes.byteLength === 0) {
      session.streamSeq += 1;
      appendMemoryFullEditChunkMeta(session, 0, modelVersionId);
      return;
    }

    for (let offset = 0; offset < bytes.byteLength; offset += IMPORT_FILE_CHUNK_BYTE_SIZE) {
      const chunk = bytes.subarray(offset, Math.min(bytes.byteLength, offset + IMPORT_FILE_CHUNK_BYTE_SIZE));
      session.streamSeq += 1;
      session.chunkCount += 1;
      const inputByteLength = offset + chunk.byteLength;
      appendMemoryFullEditChunkMeta(session, inputByteLength, modelVersionId);

    }
  }



  async function createFullEditSession(params: {
    language: SupportedEditorLanguageId;
    reason: FullEditReason;
    transportKind: FullEditTransportKind;
    editorReadOnly: boolean;
    sourceWritebackPolicy: SourceWritebackPolicy;
    documentKey?: string;
    isFresh?: () => boolean;
  }): Promise<FullEditSession | null> {
    const model = options.getModel();
    const monaco = options.getMonaco();
    const editor = options.getEditor();
    if (!model || !monaco) return null;
    if (params.isFresh && !params.isFresh()) return null;
    if (importSession?.active) {
      cancelImportStream();
    }

    const documentKey = params.documentKey || options.rotateActiveDocumentKey();
    if (params.documentKey) {
      options.setActiveTabDocumentKey?.(params.documentKey);
      options.setDocumentKey(params.documentKey);
    }
    const revision = options.commitEditorState();
    const nextLanguage = params.language;
    if (params.isFresh && !params.isFresh()) return null;

    importSession = {
      active: true,
      documentKey,
      revision,
      language: nextLanguage,
      reason: params.reason,
      transportKind: params.transportKind,
      sourceWritebackPolicy: params.sourceWritebackPolicy,
      decoder: new TextDecoder(),
      inputByteLength: 0,
      streamSeq: 0,
      chunkCount: 0,
      textFlushCount: 0,
      textFlushedChars: 0,
      visibleText: '',
      pendingText: '',
      pendingBytes: 0,
      flushHandle: null,
      hasVisibleFlush: false,
      ownerKey: model.uri.toString(),
      sessionId: `${documentKey}:${revision}`,
      graphJobSession: null,
    };
    options.setModelDocumentKey(model, documentKey);
    options.clearSemanticTokensForDocument(documentKey);
    fullEditSink.begin({
      sessionId: `${documentKey}:${revision}`,
      ownerKey: model.uri.toString(),
      documentKey,
      revision,
      language: nextLanguage,
      transportKind: params.transportKind,
      reason: params.reason,
    });
    options.applyImportLanguage(nextLanguage);
    editor?.updateOptions({ readOnly: params.editorReadOnly });
    return importSession;
  }

  async function beginImportStream(
    language: string,
    reason: Extract<FullEditReason, 'import-file' | 'drop-file'> = 'import-file',
  ): Promise<FullEditSession | null> {
    const session = await createFullEditSession({
      language: language as SupportedEditorLanguageId,
      reason,
      transportKind: 'file',
      editorReadOnly: true,
      sourceWritebackPolicy: 'intake',
    });
    if (!session) return null;
    options.setEditorValue('');
    options.setSourceText('');
    setImportModelEolToLf();
    return session;
  }

  async function startFullEditSession(params: {
    language: SupportedEditorLanguageId;
    text: string;
    reason: Extract<
      FullEditReason,
      | 'initial-example'
      | 'language-example'
      | 'language-switch'
      | 'whole-document-replacement'
      | 'tab-reactivate'
      | 'import-file'
      | 'drop-file'
    >;
    transportKind?: FullEditTransportKind;
    sourceWritebackPolicy?: SourceWritebackPolicy;
    documentKey?: string;
    isFresh?: () => boolean;
  }): Promise<number> {
    const model = options.getModel();
    if (!model) return 0;
    const session = await createFullEditSession({
      language: params.language,
      reason: params.reason,
      transportKind: params.transportKind ?? 'memory',
      editorReadOnly: false,
      sourceWritebackPolicy: params.sourceWritebackPolicy ?? 'intake',
      documentKey: params.documentKey,
      isFresh: params.isFresh,
    });
    if (!session) return 0;
    if (params.isFresh && !params.isFresh()) {
      cancelImportStream();
      return 0;
    }
    session.visibleText = params.text;
    session.hasVisibleFlush = true;
    session.textFlushedChars = params.text.length;
    options.setSourceText(params.text);
    const changed = options.setEditorValue(params.text);
    if (!changed) {
      options.setSourceText(params.text);
    }
    const bytes = new TextEncoder().encode(params.text);
    const modelVersionId = model.getVersionId();
    session.inputByteLength = bytes.byteLength;
    feedMemoryFullEditBytes(session, bytes, modelVersionId);
    fullEditSink.markFinalizing({ sessionId: session.sessionId, ownerKey: session.ownerKey });
    void runIntakeJob({
      documentKey: session.documentKey,
      language: session.language,
      text: session.visibleText,
      settings: documentJobSettingsFor(),
      revision: session.revision,
      builderConfig: options.getGraphBuilderConfig(),
    }).then((intakeResult) => {
      if (intakeResult.status === 'completed') {
        const authoritativeSourceText = (intakeResult as { sourceText?: string | null }).sourceText ?? null;
        const shouldApplyAuthoritativeSourceText =
          session.sourceWritebackPolicy === 'intake' && authoritativeSourceText != null;
        if (shouldApplyAuthoritativeSourceText) {
          session.visibleText = authoritativeSourceText;
          options.setEditorValue(authoritativeSourceText);
        }
        void applyIntakeSuccessEffects({
          documentKey: session.documentKey,
          language: session.language,
          revision: session.revision,
          snapshotId: intakeResult.snapshotId,
          analysis: intakeResult.analysis,
          sourceText: shouldApplyAuthoritativeSourceText ? authoritativeSourceText : undefined,
        });
      }
      if (intakeResult.status === 'failed') {
        options.updateActiveTempModel((current) => ({ ...current, error: intakeResult.error }));
        toast.error('Graph rebuild failed');
      }
    }).finally(() => {
      fullEditSink.finish({ sessionId: session.sessionId, ownerKey: session.ownerKey });
      if (importSession?.sessionId === session.sessionId) {
        importSession = null;
      }
    });
    return session.revision;
  }

  function appendImportChunk(chunk: Uint8Array): void {
    const session = importSession;
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!session?.active || !model || !monaco) return;
    const ownerKey = model.uri.toString();
    if (ownerKey !== session.ownerKey) return;
    if (fullEditSink.getState().phase !== 'streaming') return;
    const text = session.decoder.decode(chunk, { stream: true });
    session.inputByteLength += chunk.byteLength;
    session.streamSeq += 1;
    session.chunkCount += 1;
    if (text) {
      bufferImportText(session.sessionId, text, chunk.byteLength);
    }
    fullEditSink.appendChunkMeta({
      sessionId: session.sessionId,
      ownerKey: session.ownerKey,
      streamSeq: session.streamSeq,
      inputByteLength: session.inputByteLength,
      modelVersionId: model.getVersionId(),
    });

  }

  function appendImportBytes(bytes: Uint8Array): void {
    for (let offset = 0; offset < bytes.byteLength; offset += IMPORT_FILE_CHUNK_BYTE_SIZE) {
      appendImportChunk(bytes.subarray(offset, Math.min(bytes.byteLength, offset + IMPORT_FILE_CHUNK_BYTE_SIZE)));
    }
  }

  async function finishImportStream(intakeOverride?: ImportIntakeOverride): Promise<void> {
    const session = importSession;
    const editor = options.getEditor();
    const model = options.getModel();
    if (!session?.active) return;
    const { sessionId, ownerKey, documentKey } = session;
    fullEditSink.markFinalizing({ sessionId, ownerKey });
    const flushText = session.decoder.decode();
    if (flushText) {
      bufferImportText(sessionId, flushText, 0);
    }
    flushPendingText(sessionId, { force: true });
    const currentModelValue = model?.getValue() ?? '';
    options.setSourceText(currentModelValue);
    if (documentKey) {
      options.setModelDocumentKey(options.getModel(), documentKey);
      options.setDocumentKey(documentKey);
    }
    const intakeResult =
      intakeOverride ??
      (await runIntakeJob({
        documentKey,
        language: session.language,
        text: currentModelValue,
        settings: documentJobSettingsFor(),
        revision: session.revision,
        builderConfig: options.getGraphBuilderConfig(),
      }));
    const formattedSourceText = (intakeResult as { sourceText?: string | null }).sourceText ?? null;
    if (importSession && formattedSourceText != null) {
      importSession.visibleText = formattedSourceText;
    }

    if (isIntakeResult(intakeResult) ? intakeResult.status === 'failed' : !intakeResult.snapshotId) {
      detachImportGraphJobSession(session);
      if (!importSession?.active || importSession.sessionId !== sessionId) return;
      settleImportSessionUi(session, 'finish');
      const error = isIntakeResult(intakeResult) ? intakeResult.error : 'Graph import failed';
      options.updateActiveTempModel((current) => ({ ...current, error: error ?? 'Graph import failed' }));
      toast.error('Graph import failed');
      return;
    }

    if (typeof formattedSourceText === 'string') {
      session.visibleText = formattedSourceText;
      options.setEditorValue(formattedSourceText);
      setImportModelEolToLf();
    }

    await applyIntakeSuccessEffects({
      documentKey,
      language: session.language,
      revision: session.revision,
      snapshotId: intakeResult.snapshotId,
      analysis: intakeResult.analysis,
      sourceText: formattedSourceText,
    });

    detachImportGraphJobSession(session);
    if (!importSession?.active || importSession.sessionId !== sessionId) return;
    settleImportSessionUi(session, 'finish');

  }

  function cancelImportStream(): void {
    if (!importSession?.active) return;
    const session = importSession;
    cancelImportTextFlush(session);
    session.pendingText = '';
    session.pendingBytes = 0;
    cancelImportGraphJobSession(session);
    settleImportSessionUi(session, 'cancel');

    if (options.getModel()) {
      options.setSourceText(options.getModel()?.getValue() ?? '');
      options.commitEditorState();
    }
  }

  async function importConvertedFile(
    file: File,
    sourceLanguage: string,
    targetLanguage: SupportedEditorLanguageId,
    reason: Extract<FullEditReason, 'import-file' | 'drop-file'>,
  ): Promise<void> {
    const convert = options.callWasmWorker;
    const model = options.getModel();
    if (!convert) {
      throw new Error(`Unsupported import language: ${sourceLanguage}`);
    }
    if (!model) return;

    const token = (importOnlyToken += 1);
    const freshness = createFreshnessScope(
      {
        languageId: targetLanguage,
        model,
        token,
      },
      () => ({
        languageId: options.getLanguageId(),
        model: options.getModel(),
        token: importOnlyToken,
      }),
    );

    const rawText = await freshness.step(() => file.text());
    if (rawText == null) return;

    const convertedText = await freshness.step(() =>
      convert<string>('convert', {
        sourceLanguage,
        targetFormat: targetLanguage,
        text: rawText,
        options: options.getFormattingOptions?.(),
      }),
    );
    if (convertedText == null) return;

    await startFullEditSession({
      language: targetLanguage,
      text: convertedText,
      reason,
      transportKind: 'memory',
      isFresh: freshness.isCurrent,
    });
  }

  async function importStream(
    file: File,
    sourceLanguage: string,
    reason: Extract<FullEditReason, 'import-file' | 'drop-file'> = 'import-file',
    targetLanguage: SupportedEditorLanguageId = options.getLanguageId(),
  ): Promise<void> {
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!model || !monaco) return;
    if (!isEditorLanguage(sourceLanguage) || sourceLanguage !== targetLanguage) {
      await importConvertedFile(file, sourceLanguage, targetLanguage, reason);
      return;
    }
    const session = await beginImportStream(targetLanguage, reason);
    if (!session) return;
    try {
      const [workerStream, uiStream] = file.stream().tee();
      const graphJobSession = startReadableDocumentJobSessionForGraph({
        sessionId: session.sessionId,
        documentKey: session.documentKey,
        revision: session.revision,
        language: session.language,
        readable: workerStream,
        settings: documentJobSettingsFor(),
        builderConfig: options.getGraphBuilderConfig(),
        chunkSize: selectGraphStreamChunkSize(file.size),
        totalBytes: file.size,
      });
      session.graphJobSession = graphJobSession;
      let resolvedIntakeResult: Awaited<typeof graphJobSession.result> | null = null;
      const intakePromise = graphJobSession.result.then((result) => {
        resolvedIntakeResult = result;
        return result;
      });
      const reader = uiStream.getReader();

      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) {

            break;
          }
          if (value) {
            appendImportBytes(value);
            if (session.inputByteLength >= file.size && resolvedIntakeResult) {

              break;
            }
          }
        }
      } finally {
        reader.releaseLock();
      }
      const intakeResult = resolvedIntakeResult ?? (await intakePromise);
      await finishImportStream(intakeResult);
    } catch (error) {
      cancelImportStream();
      throw error;
    }
  }

  async function handleDrop(event: DragEvent): Promise<void> {
    event.preventDefault();
    const files = event.dataTransfer?.files;
    if (!files || files.length === 0) return;
    const file = files[0];

    // JSONL/NDJSON: import text without full-document parse. The cursor-driven
    // findJsonBlockAtPosition handles per-line graph display. triggerGraphSync
    // kicks off the graph for the first line immediately after import completes.
    const lowerName = file.name.toLowerCase();
    if (lowerName.endsWith('.jsonl') || lowerName.endsWith('.ndjson')) {
      const text = await file.text();
      suppressNextWholeDocumentIntake = true;
      options.applyImportLanguage('json');
      options.setEditorValue(text);
      suppressNextWholeDocumentIntake = false;
      options.setSourceText(text);
      options.updateActiveTempModel((current) => ({ ...current, error: '', diagnostics: [] }));
      options.triggerGraphSync({ lineNumber: 1, column: 1 });
      return;
    }
    const sample = await readImportSourceSample(file);
    const sourceFormat = await resolveImportSourceFormat(file.name, sample, editorLanguageFallback);
    const targetLanguage = supportedEditorLanguageSet.has(sourceFormat as SupportedEditorLanguageId)
      ? (sourceFormat as SupportedEditorLanguageId)
      : editorLanguageFallback;
    await importStream(file, sourceFormat, 'drop-file', targetLanguage);
  }

  function dispose(): void {
    if (importSession) {
      cancelImportTextFlush(importSession);
    }
    importSession = null;
  }

  return {
    isImportActive,
    isActiveSessionText,
    beginImportStream,
    startFullEditSession,
    appendImportChunk,
    finishImportStream,
    cancelImportStream,
    importStream,
    handleDrop,
    flushPendingText,
    suppressNextWholeDocumentIntake: () => suppressNextWholeDocumentIntake,
    setSuppressNextFormatSource: (value: boolean) => {
      suppressNextFormatSource = value;
    },
    dispose,
  };
}
