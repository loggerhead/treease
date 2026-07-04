import type * as Monaco from 'monaco-editor';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { resolveDocumentAnalysis } from './DocumentAnalysisResolver';
export type WasmError = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

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
