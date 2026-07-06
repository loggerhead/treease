// 职责：Editor hover 预览控制器：hover 位置计算、TreeNode 值读取、preview 触发
import type * as Monaco from 'monaco-editor';
import type { PathSeg, SnapshotReadResult } from '@core-wasm/index'
import type { PreviewContext } from '../../preview/types';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { supportedEditorLanguageIds } from '../../monaco/language-support';
import type { TreeSyncState } from '../../store/graph-selection-store';
import { generatePreview } from '../../preview';
import { isPreviewableNode, readTreeNodeString } from '../../preview/tree-node';
import { resolvePathSpanResult, resolveTreePathResult } from '../../services/TreePathService';
import { getWorkspaceSnapshotId } from '../../store/workspace-snapshot-bindings';
import { nodePreviewToTreeNode, queryNodePreview, queryPathValue } from '../../services/SnapshotProjectionService';
import { valueToTreeNode } from '../../../shared/tree-node-value';
import { resolveEditorPositionTargetResult } from './editor-position-target';

type RegisterEditorHoverPreviewOptions = {
  monaco: typeof Monaco;
  editor: Monaco.editor.IStandaloneCodeEditor;
  getTreeState: () => TreeSyncState;
  getRevision: () => number;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getNestEnabled: () => boolean;
  isImportActive?: () => boolean;
};

const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder();

function sliceUtf8ByBytes(text: string, startByte: number, endByte: number): string {
  if (startByte < 0 || endByte <= startByte) return '';
  const bytes = utf8Encoder.encode(text);
  return utf8Decoder.decode(bytes.slice(startByte, Math.min(endByte, bytes.length)));
}

function buildPreviewContextFromNodePreview(
  preview: ReturnType<typeof nodePreviewToTreeNode> | null,
  rawValue: string,
  language: SupportedEditorLanguageId,
): PreviewContext | null {
  const node = preview;
  if (!isPreviewableNode(node)) return null;
  const value = readTreeNodeString(node);
  return {
    node,
    value,
    rawValue: rawValue || value,
    language,
  };
}

function normalizePreviewValue(
  rawValue: string,
  _language: SupportedEditorLanguageId,
): { rawValue: string; value: string } | null {
  const normalized = rawValue.trim();
  if (!normalized) return null;
  if (
    (normalized.startsWith('"') && normalized.endsWith('"')) ||
    (normalized.startsWith("'") && normalized.endsWith("'"))
  ) {
    return {
      rawValue: normalized,
      value: normalized.slice(1, -1),
    };
  }
  return {
    rawValue: normalized,
    value: normalized,
  };
}

function buildPreviewContextFromRawValue(rawValue: string, language: SupportedEditorLanguageId): PreviewContext | null {
  const normalized = normalizePreviewValue(rawValue, language);
  if (!normalized?.value) return null;
  return {
    node: valueToTreeNode(normalized.value),
    value: normalized.value,
    rawValue: normalized.rawValue,
    language,
  };
}

function toHoverContents(htmlOrHtmls: string | string[]) {
  const htmls = typeof htmlOrHtmls === 'string' ? [htmlOrHtmls] : htmlOrHtmls;
  return htmls.map((value) => ({
    isTrusted: true,
    supportHtml: true,
    value,
  }));
}

function getModelVersion(model: Monaco.editor.ITextModel): number | null {
  const getVersionId = (model as Monaco.editor.ITextModel & { getVersionId?: () => number }).getVersionId;
  return typeof getVersionId === 'function' ? getVersionId.call(model) : null;
}

function isPositionInsideModel(model: Monaco.editor.ITextModel, position: Monaco.IPosition | null): boolean {
  if (!position || position.lineNumber < 1 || position.column < 1) return false;
  const getLineCount = (model as Monaco.editor.ITextModel & { getLineCount?: () => number }).getLineCount;
  if (typeof getLineCount !== 'function') return true;
  const lineCount = getLineCount.call(model);
  if (!Number.isFinite(lineCount) || position.lineNumber > lineCount) return false;
  const getLineMaxColumn = (model as Monaco.editor.ITextModel & { getLineMaxColumn?: (lineNumber: number) => number })
    .getLineMaxColumn;
  if (typeof getLineMaxColumn !== 'function') return true;
  try {
    return position.column <= getLineMaxColumn.call(model, position.lineNumber);
  } catch {
    return false;
  }
}

async function resolveHoverPreviewContext(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  language: SupportedEditorLanguageId,
  nest: boolean,
  snapshotId: number | null,
): Promise<SnapshotReadResult<PreviewContext | null>> {
  const [spanResult, nodePreview, pathValue] = await Promise.all([
    resolvePathSpanResult(model, path, documentKey, language, 'value', nest, snapshotId),
    queryNodePreview({ documentKey, snapshotId, path }),
    queryPathValue({ documentKey, snapshotId, path }),
  ]);
  if (spanResult.status !== 'ready' || nodePreview.status !== 'ready' || pathValue.status !== 'ready') {
    return { status: 'snapshotNotReady' };
  }
  const span = spanResult.data;
  const readyNodePreview = nodePreview.status === 'ready' ? nodePreview.data : null;
  const readyPathValue = pathValue.status === 'ready' ? pathValue.data : null;
  const rawValue =
    readyPathValue?.sourceText || (span ? sliceUtf8ByBytes(model.getValue(), span.startByte, span.endByte) : '');
  const node = readyNodePreview ? nodePreviewToTreeNode(readyNodePreview) : null;
  return {
    status: 'ready',
    data: buildPreviewContextFromNodePreview(node, rawValue, language) ?? buildPreviewContextFromRawValue(rawValue, language),
  };
}

export function registerEditorHoverPreview({
  monaco,
  editor,
  getTreeState,
  getRevision,
  getDocumentKey,
  getLanguageId,
  getNestEnabled,
  isImportActive,
}: RegisterEditorHoverPreviewOptions): Monaco.IDisposable {
  const disposables = supportedEditorLanguageIds.map((languageId) =>
    monaco.languages.registerHoverProvider(languageId, {
      provideHover: async (model, position) => {
        if (isImportActive?.()) return null;
        if (editor.getModel() !== model) return null;
        if (!isPositionInsideModel(model, position)) return null;
        const requestRevision = getRevision();
        const requestDocumentKey = getDocumentKey();
        const activeLanguageId = getLanguageId();
        const requestVersion = getModelVersion(model);
        const isCurrent = () =>
          !isImportActive?.() &&
          editor.getModel() === model &&
          getRevision() === requestRevision &&
          getDocumentKey() === requestDocumentKey &&
          getLanguageId() === activeLanguageId &&
          getModelVersion(model) === requestVersion &&
          isPositionInsideModel(model, position);
        const treeState = getTreeState();
        if (treeState.revision !== requestRevision) return null;
        if (activeLanguageId !== languageId) return null;
        const nest = getNestEnabled();
        const snapshotId = getWorkspaceSnapshotId(requestDocumentKey);
        const pathResult = await resolveTreePathResult(model, position, requestDocumentKey, activeLanguageId, nest, snapshotId);
        if (!isCurrent()) return null;
        if (pathResult.status !== 'ready') return null;
        const path = pathResult.data;
        if (!path?.length) return null;
        const targetResult = await resolveEditorPositionTargetResult(model, position, path, requestDocumentKey, activeLanguageId, nest);
        if (!isCurrent()) return null;
        if (targetResult.status !== 'ready') return null;
        const target = targetResult.data;
        if (!target || target === 'key') return null;
        const previewContextResult = await resolveHoverPreviewContext(
          model,
          path,
          requestDocumentKey,
          activeLanguageId,
          nest,
          snapshotId,
        );
        if (!isCurrent()) return null;
        if (previewContextResult.status !== 'ready') return null;
        const previewContext = previewContextResult.data;
        if (!previewContext) return null;
        const htmlOrHtmls = await generatePreview(previewContext);
        if (!isCurrent()) return null;
        if (!htmlOrHtmls) return null;
        return {
          contents: toHoverContents(htmlOrHtmls),
        };
      },
    }),
  );
  return {
    dispose() {
      for (const disposable of disposables) {
        disposable.dispose();
      }
    },
  };
}
