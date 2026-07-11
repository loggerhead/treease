// 职责：Worker 侧文档解析 handler：parseToTree、analysis job 管理
import {
  advanceDocumentJob,
  findJsonBlockAtPosition,
  getDiagnostics,
  parseToTree,
  parseValueToTreeJson,
  startDocumentJob,
  type SnapshotId,
  type EventBatch,
  type DocumentJobAnalysisPayload,
  type TreeNode as WasmTreeNode,
} from '@core-wasm/index';

import { mergeEventBatches, streamDocumentJobText } from '../../shared/document-job-stream';
import { collectDocumentJobResult, normalizeDocumentJobAnalysisPayload } from '../../shared/document-job-result';
import { clonePlainTreeNode } from '../../shared/tree-node-value';
import type { DocumentAnalysisCacheRuntime } from './document-runtime-state';
import type { DocumentAnalysisResult, JsonBlockAtPositionResult, WorkerRequest } from './protocol';
import { readWorkerTextInput } from './request-utils';

export type DocumentParseRuntime = DocumentAnalysisCacheRuntime;



export function normalizeDocumentJobAnalysisResult(
  documentKey: string,
  language: string,
  raw: DocumentJobAnalysisPayload | null | undefined,
): DocumentAnalysisResult | null {
  return normalizeDocumentJobAnalysisPayload(documentKey, language, raw);
}

async function parseValueToTreeNode(language: string, text: string, nest: boolean): Promise<WasmTreeNode> {
  const node = await parseValueToTreeJson({ language, text, nest });
  return clonePlainTreeNode(node as unknown as WasmTreeNode);
}

export async function parseToTreeNode(language: string, text: string, nest: boolean): Promise<WasmTreeNode> {
  const node = await parseToTree(language, text, { nest });
  return clonePlainTreeNode(node as unknown as WasmTreeNode);
}

export async function handleDiagnostics(
  message: Extract<WorkerRequest, { type: 'diagnostics' }>,
): Promise<Awaited<ReturnType<typeof getDiagnostics>>> {
  return getDiagnostics(message.language, message.text);
}

export function cacheDocumentJobAnalysisResult(
  _runtime: DocumentAnalysisCacheRuntime,
  documentKey: string,
  language: string,
  _text: string,
  batch: EventBatch,
  analysis: DocumentJobAnalysisPayload | null | undefined,
  _textBytes?: ArrayBuffer | SharedArrayBuffer | Uint8Array,
): DocumentAnalysisResult | null {
  // Snapshot is stored on the Rust side; no Worker cache needed
  let _ = batch; // keep parameter for compatibility
  return normalizeDocumentJobAnalysisResult(documentKey, language, analysis);
}

type StartedDocumentJobAnalysis = {
  batch: EventBatch;
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
};

async function startSnapshotDocumentJob(
  documentKey: string,
  language: string,
  text: string,
  nest: boolean,
  textBytes?: ArrayBuffer | SharedArrayBuffer | Uint8Array,
): Promise<StartedDocumentJobAnalysis> {
  const started = await startDocumentJob({
    documentKey,
    language,
    nest,
    settings: {
      parser: { enableNest: nest, nestMaxDepth: 8 },
      formatting: {
        indent: 2,
        smart: true,
        formatSourceOnClose: true,
        maxLineLength: 100,
        maxInlineComplexity: 1,
        maxArrayInlineItems: 6,
        alignObjectArrays: true,
      },
    },
    outputGraph: true,
    outputAnalysis: true,
  });
  const streamedBatches = await streamDocumentJobText({
    jobHandle: started.jobHandle,
    text,
    advance: (input) => advanceDocumentJob(input),
  });
  const batch = mergeEventBatches([started.batch, ...streamedBatches]);
  const result = collectDocumentJobResult(batch);
  const analysis = cacheDocumentJobAnalysisResult(
    {} as DocumentAnalysisCacheRuntime,
    documentKey,
    language,
    text,
    batch,
    result.analysis,
    textBytes,
  );
  return {
    batch,
    snapshotId: result.snapshotId,
    analysis,
  };
}

export async function handleParseAndStore(
  runtime: Pick<DocumentParseRuntime, 'encoder'>,
  message: Extract<WorkerRequest, { type: 'parseAndStore' }>,
): Promise<true> {
  const { resolvedText, textBytes, hasBytes } = readWorkerTextInput(message);
  await startSnapshotDocumentJob(
    message.documentKey,
    message.language,
    resolvedText,
    message.nest,
    hasBytes ? textBytes! : undefined,
  );
  return true;
}

export async function handleParseToTree(
  message: Extract<WorkerRequest, { type: 'parseToTree' }>,
): Promise<WasmTreeNode> {
  return parseToTreeNode(message.language, message.text, message.nest);
}

function offsetToEditorPosition(text: string, offset: number): { lineNumber: number; column: number } {
  const clamped = Math.max(0, Math.min(offset, text.length));
  let lineNumber = 1;
  let lineStart = 0;
  for (let index = 0; index < clamped; index += 1) {
    if (text.charCodeAt(index) === 10) {
      lineNumber += 1;
      lineStart = index + 1;
    }
  }
  return { lineNumber, column: clamped - lineStart + 1 };
}

function byteOffsetToTextOffset(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let byteCount = 0;
  let textOffset = 0;
  while (textOffset < text.length && byteCount < byteOffset) {
    const codePoint = text.codePointAt(textOffset);
    if (codePoint == null) break;
    const charLength = codePoint > 0xffff ? 2 : 1;
    const charBytes = utf8ByteLengthForCodePoint(codePoint);
    if (byteCount + charBytes > byteOffset) break;
    byteCount += charBytes;
    textOffset += charLength;
  }
  return textOffset;
}

function utf8ByteLengthForCodePoint(codePoint: number): number {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

function createEmptyJsonBlockAtPositionResult(): JsonBlockAtPositionResult {
  return {
    found: false,
    text: '',
    startByte: 0,
    endByte: 0,
    startLineNumber: 1,
    startColumn: 1,
    endLineNumber: 1,
    endColumn: 1,
  };
}

export async function handleFindJsonBlockAtPosition(
  message: Extract<WorkerRequest, { type: 'findJsonBlockAtPosition' }>,
): Promise<JsonBlockAtPositionResult> {
  const span = await findJsonBlockAtPosition(message.language, message.text, message.row, message.column);
  if (!span?.found) return createEmptyJsonBlockAtPositionResult();

  const startOffset = byteOffsetToTextOffset(message.text, span.startByte);
  const endOffset = byteOffsetToTextOffset(message.text, span.endByte);
  const start = offsetToEditorPosition(message.text, startOffset);
  const end = offsetToEditorPosition(message.text, endOffset);
  return {
    found: true,
    text: message.text.slice(startOffset, endOffset),
    startByte: span.startByte,
    endByte: span.endByte,
    startLineNumber: start.lineNumber,
    startColumn: start.column,
    endLineNumber: end.lineNumber,
    endColumn: end.column,
  };
}

export async function handleParseValueToTree(
  message: Extract<WorkerRequest, { type: 'parseValueToTree' }>,
): Promise<WasmTreeNode> {
  return parseValueToTreeNode(message.language, message.text, message.nest);
}
