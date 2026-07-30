// Responsibility: edit GraphViewer cell values, including inline editor create/commit/cancel and WASM applyValueEdit calls.
import type { DocumentTextEdit } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { GraphCell, GraphCellKind } from '@treease/graph-viewer-runtime';
import type { EditorIO } from '../../store/document-session-store';
import type { GraphEditReplaceFallbackReason } from '../../store/editor-store';
import type { PathSeg } from '../../store/tree-path';
import { clearGraphSelectionAfterEdit } from '../GraphViewer.graph-highlight';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { TreeNode } from '@core-wasm/index'
import type { LeaferEditor, LeaferText } from './model';
import { resolveCellPath } from './graph-anchor-index';
import { createFreshnessScope } from '../../guards/freshness-scope';
import { trackEvent } from '../../analytics/ga4';

// These public Leafer event names let this controller stay outside the lazy
// Leafer runtime chunk; importing the package here defeats GraphRuntimeHost's boundary.
const INNER_EDITOR_BEFORE_OPEN = 'innerEditor.before_open';
const INNER_EDITOR_CLOSE = 'innerEditor.close';

type GraphEditEventType =
  | 'graph-edit-open'
  | 'graph-edit-commit'
  | 'graph-edit-replace-fallback'
  | 'graph-edit-result';

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
      reason: GraphEditReplaceFallbackReason;
      text: string;
      tree: TreeNode;
      value: unknown;
    }
  | {
      mode: 'snapshotNotReady';
    };

/** A value edit expressed independently of a Leafer graph cell. */
export type StructuredValueEditIntent = {
  path: PathSeg[];
  kind: 'key' | 'value';
  raw: string;
  valueType?: string;
  text?: string;
  /** Snapshot that produced the editable projection, when the caller owns one. */
  snapshotId?: number | null;
  /** Keep the exact source literal instead of structurally re-encoding it. */
  preserveSourceFormatting?: boolean;
};

export type GraphValueEditControllerDeps = {
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
  emitEditorMutation: (mutation: {
    type: 'replaceSourceText';
    payload: {
      text: string;
      graphEditFallback?: {
        reason: GraphEditReplaceFallbackReason;
        path: PathSeg[];
        kind: 'key' | 'value';
      };
    };
  }) => void;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  dispatchGraphEditEvent: (type: GraphEditEventType, detail: unknown) => void;
  runBidirectionalEdit?: <T>(documentKey: string, execute: () => Promise<T>) => Promise<T>;
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

  class GraphEditNotAppliedError extends Error {}

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
      return;
    }
    for (const state of activeEditStateByEditor.values()) {
      state.cell = null;
      state.target = null;
      state.kind = null;
      state.initialText = null;
    }
  }

  async function applyStructuredValueEdit(intent: StructuredValueEditIntent): Promise<boolean> {
    if (isReadonly()) {
      return false;
    }
    if (!intent.path.length) {
      return false;
    }
    if (intent.snapshotId != null && intent.snapshotId !== deps.getActiveSnapshotId()) {
      deps.dispatchGraphEditEvent('graph-edit-result', { applied: false, reason: 'snapshot-stale', path: intent.path });
      return false;
    }
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
    if (!editorIO || editorIO.context !== 'editor') {
      return false;
    }
    deps.dispatchGraphEditEvent('graph-edit-commit', {
      path: intent.path,
      kind: intent.kind,
      text: intent.raw,
      valueType: intent.valueType,
    });
    const parseGraphValue = async ({
      language,
      path,
      rawEdit,
      preferKey,
      nest,
      strictSourceLiteral,
    }: {
      language: string;
      path: PathSeg[];
      rawEdit: string;
      preferKey: boolean;
      nest: boolean;
      strictSourceLiteral: boolean;
    }): Promise<TreeNode> => {
      try {
        const normalizedRawEdit =
          !preferKey && intent.valueType === 'string' && intent.text === '' && rawEdit === '""' ? '' : rawEdit;
        return await callSharedWasmWorker<TreeNode>('parseValueForPath', {
          language,
          documentKey: deps.getDocumentKey(),
          text: deps.getSourceText(),
          path,
          rawEdit: normalizedRawEdit,
          preferKey,
          nest,
          strictSourceLiteral,
        });
      } catch (error) {
        deps.handleError(error, {
          component: 'GraphViewer',
          operation: 'parseValueForPath',
          metadata: { language },
        });
        throw error;
      }
    };
    const preferKey = intent.kind === 'key';
    const canonicalNextValueNode = await freshness.step(() =>
      parseGraphValue({
        language: deps.getLanguageId(),
        path: intent.path,
        rawEdit: intent.raw,
        preferKey,
        nest: deps.getEnableNest(),
        strictSourceLiteral: intent.preserveSourceFormatting === true,
      }),
    );
    if (!canonicalNextValueNode) {
      deps.dispatchGraphEditEvent('graph-edit-result', { applied: false, reason: 'parse-or-stale', path: intent.path });
      return false;
    }
    const planned = await freshness.step(() =>
      callSharedWasmWorker<PlannedGraphValueEditResult>('planGraphValueEdit', {
        documentKey: deps.getDocumentKey(),
        snapshotId: intent.snapshotId ?? deps.getActiveSnapshotId(),
        language: deps.getLanguageId(),
        text: deps.getSourceText(),
        path: intent.path,
        preferKey,
        value: canonicalNextValueNode,
        nest: deps.getEnableNest(),
        rawReplacement: intent.preserveSourceFormatting ? intent.raw : undefined,
      }),
    );
    if (!planned) {
      deps.dispatchGraphEditEvent('graph-edit-result', { applied: false, reason: 'plan-or-stale', path: intent.path });
      return false;
    }
    deps.updateActiveTempModel((current) => clearGraphSelectionAfterEdit(current, intent.path));
    if (planned.mode === 'snapshotNotReady') {
      deps.dispatchGraphEditEvent('graph-edit-result', { applied: false, reason: 'snapshot-not-ready', path: intent.path });
      return false;
    }
    const apply = async (): Promise<boolean> => {
      if (planned.mode === 'replace') {
        const graphEditFallback = {
          reason: planned.reason,
          path: intent.path,
          kind: intent.kind,
        };
        deps.dispatchGraphEditEvent('graph-edit-replace-fallback', {
          ...graphEditFallback,
          documentKey: deps.getDocumentKey(),
          language: deps.getLanguageId(),
          snapshotId: intent.snapshotId ?? deps.getActiveSnapshotId(),
        });
        deps.emitEditorMutation({
          type: 'replaceSourceText',
          payload: {
            text: planned.text,
            graphEditFallback,
          },
        });
        return true;
      }
      if (!editorIO.applyTextEdits(planned.edits)) {
        throw new GraphEditNotAppliedError('Graph edit was not applied');
      }
      return true;
    };

    try {
      const applied = deps.runBidirectionalEdit
        ? await deps.runBidirectionalEdit(deps.getDocumentKey(), apply)
        : await apply();
      deps.dispatchGraphEditEvent('graph-edit-result', {
        applied,
        reason: applied ? 'applied' : 'write-not-applied',
        path: intent.path,
      });
      if (applied) trackEvent('graph_edit', { edit_type: intent.kind, result: 'success' });
      return applied;
    } catch (error) {
      if (error instanceof GraphEditNotAppliedError) {
        deps.dispatchGraphEditEvent('graph-edit-result', { applied: false, reason: 'write-not-applied', path: intent.path });
        return false;
      }
      throw error;
    }
  }

  async function applyGraphEdit(
    editCell: GraphCell,
    editKind: 'key' | 'value',
    raw: string,
    _editTargetOverride: LeaferText | null = null,
  ): Promise<boolean> {
    if (isReadonly() || !editCell || editCell.isMissing) return false;
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
    const path = await freshness.step(() =>
      resolveCellPath(editCell, deps.resolveTreePathByPosition, editCell.path ?? []),
    );
    if (!path?.length) return false;
    return applyStructuredValueEdit({
      path,
      kind: editKind,
      raw,
      valueType: editCell.valueType,
      text: editCell.text,
    });
  }

  async function commitTextEdit(editor?: LeaferEditor | null): Promise<void> {
    if (isReadonly()) {
      resetActiveEditState(editor);
      return;
    }
    const state = editor ? (activeEditStateByEditor.get(editor) ?? null) : getFirstActiveEditState();
    if (!state?.cell || !state.target) {
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
    editor.on?.(INNER_EDITOR_BEFORE_OPEN, (event: unknown) => {
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
    editor.on?.(INNER_EDITOR_CLOSE, () => {
      void commitTextEdit(editor);
    });
  }

  return {
    applyGraphEdit,
    applyStructuredValueEdit,
    bindGraphEditorLifecycle,
    commitTextEdit,
    getActiveEditCellPath,
    hasActiveEdit,
    resetActiveEditState,
  };
}
