import type { StoredDocumentAnalysis } from '@core-wasm/index';
import type { DocumentAnalysisResult, WasmErrorLike } from './worker-protocol/protocol';

export function decodeStoredDiagnostics(raw: Uint32Array): WasmErrorLike[] {
  const errors: WasmErrorLike[] = [];
  for (let i = 0; i + 4 < raw.length; i += 5) {
    errors.push({
      startLineNumber: raw[i] + 1,
      startColumn: raw[i + 1] + 1,
      endLineNumber: raw[i + 2] + 1,
      endColumn: raw[i + 3] + 1,
      kind: raw[i + 4],
    });
  }
  return errors;
}

export function semanticTokensToArrayBuffer(raw: Uint32Array): ArrayBuffer {
  if (raw.byteLength === 0) return new ArrayBuffer(0);
  const copy = new Uint8Array(raw.byteLength);
  copy.set(new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength));
  return copy.buffer as ArrayBuffer;
}
export function decodeAnalysisValueJson(
  valueJson: string | null | undefined,
  documentKey: string,
  language: string,
): unknown {
  if (!valueJson) return null;
  try {
    return JSON.parse(valueJson);
  } catch (error) {
    console.warn('[wasm] failed to decode analysis valueJson', {
      documentKey,
      language,
      error: error instanceof Error ? error.message : String(error),
    });
    return undefined;
  }
}


export function normalizeStoredAnalysisResult(
  documentKey: string,
  language: string,
  raw: StoredDocumentAnalysis | null | undefined,
): DocumentAnalysisResult | null {
  if (!raw) return null;
  const value = decodeAnalysisValueJson(raw.valueJson, documentKey, language);
  return {
    documentKey,
    tree: raw.tree ?? null,
    value,
    diagnostics: decodeStoredDiagnostics(raw.diagnostics),
    semanticTokens: semanticTokensToArrayBuffer(raw.semanticTokens),
    semanticTokenVersion: 1,
    sourceByteLength: raw.sourceByteLength,
    language: raw.language || language,
  };
}

export function hasMeaningfulStoredAnalysis(
  analysis: StoredDocumentAnalysis | null | undefined,
): analysis is StoredDocumentAnalysis {
  if (!analysis) return false;
  if (analysis.sourceByteLength > 0) return true;
  if (analysis.semanticTokens.byteLength > 0) return true;
  if (analysis.diagnostics.length > 0) return true;
  if (analysis.tree) return true;
  return typeof analysis.valueJson === 'string' && analysis.valueJson.length > 0;
}
