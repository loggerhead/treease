import type * as Monaco from 'monaco-editor';
import { get } from 'svelte/store';
import type { SupportedEditorLanguageId } from './language-support';
import { supportedEditorLanguageIds } from './language-support';
import type { MonacoApi } from './public-types';
import {
  detectCssColorFormat,
  formatCssColor,
  fromMonacoColor,
  getCssColorMatches,
  parseCssColor,
  toMonacoColor,
  type CssColorFormat,
} from '../preview/color';
import { resolveTreePathResult, resolvePathSpanResult, toByteColumn } from '../services/TreePathService';
import { resolveEditorPositionTargetResult } from '../components/Editor/editor-position-target';
import { getWorkspaceSnapshotId } from '../store/workspace-snapshot-bindings';
import { settings } from '../settings/settings-store';

type ColorProviderRegistrarOptions = {
  monaco: MonacoApi;
};

type ColorProviderViewportState = {
  visibleRanges: Monaco.Range[];
  versionId: number;
  updatedAt: number;
};

export type DocumentColorRegistrar = ((languageId: string) => void) & {
  updateViewport: (model: Monaco.editor.ITextModel, visibleRanges: Monaco.Range[]) => void;
  refreshVisibleColors: (model: Monaco.editor.ITextModel) => void;
};

type ColorInfoMetadata = {
  format: CssColorFormat;
  originalText: string;
  alphaByte: number;
};

type MonacoColorInfo = Monaco.languages.IColorInformation & {
  __treeaseColorInfo?: ColorInfoMetadata;
};

const colorInfoMetadata = new WeakMap<Monaco.languages.IColorInformation, ColorInfoMetadata>();
const colorActivationLanguageSet = new Set<SupportedEditorLanguageId>(supportedEditorLanguageIds);
const viewportStateByModel = new WeakMap<Monaco.editor.ITextModel, ColorProviderViewportState>();
const COLOR_VIEWPORT_OVERSCAN_LINES = 200;
const COLOR_DETECTOR_CONTRIBUTION_ID = 'editor.contrib.colorDetector';

function isSupportedLanguage(languageId: string): languageId is SupportedEditorLanguageId {
  return colorActivationLanguageSet.has(languageId as SupportedEditorLanguageId);
}

function byteOffsetToPosition(text: string, byteOffset: number): { lineNumber: number; column: number } {
  const target = Math.max(0, byteOffset);
  const lines = text.split('\n');
  let consumedBytes = 0;

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex] ?? '';
    const lineBytes = new TextEncoder().encode(line).length;
    const newlineBytes = lineIndex < lines.length - 1 ? 1 : 0;
    const lineStart = consumedBytes;
    const lineEnd = lineStart + lineBytes;

    if (target <= lineEnd) {
      let charIndex = 0;
      while (charIndex < line.length) {
        const nextBytes = lineStart + new TextEncoder().encode(line.slice(0, charIndex + 1)).length;
        if (target < nextBytes) break;
        charIndex += 1;
      }
      return { lineNumber: lineIndex + 1, column: charIndex + 1 };
    }

    consumedBytes = lineEnd + newlineBytes;
  }

  const lastLine = lines[lines.length - 1] ?? '';
  return { lineNumber: lines.length, column: lastLine.length + 1 };
}

function buildRangeFromByteOffsets(
  monaco: MonacoApi,
  text: string,
  startByte: number,
  endByte: number,
): Monaco.Range {
  const start = byteOffsetToPosition(text, startByte);
  const end = byteOffsetToPosition(text, endByte);
  return new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column);
}

function createFallbackPosition(
  monaco: MonacoApi,
  offset: number,
  text: string,
): Monaco.Position {
  const lines = text.slice(0, Math.max(0, offset)).split('\n');
  return new monaco.Position(lines.length, (lines[lines.length - 1]?.length ?? 0) + 1);
}

function getCandidatePosition(
  monaco: MonacoApi,
  model: Monaco.editor.ITextModel,
  offset: number,
): Monaco.Position {
  const getPositionAt = (model as Monaco.editor.ITextModel & { getPositionAt?: (offset: number) => Monaco.Position })
    .getPositionAt;
  if (typeof getPositionAt === 'function') {
    return getPositionAt.call(model, Math.max(0, offset));
  }
  return createFallbackPosition(monaco, offset, model.getValue());
}

function getModelOffsetAt(model: Monaco.editor.ITextModel, position: Monaco.IPosition): number {
  const getOffsetAt = (model as Monaco.editor.ITextModel & { getOffsetAt?: (position: Monaco.IPosition) => number })
    .getOffsetAt;
  if (typeof getOffsetAt === 'function') return getOffsetAt.call(model, position);
  const text = model.getValue();
  const lines = text.split('\n');
  let offset = 0;
  for (let line = 1; line < position.lineNumber; line += 1) {
    offset += (lines[line - 1]?.length ?? 0) + 1;
  }
  return offset + Math.max(0, position.column - 1);
}

function getMetadata(colorInfo: Monaco.languages.IColorInformation): ColorInfoMetadata | undefined {
  return colorInfoMetadata.get(colorInfo) ?? (colorInfo as MonacoColorInfo).__treeaseColorInfo;
}

function sliceUtf8ByBytes(text: string, startByte: number, endByte: number): string {
  if (startByte < 0 || endByte <= startByte) return '';
  const bytes = textEncoder.encode(text);
  return textDecoder.decode(bytes.slice(startByte, Math.min(endByte, bytes.length)));
}

function normalizeLanguageId(model: Monaco.editor.ITextModel): SupportedEditorLanguageId | null {
  const languageId = model.getLanguageId();
  return isSupportedLanguage(languageId) ? languageId : null;
}

function getModelDocumentKey(model: Monaco.editor.ITextModel): string {
  return ((model as Monaco.editor.ITextModel & { __treeaseDocumentKey?: string }).__treeaseDocumentKey ?? '').trim();
}

function isStaleColorResolution(
  model: Monaco.editor.ITextModel,
  token: Monaco.CancellationToken | undefined,
  initialVersionId: number,
  initialLanguageId: string,
): boolean {
  return Boolean(
    token?.isCancellationRequested ||
    model.getVersionId() !== initialVersionId ||
    model.getLanguageId() !== initialLanguageId,
  );
}

function expandRangeToFullLines(monaco: MonacoApi, model: Monaco.editor.ITextModel, range: Monaco.Range): Monaco.Range {
  const lineCount = model.getLineCount();
  const startLine = Math.max(1, range.startLineNumber - COLOR_VIEWPORT_OVERSCAN_LINES);
  const endLine = Math.min(lineCount, range.endLineNumber + COLOR_VIEWPORT_OVERSCAN_LINES);
  return new monaco.Range(startLine, 1, endLine, model.getLineMaxColumn(endLine));
}

function mergeLineRanges(monaco: MonacoApi, model: Monaco.editor.ITextModel, ranges: Monaco.Range[]): Monaco.Range[] {
  const sorted = ranges
    .slice()
    .sort((left, right) => left.startLineNumber - right.startLineNumber || left.endLineNumber - right.endLineNumber);
  const merged: Monaco.Range[] = [];
  for (const range of sorted) {
    const previous = merged[merged.length - 1];
    if (!previous || range.startLineNumber > previous.endLineNumber + 1) {
      merged.push(range);
      continue;
    }
    merged[merged.length - 1] = new monaco.Range(
      previous.startLineNumber,
      1,
      Math.max(previous.endLineNumber, range.endLineNumber),
      model.getLineMaxColumn(Math.max(previous.endLineNumber, range.endLineNumber)),
    );
  }
  return merged;
}

function getColorScanRanges(monaco: MonacoApi, model: Monaco.editor.ITextModel): Monaco.Range[] {
  const viewportState = viewportStateByModel.get(model);
  const visibleRanges = viewportState?.versionId === model.getVersionId() ? viewportState.visibleRanges : [];
  if (visibleRanges.length === 0) {
    const endLine = Math.min(model.getLineCount(), COLOR_VIEWPORT_OVERSCAN_LINES * 2);
    return [new monaco.Range(1, 1, endLine, model.getLineMaxColumn(endLine))];
  }
  return mergeLineRanges(
    monaco,
    model,
    visibleRanges.map((range) => expandRangeToFullLines(monaco, model, range)),
  );
}

async function findValueConstrainedColors(
  monaco: MonacoApi,
  model: Monaco.editor.ITextModel,
  token?: Monaco.CancellationToken,
): Promise<Monaco.languages.IColorInformation[]> {
  const languageId = normalizeLanguageId(model);
  if (!languageId) return [];
  const initialVersionId = model.getVersionId();
  const initialLanguageId = model.getLanguageId();
  const documentKey = getModelDocumentKey(model) || `${model.uri.toString()}-${model.getVersionId()}`;
  const nest = get(settings).parser.enableNest;
  const seenRanges = new Set<string>();
  const result: Monaco.languages.IColorInformation[] = [];
  const scanRanges = getColorScanRanges(monaco, model);
  let fullText: string | null = null;

  for (const scanRange of scanRanges) {
    const scanText = model.getValueInRange(scanRange);
    const scanStartOffset = getModelOffsetAt(model, {
      lineNumber: scanRange.startLineNumber,
      column: scanRange.startColumn,
    });
    for (const candidate of getCssColorMatches(scanText)) {
      if (isStaleColorResolution(model, token, initialVersionId, initialLanguageId)) return [];
      const candidateGlobalStart = scanStartOffset + candidate.start;
      const candidateGlobalEnd = scanStartOffset + candidate.end;
      const candidateOffset = candidateGlobalStart + Math.max(1, Math.floor((candidate.end - candidate.start) / 2));
      const position = getCandidatePosition(monaco, model, candidateOffset);
      const snapshotId = getWorkspaceSnapshotId(documentKey);
      const pathResult = await resolveTreePathResult(model, position, documentKey, languageId, nest, snapshotId);
      if (isStaleColorResolution(model, token, initialVersionId, initialLanguageId)) return [];
      if (pathResult.status !== 'ready') return [];
      const path = pathResult.data;
      if (!path.length) continue;
      const targetResult = await resolveEditorPositionTargetResult(model, position, path, documentKey, languageId, nest);
      if (isStaleColorResolution(model, token, initialVersionId, initialLanguageId)) return [];
      if (targetResult.status !== 'ready') return [];
      const target = targetResult.data;
      if (!target || target === 'key') continue;
      const spanResult = await resolvePathSpanResult(model, path, documentKey, languageId, 'value', nest, snapshotId);
      if (isStaleColorResolution(model, token, initialVersionId, initialLanguageId)) return [];
      if (spanResult.status !== 'ready') return [];
      const span = spanResult.data;
      if (!span) continue;
      fullText ??= model.getValue();
      const valueText = sliceUtf8ByBytes(fullText, span.startByte, span.endByte);
      const spanMatches = getCssColorMatches(valueText);
      if (spanMatches.length === 0) continue;

      const candidateByteStart = toByteColumn(fullText, candidateGlobalStart);
      const candidateByteEnd = toByteColumn(fullText, candidateGlobalEnd);
      for (const spanMatch of spanMatches) {
        const tokenStartByte = span.startByte + toByteColumn(valueText, spanMatch.start);
        const tokenEndByte = span.startByte + toByteColumn(valueText, spanMatch.end);
        if (tokenEndByte <= candidateByteStart || tokenStartByte >= candidateByteEnd) continue;
        const parsed = parseCssColor(spanMatch.text);
        if (!parsed) continue;
        const range = buildRangeFromByteOffsets(monaco, fullText, tokenStartByte, tokenEndByte);
        const rangeKey = `${range.startLineNumber}:${range.startColumn}:${range.endLineNumber}:${range.endColumn}`;
        if (seenRanges.has(rangeKey)) continue;
        seenRanges.add(rangeKey);
        const format = detectCssColorFormat(spanMatch.text);
        if (!format) continue;
        const colorInfo: MonacoColorInfo = {
          range,
          color: toMonacoColor(parsed),
          __treeaseColorInfo: {
            format,
            originalText: spanMatch.text,
            alphaByte: Math.round(parsed.a * 255),
          },
        };
        colorInfoMetadata.set(colorInfo, colorInfo.__treeaseColorInfo);
        result.push(colorInfo);
      }
    }
  }

  return result;
}

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

export function createDocumentColorRegistrar({ monaco }: ColorProviderRegistrarOptions): DocumentColorRegistrar {
  const registeredLanguages = new Set<string>();

  function ensureDocumentColorProvider(languageId: string) {
    if (!isSupportedLanguage(languageId) || registeredLanguages.has(languageId)) return;
    registeredLanguages.add(languageId);
    monaco.languages.registerColorProvider(languageId, {
      provideDocumentColors: async (model, token) => findValueConstrainedColors(monaco, model, token),
      provideColorPresentations: (model, colorInfo) => {
        const rgba = fromMonacoColor(colorInfo.color);

        const originalText = model.getValueInRange(colorInfo.range);
        const parsed = parseCssColor(originalText);
        let detectedFormat: CssColorFormat | null = null;
        if (parsed) {
          detectedFormat = detectCssColorFormat(originalText);
          const rgbClose =
            Math.abs(parsed.r - rgba.r) <= 1 && Math.abs(parsed.g - rgba.g) <= 1 && Math.abs(parsed.b - rgba.b) <= 1;
          const alphaClose = Math.abs(parsed.a - rgba.a) <= 0.01;
          if (rgbClose && alphaClose) {
            rgba.a = parsed.a;
          }
        } else {
          const metadata = getMetadata(colorInfo);
          if (metadata?.format) {
            detectedFormat = metadata.format;
            const original = parseCssColor(metadata.originalText);
            if (original) {
              const rgbClose =
                Math.abs(original.r - rgba.r) <= 1 &&
                Math.abs(original.g - rgba.g) <= 1 &&
                Math.abs(original.b - rgba.b) <= 1;
              const alphaClose = Math.abs(original.a - rgba.a) <= 0.01;
              if (rgbClose && alphaClose && metadata.alphaByte !== undefined) {
                rgba.a = metadata.alphaByte / 255;
              }
            }
          }
        }
        const hexFormat: CssColorFormat = detectedFormat === 'hexa' ? 'hexa' : 'hex';
        const rgbFormat: CssColorFormat = detectedFormat === 'rgba' ? 'rgba' : 'rgb';
        const hslFormat: CssColorFormat = detectedFormat === 'hsla' ? 'hsla' : 'hsl';
        const orderedFormats: CssColorFormat[] = [hexFormat, rgbFormat, hslFormat];

        return orderedFormats.map((format) => {
          const text = formatCssColor(rgba, format);
          return {
            label: text,
            textEdit: {
              range: colorInfo.range,
              text,
            },
          } satisfies Monaco.languages.IColorPresentation;
        });
      },
    });
  }

  function updateViewport(model: Monaco.editor.ITextModel, visibleRanges: Monaco.Range[]): void {
    viewportStateByModel.set(model, {
      visibleRanges: visibleRanges.map(
        (range) => new monaco.Range(range.startLineNumber, range.startColumn, range.endLineNumber, range.endColumn),
      ),
      versionId: model.getVersionId(),
      updatedAt: Date.now(),
    });
  }

  function refreshVisibleColors(model: Monaco.editor.ITextModel): void {
    for (const editor of monaco.editor.getEditors()) {
      if (editor.getModel() !== model) continue;
      const contribution = editor.getContribution<Monaco.editor.IEditorContribution & { updateColors?: () => void }>(
        COLOR_DETECTOR_CONTRIBUTION_ID,
      );
      contribution?.updateColors?.();
      return;
    }
  }

  return Object.assign(ensureDocumentColorProvider, {
    updateViewport,
    refreshVisibleColors,
  });
}
