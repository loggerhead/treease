import type { BuilderConfig, DocumentJobSettings, DocumentTextEdit, SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';

import { commitApplyEdits } from '../../services/DocumentCommitService';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';

export type EditorTabSnapshotBinding = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
};

type CommitEditorTabTextChangeOptions = {
  requestModel: Monaco.editor.ITextModel;
  requestLanguage: SupportedEditorLanguageId;
  requestDocumentKey: string;
  nextText: string;
  documentTextEdits: DocumentTextEdit[];
  baseSnapshotId: SnapshotId | null;
  settings: DocumentJobSettings;
  builderConfig: BuilderConfig;
  commitRevision: () => number;
  isFresh: (state: { revision: number }) => boolean;
  applyCommittedSourceText?: (sourceText: string) => void;
  bindSnapshot: (binding: EditorTabSnapshotBinding) => void;
  applyGraphAnalysis: (
    requestModel: Monaco.editor.ITextModel,
    requestLanguage: SupportedEditorLanguageId,
    requestDocumentKey: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
  ) => Promise<void>;
};

export function commitEditorTabTextChange(options: CommitEditorTabTextChangeOptions): number {
  const revision = options.commitRevision();
  void commitApplyEdits({
    documentKey: options.requestDocumentKey,
    language: options.requestLanguage,
    edits: options.documentTextEdits,
    baseSnapshotId: options.baseSnapshotId,
    revision,
    settings: options.settings,
    builderConfig: options.builderConfig,
  }).then((result) => {
    if (
      result.sourceText != null &&
      options.documentTextEdits.length === 0 &&
      options.isFresh({ revision }) &&
      result.sourceText !== options.requestModel.getValue()
    ) {
      options.applyCommittedSourceText?.(result.sourceText);
    }
    if (result.status === 'snapshotReady' && options.isFresh({ revision })) {
      options.bindSnapshot({
        documentKey: options.requestDocumentKey,
        revision,
        snapshotId: result.snapshotId,
      });
    }
    if (result.analysis != null && options.isFresh({ revision })) {
      void options.applyGraphAnalysis(
        options.requestModel,
        options.requestLanguage,
        options.requestDocumentKey,
        revision,
        result.analysis,
      );
    }
  });
  return revision;
}
