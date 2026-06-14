import type { TreeNode } from '@core-wasm/index'
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import { getValueAtPath, normalizeKeyInput, renameKeyAtPath, setValueAtPath } from './graph-viewer-path';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { isPathSegKey, pathSegKeyValue, type PathSeg } from '../store/tree-path';

export type EditContext = {
  currentData: unknown;
  languageId: string;
  nest: boolean;
};

export type ValueParseArgs = {
  language: string;
  text: string;
  nest: boolean;
  path: any[];
  rawEdit: string;
  preferKey: boolean;
};

export type ValueParseResult = {
  tree: TreeNode;
  value: unknown;
};

export type ValueParser = (args: ValueParseArgs) => Promise<ValueParseResult>;

export type EditResult = {
  updated: unknown;
  nextValue: unknown;
  nextValueNode: TreeNode | null;
  editPath: any[];
  preferKey: boolean;
};

type ParsedTreeData = {
  tree: TreeNode;
  value: unknown;
};

async function parseValueToData(args: ValueParseArgs): Promise<ValueParseResult> {
  return callSharedWasmWorker<ParsedTreeData>('parseValueToData', {
    language: args.language,
    text: args.rawEdit,
    nest: args.nest,
  });
}

/**
 * 提交文本编辑。
 * @param context 编辑上下文
 * @param activeEditCell 当前编辑单元格
 * @param activeEditTarget 当前编辑目标
 * @param activeEditKind 当前编辑类型
 * @returns 编辑结果或 null
 */
export async function commitTextEdit(
  context: EditContext,
  activeEditCell: any,
  activeEditTarget: any,
  activeEditKind: string | null,
  valueParser: ValueParser = parseValueToData,
): Promise<EditResult | null> {
  if (context.currentData == null || !activeEditCell || !activeEditTarget) {
    return null;
  }
  const rawText = activeEditTarget?.text ?? '';
  const raw = typeof rawText === 'string' ? rawText : String(rawText ?? '');
  const editPath = (Array.isArray(activeEditCell.path) ? activeEditCell.path : []) as PathSeg[];
  if (!Array.isArray(editPath)) return null;

  const nextData =
    typeof structuredClone === 'function' ? structuredClone(context.currentData) : JSON.parse(JSON.stringify(context.currentData));
  const editKind = activeEditKind ?? activeEditTarget?.__graphCellKind ?? null;
  const preferKey = editKind === 'key';
  let updated: unknown = nextData;
  let nextValue: unknown = raw;
  let nextValueNode: TreeNode | null = null;

  if (preferKey) {
    nextValue = normalizeKeyInput(raw, context.languageId as SupportedEditorLanguageId);
    const lastSeg = editPath.length ? editPath[editPath.length - 1] : null;
    if (lastSeg && isPathSegKey(lastSeg) && String(nextValue) === pathSegKeyValue(lastSeg)) {
      return null;
    }
    updated = renameKeyAtPath(nextData, editPath, String(nextValue));
  } else {
    const previousValue = getValueAtPath(context.currentData, editPath);
    try {
      const data = await valueParser({
        language: context.languageId,
        text: raw,
        nest: context.nest,
        path: editPath,
        rawEdit: raw,
        preferKey: false,
      });
      nextValueNode = data.tree;
      nextValue = data.value;
    } catch {
      return null;
    }
    if (Object.is(previousValue, nextValue)) {
      return null;
    }
    updated = setValueAtPath(nextData, editPath, nextValue);
    const updatedValue = getValueAtPath(updated, editPath);
    if (Object.is(previousValue, updatedValue)) {
      return null;
    }
  }

  return { updated, nextValue, nextValueNode, editPath, preferKey };
}
