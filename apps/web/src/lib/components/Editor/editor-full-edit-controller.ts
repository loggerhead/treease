// 职责：Editor full-edit 控制器：管理 full-edit session 生命周期、stream attach/commit
import type { BuilderConfig, SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { toast } from 'svelte-sonner';

import { runIntakeJob, type IntakeResult } from '../../services/DocumentIntake';
import {
  beginEditorCommitTransaction,
  settleEditorCommitTransaction,
  type EditorCommitLanding,
} from '../../services/EditorCommitTransaction';
import {
  clearFullEditDocumentJobSession,
  startReadableDocumentJobSessionForGraph,
  type FullEditDocumentJobSession,
} from '../../graph-stream/full-edit-document-job-session';
import { createFreshnessScope } from '../../guards/freshness-scope';
import { createViewRuntimeOperation, type ViewRuntimeOperation } from '../../guards/view-runtime-operation';

import { readImportSourceSample, resolveImportSourceFormat } from '../../import/resolve-import-source';
import {
  IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD,
  IMPORT_FILE_CHUNK_BYTE_SIZE,
  selectImportEditorFlushByteThreshold,
} from '../../import/import-config';
import { selectGraphStreamChunkSize } from '../../graph-stream/chunk-size-policy';
import { buildDocumentJobSettings, type DocumentJobGraphResult } from '../../graph-stream/document-job-runner';
import {
  editorLanguageFallback,
  supportedEditorLanguageSet,
  type SupportedEditorLanguageId,
} from '../../monaco/language-support';
import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
import { createPrimaryFullEditSink, type FullEditSink } from './editor-full-edit-sink';
import { trackEvent } from '../../analytics/ga4';

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
type FullEditTerminalStatus = IntakeResult['status'] | 'cancelled' | 'stale' | 'skipped';
export type FullEditTerminalOutcome = {
  revision: number;
  status: FullEditTerminalStatus;
  snapshotId: SnapshotId | null;
  result: IntakeResult | null;
};
type ImportIntakeOverride = {
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
  sourceText?: string | null;
};
type ImportIntakeResult = IntakeResult | ImportIntakeOverride | DocumentJobGraphResult;

function isIntakeResult(result: ImportIntakeResult): result is IntakeResult {
  return 'resultStatus' in result;
}

function isDocumentJobGraphResult(result: ImportIntakeResult): result is DocumentJobGraphResult {
  return 'status' in result && 'jobHandle' in result;
}

function isCancelledIntakeResult(result: IntakeResult): boolean {
  return result.status === 'failed' && typeof result.error === 'string' && result.error.startsWith('cancelled:');
}

function isDiagnosticsOnlyIntakeResult(result: ImportIntakeResult): boolean {
  return (
    (isIntakeResult(result) && result.status === 'diagnosticsOnly') ||
    (isDocumentJobGraphResult(result) && result.status === 'parseFailed')
  );
}

function isFailedImportIntakeResult(result: ImportIntakeResult): boolean {
  if (isIntakeResult(result)) return result.status === 'failed';
  if (isDocumentJobGraphResult(result)) return result.status !== 'snapshotReady';
  return result.snapshotId == null;
}

function getImportResultSourceText(result: ImportIntakeResult): string | null {
  return typeof result.sourceText === 'string' ? result.sourceText : null;
}

type FullEditSession = {
  active: boolean;
  documentKey: string;
  revision: number;
  language: SupportedEditorLanguageId;
  reason: FullEditReason;
  transportKind: FullEditTransportKind;
  sourceWritebackPolicy: SourceWritebackPolicy;
  formatSourceOnClose: boolean;
  decoder: TextDecoder;
  inputByteLength: number;
  streamSeq: number;
  chunkCount: number;
  textFlushCount: number;
  textFlushedChars: number;
  visibleText: string;
  pendingText: string;
  pendingBytes: number;
  editorFlushByteThreshold: number;
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
  setEditorValueForFullEdit: (value: string) => boolean;
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
  runBidirectionalEdit?: <T>(source: string, execute: () => Promise<T>) => Promise<T>;
};

export function createEditorFullEditController(options: CreateEditorFullEditControllerOptions) {
  let importSession: FullEditSession | null = null;
  let importConversionOperation: ViewRuntimeOperation | null = null;
  let suppressNextWholeDocumentIntake = false;
  const fullEditSink = options.fullEditSink ?? createPrimaryFullEditSink();
  const meteredFullEditReasons = new Set<FullEditReason>([
    'import-file',
    'drop-file',
    'language-switch',
    'whole-document-replacement',
  ]);

  function runMeteredFullEdit<T>(reason: FullEditReason, source: string, execute: () => Promise<T>): Promise<T> {
    if (!meteredFullEditReasons.has(reason) || !options.runBidirectionalEdit) return execute();
    return options.runBidirectionalEdit(source, execute);
  }

  const defaultFormattingOptions = {
    indent: 2,
    smart: true,
    formatSourceOnClose: true,
    maxLineLength: 100,
    maxInlineComplexity: 1,
    maxArrayInlineItems: 6,
    alignObjectArrays: true,
  };

  function documentJobSettingsFor(formatSourceOnClose = true) {
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

  function applyIntakeDocumentEffects(params: {
    documentKey: string;
    language: SupportedEditorLanguageId;
    revision: number;
    snapshotId: SnapshotId | null;
    resultStatus: Extract<DocumentJobGraphResult['status'], 'snapshotReady' | 'parseFailed'>;
    analysis: DocumentAnalysisResult | null;
    sourceText?: string | null;
  }): Promise<void> | void {
    // Document Runtime terminal state, canonical source text, and analysis land
    // in EditorCommitTransaction. Full Edit only mirrors its visible session UI.
    if (params.resultStatus === 'snapshotReady' && params.snapshotId != null) {
      fullEditSink.bindSnapshot({
        documentKey: params.documentKey,
        revision: params.revision,
        snapshotId: params.snapshotId,
      });
    }
    if (params.resultStatus === 'parseFailed') {
      trackEvent('parse_failed', { language: params.language, source: 'document_runtime' });
    }
  }

  function createIntakeLanding(session: FullEditSession): EditorCommitLanding {
    return {
      writeSourceText: (sourceText) => {
        if (session.sourceWritebackPolicy !== 'intake') return;
        session.visibleText = sourceText;
        options.setEditorValueForFullEdit(sourceText);
      },
      applyAnalysis: (analysis) => {
        const activeModel = options.getModel();
        if (!activeModel) return;
        return options.applyGraphAnalysis(activeModel, session.language, session.documentKey, session.revision, analysis);
      },
    };
  }

  async function settleReadableImportResult(
    session: FullEditSession,
    result: DocumentJobGraphResult,
  ): Promise<DocumentJobGraphResult | null> {
    const model = options.getModel();
    const freshness = createFreshnessScope(
      {
        documentKey: session.documentKey,
        languageId: session.language,
        revision: session.revision,
        sessionId: session.sessionId,
        model,
      },
      () => ({
        documentKey: importSession?.documentKey,
        languageId: importSession?.language,
        revision: importSession?.revision,
        sessionId: importSession?.sessionId,
        model: options.getModel(),
      }),
    );
    const settled = await settleEditorCommitTransaction(
      {
        documentKey: session.documentKey,
        language: session.language,
        revision: session.revision,
        settings: documentJobSettingsFor(session.formatSourceOnClose),
        builderConfig: options.getGraphBuilderConfig(),
        intent: { kind: 'analyzeSource', text: session.visibleText },
        freshness,
        landing: createIntakeLanding(session),
      },
      {
        ...result,
        documentKey: session.documentKey,
        language: session.language,
        revision: session.revision,
      },
    );
    return settled.status === 'cancelled' ? null : result;
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
    if (!session.active) return;
    session.active = false;
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

  function isCurrentImportSession(sessionId: string): boolean {
    return importSession?.active === true && importSession.sessionId === sessionId;
  }

  function flushPendingText(sessionId: string, state?: { force?: boolean }): boolean {
    const session = importSession;
    if (!session || !isCurrentImportSession(sessionId)) return false;
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
    if (!session || !isCurrentImportSession(sessionId)) return;
    if (session.flushHandle != null) return;
    session.flushHandle = requestAnimationFrame(() => {
      const activeSession = importSession;
      if (!activeSession || !isCurrentImportSession(sessionId)) return;
      activeSession.flushHandle = null;
      flushPendingText(sessionId);
    });
  }

  function bufferImportText(sessionId: string, text: string, byteLength: number): void {
    const session = importSession;
    if (!session || !isCurrentImportSession(sessionId) || !text) return;
    session.pendingText += text;
    session.pendingBytes += byteLength;
    if (!session.hasVisibleFlush || session.pendingBytes >= session.editorFlushByteThreshold) {
      scheduleImportTextFlush(sessionId);
    }
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
    formatSourceOnClose: boolean;
    editorFlushByteThreshold?: number;
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
    options.setActiveTabDocumentKey?.(documentKey);
    options.setDocumentKey(documentKey);
    options.setModelDocumentKey(model, documentKey);
    const revision = options.commitEditorState();
    const nextLanguage = params.language;
    if (params.isFresh && !params.isFresh()) return null;
    const beginFreshness = createFreshnessScope(
      { documentKey, languageId: nextLanguage, revision, token: revision, model },
      () => ({
        documentKey: importSession?.documentKey ?? documentKey,
        languageId: importSession?.language ?? nextLanguage,
        revision: importSession?.revision ?? revision,
        token: params.isFresh?.() === false ? -1 : revision,
        model: options.getModel(),
      }),
    );
    if (!beginEditorCommitTransaction({
      documentKey,
      language: nextLanguage,
      revision,
      freshness: beginFreshness,
    })) return null;

    importSession = {
      active: true,
      documentKey,
      revision,
      language: nextLanguage,
      reason: params.reason,
      transportKind: params.transportKind,
      sourceWritebackPolicy: params.sourceWritebackPolicy,
      formatSourceOnClose: params.formatSourceOnClose,
      decoder: new TextDecoder(),
      inputByteLength: 0,
      streamSeq: 0,
      chunkCount: 0,
      textFlushCount: 0,
      textFlushedChars: 0,
      visibleText: '',
      pendingText: '',
      pendingBytes: 0,
      editorFlushByteThreshold: params.editorFlushByteThreshold ?? IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD,
      flushHandle: null,
      hasVisibleFlush: false,
      ownerKey: model.uri.toString(),
      sessionId: `${documentKey}:${revision}`,
      graphJobSession: null,
    };
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
    totalBytes = 0,
  ): Promise<FullEditSession | null> {
    const session = await createFullEditSession({
      language: language as SupportedEditorLanguageId,
      reason,
      transportKind: 'file',
      editorReadOnly: true,
      sourceWritebackPolicy: 'intake',
      formatSourceOnClose: true,
      editorFlushByteThreshold: selectImportEditorFlushByteThreshold(totalBytes),
    });
    if (!session) return null;
    const changed = options.setEditorValueForFullEdit('');
    if (!changed) {
      options.setSourceText('');
    }
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
    formatSourceOnClose?: boolean;
    documentKey?: string;
    isFresh?: () => boolean;
    skipUsageMetering?: boolean;
  }): Promise<number> {
    const execute = async () => {
      const prepared = await prepareMemoryFullEditSession({
        ...params,
        editorReadOnly: false,
      });
      if (!prepared) return 0;
      void runMemoryFullEditSessionToTerminal(prepared.session, prepared.isSessionCurrent);
      return prepared.session.revision;
    };
    return params.skipUsageMetering ? execute() : runMeteredFullEdit(params.reason, params.text, execute);
  }

  async function runFullEditSessionToTerminal(params: {
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
    formatSourceOnClose?: boolean;
    documentKey?: string;
    editorReadOnly?: boolean;
    isFresh?: () => boolean;
    skipUsageMetering?: boolean;
  }): Promise<FullEditTerminalOutcome> {
    const execute = async () => {
      const prepared = await prepareMemoryFullEditSession({
        ...params,
        editorReadOnly: params.editorReadOnly ?? false,
      });
      if (!prepared) {
        return {
          revision: 0,
          status: 'skipped' as const,
          snapshotId: null,
          result: null,
        };
      }
      return runMemoryFullEditSessionToTerminal(prepared.session, prepared.isSessionCurrent);
    };
    return params.skipUsageMetering ? execute() : runMeteredFullEdit(params.reason, params.text, execute);
  }

  async function prepareMemoryFullEditSession(params: {
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
    formatSourceOnClose?: boolean;
    documentKey?: string;
    editorReadOnly: boolean;
    isFresh?: () => boolean;
  }): Promise<{ session: FullEditSession; isSessionCurrent: () => boolean } | null> {
    const model = options.getModel();
    if (!model) return null;
    const session = await createFullEditSession({
      language: params.language,
      reason: params.reason,
      transportKind: params.transportKind ?? 'memory',
      editorReadOnly: params.editorReadOnly,
      sourceWritebackPolicy: params.sourceWritebackPolicy ?? 'intake',
      formatSourceOnClose: params.formatSourceOnClose ?? true,
      documentKey: params.documentKey,
      isFresh: params.isFresh,
    });
    if (!session) return null;
    if (params.isFresh && !params.isFresh()) {
      cancelImportStream();
      return null;
    }
    const isSessionCurrent = () =>
      importSession?.active === true &&
      importSession.sessionId === session.sessionId &&
      importSession.ownerKey === session.ownerKey &&
      (params.isFresh?.() ?? true);
    session.visibleText = params.text;
    session.hasVisibleFlush = true;
    session.textFlushedChars = params.text.length;
    options.setSourceText(params.text);
    const changed = options.setEditorValueForFullEdit(params.text);
    if (!changed) {
      options.setSourceText(params.text);
    }
    const bytes = new TextEncoder().encode(params.text);
    const modelVersionId = model.getVersionId();
    session.inputByteLength = bytes.byteLength;
    feedMemoryFullEditBytes(session, bytes, modelVersionId);
    fullEditSink.markFinalizing({ sessionId: session.sessionId, ownerKey: session.ownerKey });
    return { session, isSessionCurrent };
  }

  async function runMemoryFullEditSessionToTerminal(
    session: FullEditSession,
    isSessionCurrent: () => boolean,
  ): Promise<FullEditTerminalOutcome> {
    try {
      const intakeResult = await runIntakeJob({
        documentKey: session.documentKey,
        language: session.language,
        text: session.visibleText,
        settings: documentJobSettingsFor(session.formatSourceOnClose),
        revision: session.revision,
        builderConfig: options.getGraphBuilderConfig(),
        isFresh: isSessionCurrent,
        landing: createIntakeLanding(session),
      });
      if (!isSessionCurrent()) {
        return {
          revision: session.revision,
          status: 'stale',
          snapshotId: null,
          result: intakeResult,
        };
      }
      if (intakeResult.status === 'completed') {
        await applyIntakeDocumentEffects({
          documentKey: session.documentKey,
          language: session.language,
          revision: session.revision,
          snapshotId: intakeResult.snapshotId,
          resultStatus: 'snapshotReady',
          analysis: intakeResult.analysis,
          sourceText: intakeResult.sourceText,
        });
      }
      if (intakeResult.status === 'diagnosticsOnly') {
        await applyIntakeDocumentEffects({
          documentKey: session.documentKey,
          language: session.language,
          revision: session.revision,
          snapshotId: intakeResult.snapshotId,
          resultStatus: 'parseFailed',
          analysis: intakeResult.analysis,
          sourceText: intakeResult.sourceText,
        });
      }
      if (intakeResult.status === 'failed') {
        if (isCancelledIntakeResult(intakeResult)) {
          return {
            revision: session.revision,
            status: 'cancelled',
            snapshotId: null,
            result: intakeResult,
          };
        }
        options.updateActiveTempModel((current) => ({ ...current, error: intakeResult.error }));
        toast.error('Graph rebuild failed');
      }
      return {
        revision: session.revision,
        status: intakeResult.status,
        snapshotId: intakeResult.status === 'completed' ? intakeResult.snapshotId : null,
        result: intakeResult,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      options.updateActiveTempModel((current) => ({ ...current, error: message }));
      toast.error('Graph rebuild failed');
      return {
        revision: session.revision,
        status: 'failed',
        snapshotId: null,
        result: {
          status: 'failed',
          resultStatus: 'jobFailed',
          documentKey: session.documentKey,
          revision: session.revision,
          snapshotId: null,
          analysis: null,
          sourceText: session.visibleText,
          error: message,
        },
      };
    } finally {
      settleImportSessionUi(session, 'finish');
    }
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
    const model = options.getModel();
    if (!session?.active) return;
    const { sessionId, ownerKey, documentKey } = session;
    fullEditSink.markFinalizing({ sessionId, ownerKey });
    try {
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
          isFresh: () => isCurrentImportSession(sessionId),
          landing: createIntakeLanding(session),
        }));
      if (isDocumentJobGraphResult(intakeResult)) {
        const settled = await settleReadableImportResult(session, intakeResult);
        if (!settled) return;
      }
      const formattedSourceText = getImportResultSourceText(intakeResult);
      if (importSession && formattedSourceText != null) {
        importSession.visibleText = formattedSourceText;
      }

      if (isDiagnosticsOnlyIntakeResult(intakeResult)) {
        detachImportGraphJobSession(session);
        await applyIntakeDocumentEffects({
          documentKey,
          language: session.language,
          revision: session.revision,
          snapshotId: intakeResult.snapshotId,
          resultStatus: 'parseFailed',
          analysis: intakeResult.analysis,
          sourceText: formattedSourceText,
        });
        return;
      }

      if (isFailedImportIntakeResult(intakeResult)) {
        detachImportGraphJobSession(session);
        const error = isIntakeResult(intakeResult) ? intakeResult.error : 'Graph import failed';
        options.updateActiveTempModel((current) => ({ ...current, error: error ?? 'Graph import failed' }));
        toast.error('Graph import failed');
        return;
      }

      if (typeof formattedSourceText === 'string') {
        session.visibleText = formattedSourceText;
        options.setEditorValueForFullEdit(formattedSourceText);
        setImportModelEolToLf();
      }

      await applyIntakeDocumentEffects({
        documentKey,
        language: session.language,
        revision: session.revision,
        snapshotId: intakeResult.snapshotId,
        resultStatus: 'snapshotReady',
        analysis: intakeResult.analysis,
        sourceText: formattedSourceText,
      });

      detachImportGraphJobSession(session);
    } finally {
      settleImportSessionUi(session, 'finish');
    }
  }

  function cancelImportStream(): void {
    void importConversionOperation?.cancel();
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

    void importConversionOperation?.cancel();
    const operation = createViewRuntimeOperation({
      captured: {
        languageId: targetLanguage,
        model,
      },
      getCurrent: () => ({
        languageId: options.getLanguageId(),
        model: options.getModel(),
      }),
    });
    importConversionOperation = operation;
    await operation.run({
      execute: async ({ step }) => {
        const rawText = await step(() => file.text());
        return step(() =>
          convert<string>('convert', {
            sourceLanguage,
            targetFormat: targetLanguage,
            text: rawText,
            options: options.getFormattingOptions?.(),
          }),
        );
      },
      land: async (convertedText) => {
        await startFullEditSession({
          language: targetLanguage,
          text: convertedText,
          reason,
          transportKind: 'memory',
          isFresh: operation.isCurrent,
          // importStream owns the reservation using the original file sample.
          // The converted text must not reserve the same full build a second time.
          skipUsageMetering: true,
        });
      },
    });
  }

  async function importStream(
    file: File,
    sourceLanguage: string,
    reason: Extract<FullEditReason, 'import-file' | 'drop-file'> = 'import-file',
    targetLanguage: SupportedEditorLanguageId = options.getLanguageId(),
  ): Promise<void> {
    const sourceSample = await readImportSourceSample(file);
    return runMeteredFullEdit(reason, sourceSample, () => importStreamUnmetered(file, sourceLanguage, reason, targetLanguage));
  }

  async function importStreamUnmetered(
    file: File,
    sourceLanguage: string,
    reason: Extract<FullEditReason, 'import-file' | 'drop-file'>,
    targetLanguage: SupportedEditorLanguageId,
  ): Promise<void> {
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!model || !monaco) return;
    if (!isEditorLanguage(sourceLanguage) || sourceLanguage !== targetLanguage) {
      await importConvertedFile(file, sourceLanguage, targetLanguage, reason);
      return;
    }
    const session = await beginImportStream(targetLanguage, reason, file.size);
    if (!session) return;
    try {
      const graphJobSession = startReadableDocumentJobSessionForGraph({
        sessionId: session.sessionId,
        documentKey: session.documentKey,
        revision: session.revision,
        language: session.language,
        readable: file.stream(),
        onChunk: (chunk) => {
          appendImportBytes(chunk);
        },
        settings: documentJobSettingsFor(),
        builderConfig: options.getGraphBuilderConfig(),
        chunkSize: selectGraphStreamChunkSize(file.size),
        totalBytes: file.size,
      });
      session.graphJobSession = graphJobSession;
      const intakeResult = await graphJobSession.result;
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
    runFullEditSessionToTerminal,
    appendImportChunk,
    finishImportStream,
    cancelImportStream,
    importStream,
    handleDrop,
    flushPendingText,
    suppressNextWholeDocumentIntake: () => suppressNextWholeDocumentIntake,
    dispose,
  };
}
