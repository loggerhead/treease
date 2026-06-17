import type { DocumentJobAnalysisPayload, DocumentJobSettings, EventBatch, SnapshotId, StartDocumentJobResult } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { resolveDocumentAnalysis } from './DocumentAnalysisResolver';
import { mergeEventBatches, streamDocumentJobText } from '../../shared/document-job-stream';
import { collectDocumentJobResult, semanticTokensToBuffer } from '../../shared/document-job-result';
import { decodeAnalysisValueJson } from '../../shared/stored-analysis';
export type WasmError = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

type StoredDocumentAnalysis = {
  tree?: unknown;
  value?: unknown;
  diagnostics: WasmError[];
  semanticTokens?: ArrayBuffer;
  semanticTokenVersion?: number;
  sourceByteLength?: number;
  language?: string;
  snapshotId: SnapshotId | null;
};
type AnalyzeDocumentAndStoreOptions = {
  onAnalysisDelta?: (analysis: StoredDocumentAnalysis) => void | Promise<void>;
};

function createEditorAnalysisJobSettings(nest: boolean): DocumentJobSettings {
  return {
    parser: {
      enableNest: nest,
      nestMaxDepth: 8,
    },
    formatting: {
      indent: 2,
      smart: false,
      formatSourceOnClose: false,
      maxLineLength: 100,
      maxInlineComplexity: 1,
      maxArrayInlineItems: 6,
      alignObjectArrays: true,
    },
  };
}

export async function analyzeDocumentAndStore(
  languageId: SupportedEditorLanguageId,
  text: string,
  documentKey: string,
  nest: boolean,
  options?: AnalyzeDocumentAndStoreOptions,
): Promise<StoredDocumentAnalysis | null> {
  if (!documentKey) return null;
  let latestStreamingAnalysis: StoredDocumentAnalysis | null = null;
  const started = await callSharedWasmWorker<StartDocumentJobResult>('startDocumentJob', {
    documentKey,
    language: languageId,
    nest,
    settings: createEditorAnalysisJobSettings(nest),
    outputAnalysis: true,
    outputGraph: false,
  });
  const streamedBatches = await streamDocumentJobText({
    jobHandle: started.jobHandle,
    text,
    advance: (input) => callSharedWasmWorker<EventBatch>('advanceDocumentJob', input),
    onBatch: async (batch) => {
      for (const event of batch.events) {
        if (event.type !== 'analysisDelta') continue;
        const analysis = normalizeAnalysisPayload(event.analysis, null);
        if (!analysis) continue;
        latestStreamingAnalysis = analysis;
        await options?.onAnalysisDelta?.(analysis);
      }
    },
  });
  const batch = mergeEventBatches([started.batch, ...streamedBatches]);
  const result = collectDocumentJobResult(batch);
  const terminalAnalysis = normalizeAnalysisPayload(result.analysis, result.snapshotId);
  if (terminalAnalysis) return terminalAnalysis;
  if (!latestStreamingAnalysis) return null;
  return { ...latestStreamingAnalysis, snapshotId: result.snapshotId };
}


function normalizeAnalysisPayload(
  raw: DocumentJobAnalysisPayload | null,
  snapshotId: SnapshotId | null,
): StoredDocumentAnalysis | null {
  if (!raw) return null;
  return {
    tree: raw.tree,
    value: decodeAnalysisValueJson(raw.valueJson, 'editor-diagnostics', raw.language),
    diagnostics: raw.diagnostics ?? [],
    semanticTokens: semanticTokensToBuffer(raw.semanticTokens?.data),
    semanticTokenVersion: raw.semanticTokens?.version ?? 1,
    sourceByteLength: raw.sourceByteLength,
    language: raw.language,
    snapshotId,
  };
}

export type DiagnosticItem = {
  code: 'syntax-error' | 'missing-node';
  message: string;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  context: Array<{ lineNumber: number; text: string }>;
};

export type DiagnosticsResult = {
  markers: Monaco.editor.IMarkerData[];
  diagnostics: DiagnosticItem[];
  error: string;
};

function createEmptyDiagnosticsResult(): DiagnosticsResult {
  return {
    markers: [],
    diagnostics: [],
    error: '',
  };
}

function compareErrors(left: WasmError, right: WasmError): number {
  if (left.startLineNumber !== right.startLineNumber) return left.startLineNumber - right.startLineNumber;
  if (left.startColumn !== right.startColumn) return left.startColumn - right.startColumn;
  if (left.endLineNumber !== right.endLineNumber) return left.endLineNumber - right.endLineNumber;
  return left.endColumn - right.endColumn;
}

function dedupeErrors(errors: WasmError[]): WasmError[] {
  const seen = new Set<string>();
  const unique: WasmError[] = [];
  for (const error of errors) {
    const range = normalizeErrorRange(error);
    const key = `${range.startLineNumber}:${range.startColumn}:${range.endLineNumber}:${range.endColumn}:${error.kind ?? -1}`;
    if (seen.has(key)) continue;
    seen.add(key);
    unique.push(error);
  }
  return unique;
}

function normalizeErrorRange(error: WasmError) {
  const startLineNumber =
    typeof error.startLineNumber === 'number' && error.startLineNumber >= 1
      ? error.startLineNumber
      : 1;
  const startColumn =
    typeof error.startColumn === 'number' && error.startColumn >= 1 ? error.startColumn : 1;
  const endLineNumber =
    typeof error.endLineNumber === 'number' && error.endLineNumber >= startLineNumber
      ? error.endLineNumber
      : startLineNumber;
  const endColumn =
    typeof error.endColumn === 'number' && error.endColumn >= startColumn ? error.endColumn : startColumn;
  return {
    startLineNumber,
    startColumn,
    endLineNumber,
    endColumn,
  };
}

function diagnosticCodeFromKind(kind: number | undefined): DiagnosticItem['code'] {
  return kind === 2 ? 'missing-node' : 'syntax-error';
}

function diagnosticMessageFromCode(code: DiagnosticItem['code']): string {
  return code === 'missing-node' ? 'Missing node' : 'Syntax error';
}

export async function diagnosticsResultFromErrors(
  monaco: typeof Monaco,
  model: Monaco.editor.ITextModel,
  languageId: SupportedEditorLanguageId,
  errors: WasmError[],
  nest: boolean,
): Promise<DiagnosticsResult> {
  const text = model.getValue();
  void languageId;
  void nest;

  const lines = text.split(/\r?\n/);
  const orderedErrors = dedupeErrors([...errors].sort(compareErrors));
  const markers: Monaco.editor.IMarkerData[] = orderedErrors.map((error) => {
    const range = normalizeErrorRange(error);
    const code = diagnosticCodeFromKind(error.kind);
    return {
      ...range,
      severity: monaco.MarkerSeverity.Error,
      message: diagnosticMessageFromCode(code),
    };
  });

  const diagnostics: DiagnosticItem[] = orderedErrors.map((error) => {
    const range = normalizeErrorRange(error);
    const code = diagnosticCodeFromKind(error.kind);
    return {
      code,
      ...range,
      message: diagnosticMessageFromCode(code),
      context: [range.startLineNumber - 1, range.startLineNumber, range.startLineNumber + 1]
        .filter((line) => line >= 1 && line <= lines.length)
        .map((line) => ({
          lineNumber: line,
          text: lines[line - 1] ?? '',
        })),
    };
  });

  return { markers, diagnostics, error: '' };
}

export async function readStoredDiagnosticsResult(
  monaco: typeof Monaco,
  model: Monaco.editor.ITextModel,
  languageId: SupportedEditorLanguageId,
  documentKey: string,
  nest: boolean,
  preloadedErrors: WasmError[] | null = null,
): Promise<DiagnosticsResult> {
  const text = model.getValue();
  const emptyResult = createEmptyDiagnosticsResult();

  if (!documentKey) {
    return emptyResult;
  }

  const resolved = await resolveDocumentAnalysis({
    documentKey,
    preloadedAnalysis: preloadedErrors === null ? null : { diagnostics: preloadedErrors },
  });
  if (resolved.status !== 'resolved') {
    return emptyResult;
  }

  const errors = resolved.analysis.diagnostics ?? [];
  return diagnosticsResultFromErrors(monaco, model, languageId, errors, nest);
}
