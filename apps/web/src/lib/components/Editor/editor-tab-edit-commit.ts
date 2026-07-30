import type { BuilderConfig, DocumentJobSettings, DocumentTextEdit, SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';

import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { DocumentAnalysisResult } from '../../../shared/worker-protocol/protocol';
import { createFreshnessScope } from '../../guards/freshness-scope';
import { runEditorCommitTransaction } from '../../services/EditorCommitTransaction';

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
  runUsage?: (source: string, execute: () => Promise<unknown>) => Promise<unknown>;
  isFresh: (state: { revision: number }) => boolean;
  applyCommittedSourceText?: (sourceText: string) => void;
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
  const freshness = createFreshnessScope(
    {
      documentKey: options.requestDocumentKey,
      languageId: options.requestLanguage,
      revision,
      token: revision,
      model: options.requestModel,
    },
    () => ({
      documentKey: options.requestDocumentKey,
      languageId: options.requestLanguage,
      revision,
      token: options.isFresh({ revision }) ? revision : -1,
      model: options.requestModel,
    }),
  );

  const commit = () => runEditorCommitTransaction({
    documentKey: options.requestDocumentKey,
    language: options.requestLanguage,
    revision,
    settings: options.settings,
    builderConfig: options.builderConfig,
    intent: {
      kind: 'applyEdits',
      edits: options.documentTextEdits,
      baseSnapshotId: options.baseSnapshotId,
    },
    freshness,
    landing: {
      writeSourceText: (sourceText) => {
        if (sourceText !== options.requestModel.getValue()) {
          options.applyCommittedSourceText?.(sourceText);
        }
      },
      applyAnalysis: (analysis) =>
        options.applyGraphAnalysis(
          options.requestModel,
          options.requestLanguage,
          options.requestDocumentKey,
          revision,
          analysis,
        ),
    },
  });
  void (options.runUsage ? options.runUsage(options.nextText, commit) : commit());
  return revision;
}
