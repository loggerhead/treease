// 职责：GraphViewer 单元格值编辑：inline editor 创建/提交/取消、WASM applyValueEdit 调用
import { InnerEditorEvent } from '@leafer-in/editor';
import type { DocumentTextEdit } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { GraphCell, GraphCellKind } from '../../graph/graph-viewer-render';
import type { PathSeg } from '../../store/tree-path';
import type { EditorIO } from '../../store/editor-store';
import { clearGraphSelectionAfterEdit } from '../GraphViewer.graph-highlight';
import { commitTextEdit as commitGraphTextEdit, type ValueParseResult } from '../../graph/GraphEditHandler';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import { treeNodeToValue } from '../../../shared/tree-node-value';
import type { TreeNode } from '@core-wasm/index'
import type { LeaferEditor, LeaferText } from './model';
import { resolveCellPath } from './graph-anchor-index';
import { createFreshnessScope } from '../../guards/freshness-scope';

type GraphEditEventType = 'graph-edit-open' | 'graph-edit-commit';

type PlannedGraphValueEditResult =
  | {
      mode: 'edits';
      edits: DocumentTextEdit[];
      text: string;
      tree: TreeNode;
      value: unknown;
    }
  | {
      mode: 'replace';
      reason:
        | 'graph-edit-not-single-range'
        | 'snapshotNotReady'
        | 'missingAnalysis'
        | 'missingDocument'
        | 'invalidPath'
        | 'invalidReplacement'
        | 'unsupportedLanguage'
        | 'unsupportedEdit'
        | 'unsafeEdit';
      text: string;
      tree: TreeNode;
      value: unknown;
    };

type GraphValueEditControllerDeps = {
  getCurrentData: () => unknown;
  getSourceText: () => string;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getEnableNest: () => boolean;
  isReadonly?: () => boolean;
  getEditorIO: () => EditorIO | null;
  getEditorRevision: () => number;
  getActiveSnapshotId: () => number | null;
  resolveTreePathByPosition: (row: number, column: number) => Promise<PathSeg[]>;
  nextTreeStateToken: () => number;
  publishTreeState: (
    requestId: number,
    tree: TreeNode | null,
    value: unknown,
    source: 'graph',
    revision: number,
  ) => boolean;
  emitEditorMutation: (mutation: { type: 'replaceSourceText'; payload: { text: string } }) => void;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  refreshTooltipVisibility: () => void;
  dispatchGraphEditEvent: (type: GraphEditEventType, detail: unknown) => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
};



export function createGraphValueEditController(deps: GraphValueEditControllerDeps) {
  type ActiveEditState = {
    cell: GraphCell | null;
    target: LeaferText | null;
    kind: GraphCellKind | null;
    initialText: string | null;
  };

  const activeEditStateByEditor = new Map<LeaferEditor, ActiveEditState>();
  const boundGraphEditors = new Set<LeaferEditor>();

  function isReadonly(): boolean {
    return deps.isReadonly?.() === true;
  }

  function getEditorActiveEditState(editor: LeaferEditor): ActiveEditState {
    let state = activeEditStateByEditor.get(editor);
    if (!state) {
      state = {
        cell: null,
        target: null,
        kind: null,
        initialText: null,
      };
      activeEditStateByEditor.set(editor, state);
    }
    return state;
  }

  function getFirstActiveEditState(): ActiveEditState | null {
    for (const state of activeEditStateByEditor.values()) {
      if (state.cell && state.target) return state;
    }
    return null;
  }

  function getActiveEditCellPath(): PathSeg[] | undefined {
    return getFirstActiveEditState()?.cell?.path;
  }

  function hasActiveEdit(): boolean {
    return !!getFirstActiveEditState();
  }

  function resetActiveEditState(editor?: LeaferEditor | null): void {
    if (editor) {
      const state = activeEditStateByEditor.get(editor);
      if (state) {
        state.cell = null;
        state.target = null;
        state.kind = null;
        state.initialText = null;
      }
      deps.refreshTooltipVisibility();
      return;
    }
    for (const state of activeEditStateByEditor.values()) {
      state.cell = null;
      state.target = null;
      state.kind = null;
      state.initialText = null;
    }
    deps.refreshTooltipVisibility();
  }

  async function applyGraphEdit(
    editCell: GraphCell,
    editKind: 'key' | 'value',
    raw: string,
    editTargetOverride: LeaferText | null = null,
  ): Promise<boolean> {
    if (isReadonly()) {
      return false;
    }
    if (!editCell) {
      return false;
    }
    const currentData = deps.getCurrentData();
    const editorIO = deps.getEditorIO();
    const getCurrentFreshnessContext = () => {
      const currentEditorIO = deps.getEditorIO();
      return {
        documentKey: deps.getDocumentKey(),
        revision: deps.getEditorRevision(),
        languageId: deps.getLanguageId(),
        model: currentEditorIO?.context === 'editor' ? currentEditorIO.getModel() : null,
      };
    };
    const freshness = createFreshnessScope(
      {
        documentKey: deps.getDocumentKey(),
        revision: deps.getEditorRevision(),
        languageId: deps.getLanguageId(),
        model: editorIO?.context === 'editor' ? editorIO.getModel() : null,
      },
      getCurrentFreshnessContext,
    );
    if (!freshness.isCurrent()) {
      return false;
    }
    const editPath = await freshness.step(() =>
      resolveCellPath(editCell, deps.resolveTreePathByPosition, editCell.path ?? []),
    );
    if (!editPath?.length) {
      return false;
    }
    deps.dispatchGraphEditEvent('graph-edit-commit', {
      path: editPath,
      kind: editKind,
      text: raw,
      valueType: editCell?.valueType,
    });
    const parseGraphValue = async ({
      language,
      path,
      rawEdit,
      preferKey,
      nest,
    }: {
      language: string;
      path: PathSeg[];
      rawEdit: string;
      preferKey: boolean;
      nest: boolean;
    }): Promise<ValueParseResult> => {
      if (!preferKey && editCell?.valueType === 'string') {
        const normalizedStringEdit = editCell?.text === '' && rawEdit === '""' ? '' : rawEdit;
        const tree = await callSharedWasmWorker<TreeNode>('valueToTreeNode', { value: normalizedStringEdit });
        return { tree, value: normalizedStringEdit } satisfies ValueParseResult;
      }
      try {
        const tree = await callSharedWasmWorker<TreeNode>('parseValueForPath', {
          language,
          documentKey: deps.getDocumentKey(),
          text: deps.getSourceText(),
          path,
          rawEdit,
          preferKey,
          nest,
        });
        return { tree, value: treeNodeToValue(tree) } satisfies ValueParseResult;
      } catch (error) {
        deps.handleError(error, {
          component: 'GraphViewer',
          operation: 'parseValueForPath',
          metadata: { language },
        });
        throw error;
      }
    };
    let result: Awaited<ReturnType<typeof commitGraphTextEdit>> = null;
    if (currentData != null) {
      result = await freshness.step(() =>
        commitGraphTextEdit(
          {
            currentData,
            languageId: deps.getLanguageId(),
            nest: deps.getEnableNest(),
          },
          { ...editCell, path: editPath },
          { ...editTargetOverride, text: raw, __graphCellKind: editKind, __graphCellValueType: editCell?.valueType },
          editKind,
          parseGraphValue,
        ),
      );
    }
    if (!result && editKind === 'value' && raw !== String(editCell?.text ?? '')) {
      try {
        const parsed = await freshness.step(() =>
          parseGraphValue({
            language: deps.getLanguageId(),
            path: editPath,
            rawEdit: raw,
            preferKey: false,
            nest: deps.getEnableNest(),
          }),
        );
        result = {
          updated: null,
          nextValue: parsed.value,
          nextValueNode: parsed.tree,
          editPath,
          preferKey: false,
        };
      } catch {
        return false;
      }
    }
    if (!result || !editorIO || editorIO.context !== 'editor') {
      return false;
    }
    const { nextValue, nextValueNode, preferKey } = result;
    const canonicalNextValueNode =
      nextValueNode ??
      (await freshness.step(() => callSharedWasmWorker<TreeNode>('valueToTreeNode', { value: nextValue })));
    if (!canonicalNextValueNode) {
      return false;
    }
    const planned = await freshness.step(() =>
      callSharedWasmWorker<PlannedGraphValueEditResult>('planGraphValueEdit', {
        documentKey: deps.getDocumentKey(),
        snapshotId: deps.getActiveSnapshotId(),
        language: deps.getLanguageId(),
        text: deps.getSourceText(),
        path: editPath,
        preferKey,
        value: canonicalNextValueNode,
        nest: deps.getEnableNest(),
      }),
    );
    if (!planned) {
      return false;
    }
    const requestId = deps.nextTreeStateToken();
    if (!deps.publishTreeState(requestId, planned.tree, planned.value, 'graph', deps.getEditorRevision() + 1)) {
      return false;
    }
    deps.updateActiveTempModel((current) => clearGraphSelectionAfterEdit(current, editPath));
    if (planned.mode === 'replace') {
      deps.emitEditorMutation({
        type: 'replaceSourceText',
        payload: { text: planned.text },
      });
      return true;
    }
    const applied = editorIO.applyTextEdits(planned.edits);
    return applied;
  }

  async function commitTooltipPanelProbe(
    probe: { cell: GraphCell; kind: GraphCellKind },
    text: string,
  ): Promise<boolean> {
    if (isReadonly()) {
      return false;
    }
    if (probe.kind !== 'key' && probe.kind !== 'value') {
      return false;
    }
    deps.dispatchGraphEditEvent('graph-edit-open', {
      path: probe.cell.path,
      kind: probe.kind,
      valueType: probe.cell.valueType,
    });
    const applied = await applyGraphEdit(probe.cell, probe.kind, text, null);
    return applied;
  }

  async function commitTextEdit(editor?: LeaferEditor | null): Promise<void> {
    if (isReadonly()) {
      resetActiveEditState(editor);
      return;
    }
    const state = editor ? (activeEditStateByEditor.get(editor) ?? null) : getFirstActiveEditState();
    if (deps.getCurrentData() == null || !state?.cell || !state.target) {
      resetActiveEditState(editor);
      return;
    }
    const editCell = state.cell;
    const editTarget = state.target;
    const rawText = editTarget.text;
    const raw = typeof rawText === 'string' ? rawText : String(rawText ?? '');
    if (state.initialText != null && raw === state.initialText) {
      resetActiveEditState(editor);
      return;
    }
    const editKind = state.kind ?? editTarget.__graphCellKind ?? null;
    if (editKind !== 'key' && editKind !== 'value') {
      resetActiveEditState(editor);
      return;
    }
    try {
      await applyGraphEdit(editCell, editKind, raw, editTarget);
    } finally {
      resetActiveEditState(editor);
    }
  }

  function bindGraphEditorLifecycle(editor: LeaferEditor | null): void {
    if (!editor || boundGraphEditors.has(editor)) return;
    boundGraphEditors.add(editor);
    getEditorActiveEditState(editor);
    const textEditor = editor.getInnerEditor?.('TextEditor') ?? editor.innerEditor;
    if (textEditor?.config) {
      textEditor.config.selectAll = true;
    }
    editor.on?.(InnerEditorEvent.BEFORE_OPEN, (event: unknown) => {
      const target = (event as { editTarget?: LeaferText })?.editTarget ?? null;
      const cell = target?.__graphCell ?? null;
      if (!cell) return;
      if (isReadonly()) {
        resetActiveEditState(editor);
        return;
      }
      const state = getEditorActiveEditState(editor);
      state.cell = cell;
      state.target = target;
      state.kind = target?.__graphCellKind ?? null;
      const rawText = target?.text;
      state.initialText = typeof rawText === 'string' ? rawText : String(rawText ?? '');
      if (state.kind === 'key' || state.kind === 'value') {
        deps.dispatchGraphEditEvent('graph-edit-open', {
          path: cell.path,
          kind: state.kind,
          valueType: (cell as { valueType?: string })?.valueType,
        });
      }
    });
    editor.on?.(InnerEditorEvent.CLOSE, () => {
      void commitTextEdit(editor);
    });
  }

  return {
    applyGraphEdit,
    bindGraphEditorLifecycle,
    commitTextEdit,
    commitTooltipPanelProbe,
    getActiveEditCellPath,
    hasActiveEdit,
    resetActiveEditState,
  };
}
