declare module '@core-wasm/index' {
  export const TOKEN_TYPES: any;
  export const TOKEN_TYPE_LAYER: any;
  export const TOKEN_TYPE_THEME_KEY: any;
  export const supportedEditorLanguages: readonly any[];
  export const supportedEditorLanguageIds: readonly any[];
  export const supportedEditorLanguageSet: ReadonlySet<any>;
  export const editorLanguageFallback: any;
  export const importOnlyLanguages: readonly any[];
  export const importFormatOptions: Array<{ id: any; label: string; extensions: string[] }>;
  export const exampleLanguageByExtension: Map<string, any>;
  export function findSupportedLanguageByExtension(...args: any[]): any;
  export function findExampleLanguageByExtension(...args: any[]): any;
  export function initWasm(...args: any[]): any;
  export function getDiagnostics(...args: any[]): any;
  export function analyzeDocument(...args: any[]): any;
  export function getStoredDocumentAnalysis(...args: any[]): any;
  export function buildGraphDelta(...args: any[]): any;
  export function setBuilderConfig(...args: any[]): any;
  export function formatText(...args: any[]): any;
  export function minifyText(...args: any[]): any;
  export function sortText(...args: any[]): any;
  export function convertText(...args: any[]): any;
  export function runYqText(...args: any[]): any;
  export function getTreePath(...args: any[]): any;
  export function getPathSpan(...args: any[]): any;
  export function parseToTree(...args: any[]): any;
  export function compareStructured(...args: any[]): any;
  export function diffStructured(...args: any[]): any;
  export function advanceDocumentJob(...args: any[]): any;

  export function diffText(...args: any[]): any;
  export type FormatOptions = any;
  export type TokenType = any;
  export type SupportedEditorLanguageId = any;
  export type ImportOnlyLanguageId = any;
  export type SupportedLanguageId = any;
}
declare module 'monaco-editor/esm/vs/editor/editor.api' {
  import type * as Monaco from 'monaco-editor';
  export = Monaco;
}

declare module 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices' {
  export const StandaloneServices: any;
}

declare module 'monaco-editor/esm/vs/platform/configuration/common/configuration' {
  export const IConfigurationService: any;
}

declare module 'monaco-editor/esm/vs/editor/contrib/find/browser/findController' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/folding/browser/folding' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/stickyScroll/browser/stickyScrollContribution' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/documentSemanticTokens' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/viewportSemanticTokens' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/editor/contrib/colorPicker/browser/colorPickerContribution' {
  const value: unknown;
  export default value;
}

declare module 'monaco-editor/esm/vs/language/json/monaco.contribution' {
  export const jsonDefaults: any;
}
