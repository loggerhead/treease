import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { TreeNode } from '@core-wasm/index'

export type PreviewContext = {
  node: TreeNode;
  value: string;
  rawValue: string;
  language: SupportedEditorLanguageId;
};

export interface Previewer {
  detector: (context: PreviewContext) => Promise<boolean> | boolean;
  generator: (context: PreviewContext) => Promise<string | string[]> | string | string[];
}
