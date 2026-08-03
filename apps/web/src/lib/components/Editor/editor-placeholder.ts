import type * as Monaco from 'monaco-editor';

import { getLanguageExample } from '../../monaco/language-examples';
import {
  importFormatOptions,
  type SupportedEditorLanguageId,
} from '../../monaco/language-support';

type EditorPlaceholderControllerOptions = {
  getEditor: () => Monaco.editor.IStandaloneCodeEditor | null;
  getModel: () => Monaco.editor.ITextModel | null;
  getMonaco: () => typeof import('monaco-editor') | undefined;
  getLanguage: () => SupportedEditorLanguageId;
  getTitle?: () => string;
  onRequestImportFile: (payload: {
    sourceFormat: string;
    targetFormat: string;
    accept: string[];
  }) => void | Promise<void>;
  onLoadExample: (example: string, language: SupportedEditorLanguageId) => void | Promise<void>;
};

export function createEditorPlaceholderController(options: EditorPlaceholderControllerOptions) {
  let widget: Monaco.editor.IContentWidget | null = null;

  function remove(): void {
    const editor = options.getEditor();
    if (editor && widget) editor.removeContentWidget(widget);
    widget = null;
  }

  function update(): void {
    const editor = options.getEditor();
    const model = options.getModel();
    const monaco = options.getMonaco();
    if (!editor || !model || !monaco) return;

    if (model.getValue().trim() !== '') {
      remove();
      return;
    }

    if (widget) return;

    widget = {
      getId: () => 'treease-editor-placeholder',
      getDomNode: () => {
        const root = document.createElement('div');
        root.className = 'treease-editor-placeholder';

        const title = document.createElement('div');
        title.textContent = options.getTitle?.() ?? 'Start typing, or open a file';
        root.appendChild(title);


        const openFileRow = document.createElement('div');
        openFileRow.className = 'treease-editor-placeholder__row';
        const openFile = document.createElement('button');
        openFile.type = 'button';
        openFile.className = 'treease-editor-placeholder__link';
        openFile.textContent = 'Choose a file or drag one into this editor';
        openFile.addEventListener('click', (event) => {
          event.preventDefault();
          event.stopPropagation();
          const language = options.getLanguage();
          void options.onRequestImportFile({
            sourceFormat: language,
            targetFormat: language,
            accept: importFormatOptions.find((option) => option.id === language)?.extensions ?? [],
          });
        });
        openFileRow.appendChild(openFile);
        root.appendChild(openFileRow);

        const exampleRow = document.createElement('div');
        exampleRow.className = 'treease-editor-placeholder__row';
        const loadExample = document.createElement('button');
        loadExample.type = 'button';
        loadExample.className = 'treease-editor-placeholder__link';
        loadExample.textContent = 'Load an example file';
        loadExample.addEventListener('click', (event) => {
          event.preventDefault();
          event.stopPropagation();
          const language = options.getLanguage();
          const example = getLanguageExample(language);
          if (example) void options.onLoadExample(example, language);
        });
        exampleRow.appendChild(loadExample);
        root.appendChild(exampleRow);

        return root;
      },
      getPosition: () => ({
        position: { lineNumber: 1, column: 1 },
        preference: [monaco.editor.ContentWidgetPositionPreference.EXACT],
      }),
      suppressMouseDown: true,
    };
    const currentWidget = widget;
    editor.addContentWidget(currentWidget);
    requestAnimationFrame(() => {
      if (
        widget !== currentWidget ||
        options.getEditor() !== editor ||
        options.getModel()?.getValue().trim() !== ''
      ) {
        return;
      }
      // A sidecar can be created before its split pane has a layout. Reattach
      // once after the first frame so Monaco mounts the widget into the real view.
      editor.removeContentWidget(currentWidget);
      editor.addContentWidget(currentWidget);
    });
  }

  function refresh(): void {
    if (widget) remove();
    update();
  }

  return {
    update,
    refresh,
    dispose: remove,
  };
}
