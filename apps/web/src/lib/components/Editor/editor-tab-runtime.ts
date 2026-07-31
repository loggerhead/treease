import type * as Monaco from 'monaco-editor';

import type { EditorWorkspaceTab } from '../../store/editor-workspace';
import { ensureModelDocumentKey } from './document-key';
import type { EditorModelWithDocumentKey } from './types';

/**
 * Private Monaco resource adapter. Workspace owns topology; this owns only
 * resident models. A caller must install the successor before disposing one.
 */
export class EditorTabRuntime {
  #models = new Map<string, EditorModelWithDocumentKey>();

  constructor(private readonly monaco: typeof Monaco) {}

  get(tabId: string): EditorModelWithDocumentKey | undefined {
    return this.#models.get(tabId);
  }

  getOrCreate(tab: EditorWorkspaceTab): EditorModelWithDocumentKey {
    const existing = this.#models.get(tab.id);
    if (existing) return existing;
    const model = this.monaco.editor.createModel(
      tab.sourceText,
      tab.languageId,
      this.monaco.Uri.parse(`inmemory://model/${tab.id}`),
    ) as EditorModelWithDocumentKey;
    ensureModelDocumentKey(model, tab.documentKey);
    model.__treeaseDocumentKey = tab.documentKey;
    this.#models.set(tab.id, model);
    return model;
  }

  dispose(tabId: string): void {
    const model = this.#models.get(tabId);
    if (!model) return;
    this.#models.delete(tabId);
    model.dispose();
  }

  disposeAll(): void {
    for (const tabId of this.#models.keys()) this.dispose(tabId);
  }
}
