import type { DocumentTextEdit } from '@core-wasm/index';
import type { TreeNode } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';

import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { EditorWorkspaceState } from './editor-workspace';
import type { PathSeg } from './tree-path';

export type EditorIoContext = 'editor' | 'scratch';

export type EditorIO = {
  context: EditorIoContext;
  getModel: () => Monaco.editor.ITextModel | null;
  getText: () => string;
  setText: (value: string) => void;
  applyTextEdits: (edits: DocumentTextEdit[]) => boolean;
  getLanguage: () => SupportedEditorLanguageId;
};

export type DiagnosticContextLine = {
  lineNumber: number;
  text: string;
};

export type DiagnosticItem = {
  code: 'syntax-error' | 'missing-node';
  message: string;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  context: DiagnosticContextLine[];
};

export type GraphHighlightTarget = 'key' | 'value' | 'node';
export type TreeSelectionSource = 'editor' | 'graph' | 'breadcrumb' | 'search';

export type GraphHighlightState = {
  path: PathSeg[];
  target?: GraphHighlightTarget;
  revision: number;
  source: TreeSelectionSource;
  revealToken?: number;
};

export type TempModel = {
  diffInputText: string;
  scratchText: string;
  commandQuery: string;
  status: string;
  error: string;
  cursor: string;
  selectionLength: number;
  treePath: PathSeg[];
  graphHighlight: GraphHighlightState | null;
  diagnostics: DiagnosticItem[];
};

export type GraphEditReplaceFallbackReason =
  | 'graph-edit-not-single-range'
  | 'missingAnalysis'
  | 'missingDocument'
  | 'invalidPath'
  | 'invalidReplacement'
  | 'unsupportedLanguage'
  | 'unsupportedEdit'
  | 'unsafeEdit';

export type EditorMutation = {
  type: 'replaceSourceText';
  payload: {
    text: string;
    graphEditFallback?: {
      reason: GraphEditReplaceFallbackReason;
      path: PathSeg[];
      kind: 'key' | 'value';
    };
  };
};

export type EditorMutationEnvelope = { id: number; mutation: EditorMutation };

export type TreeSyncSource = 'editor' | 'graph';

export type TreeSyncState = {
  tree: TreeNode | null;
  value: unknown;
  revision: number;
  source: TreeSyncSource;
};

export type FullEditUiPhase = 'idle' | 'preparing' | 'streaming' | 'finalizing' | 'settled';
export type FullEditSessionKind = 'full-edit';
export type FullEditTransportKind = 'memory' | 'file';

export type FullEditUiState = {
  active: boolean;
  sessionId: string | null;
  ownerKey: string | null;
  documentKey: string | null;
  revision: number;
  streamSeq: number;
  inputByteLength: number;
  modelVersionId: number | null;
  byteLength: number;
  language: SupportedEditorLanguageId | '';
  phase: FullEditUiPhase;
  sessionKind: FullEditSessionKind | null;
  transportKind: FullEditTransportKind | null;
  reason:
    | 'initial-example'
    | 'language-example'
    | 'language-switch'
    | 'whole-document-replacement'
    | 'tab-reactivate'
    | 'import-file'
    | 'drop-file'
    | null;
};

export type JsonBlockSelection = {
  sourceDocumentKey: string;
  blockDocumentKey: string;
  revision: number;
  language: 'json';
  text: string;
  startByte: number;
  endByte: number;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
};

export type EditorState = {
  sourceText: string;
  previousSourceText: string;
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  compareEditToken: number;
  editorRevision: number;
  graphAppliedRevision: number;
  editorIO: EditorIO | null;
  editorMutation: EditorMutationEnvelope | null;
  treeState: TreeSyncState;
  fullEditUiState: FullEditUiState;
  jsonBlockSelection: JsonBlockSelection | null;
  tempModel: TempModel;
  workspace: EditorWorkspaceState;
};

export type DocumentSessionState = Pick<
  EditorState,
  | 'sourceText'
  | 'previousSourceText'
  | 'documentKey'
  | 'languageId'
  | 'compareEditToken'
  | 'editorRevision'
  | 'graphAppliedRevision'
  | 'editorIO'
>;
