import type * as Monaco from 'monaco-editor';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { DocumentOrigin } from '../../document-origin';

export type EditorModelWithDocumentKey = Monaco.editor.ITextModel & {
  __treeaseDocumentKey?: string;
  __treeaseDocumentVersion?: number;
};

export type EditorTab = {
  id: string;
  name: string;
  languageId: SupportedEditorLanguageId;
  origin: DocumentOrigin;
  documentKey: string;
  model: EditorModelWithDocumentKey;
};

export type TabSummary = {
  id: string;
  name: string;
  languageId: SupportedEditorLanguageId;
};
