type ContributionLoader = () => Promise<unknown>;

const contributionLoaders: readonly ContributionLoader[] = [
  () => import('monaco-editor/esm/vs/editor/contrib/find/browser/findController' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/folding/browser/folding' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/stickyScroll/browser/stickyScrollContribution' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/documentSemanticTokens' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/viewportSemanticTokens' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/colorPicker/browser/colorPickerContribution' as string),
];

export async function loadMonacoContributions(): Promise<void> {
  await Promise.all(contributionLoaders.map((load) => load()));
}
