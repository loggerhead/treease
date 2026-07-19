// Responsibility: register preview capabilities on window in DEV/test mode.
import { generatePreview } from '../preview';
import { supportedEditorLanguageSet, type SupportedEditorLanguageId } from '../monaco/language-support';
import { registerTreeasePreviewBridge } from './window-treease';
import { valueToTreeNode } from '../../shared/tree-node-value';

function normalizePreviewLanguage(language?: SupportedEditorLanguageId | string): SupportedEditorLanguageId {
  if (language && supportedEditorLanguageSet.has(language as SupportedEditorLanguageId)) {
    return language as SupportedEditorLanguageId;
  }
  return 'json';
}

export function installPreviewBridge(): void {
  registerTreeasePreviewBridge({
    generate: async ({ value, rawValue, language }) =>
      generatePreview({
        node: valueToTreeNode(value),
        value,
        rawValue: rawValue ?? JSON.stringify(value),
        language: normalizePreviewLanguage(language),
      }),
  });
}
