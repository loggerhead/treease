type ContributionLoader = () => Promise<unknown>;

const contributionLoaders: readonly ContributionLoader[] = [
  () => import('monaco-editor/esm/vs/editor/contrib/find/browser/findController.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/folding/browser/folding.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/stickyScroll/browser/stickyScrollContribution.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/documentSemanticTokens.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/semanticTokens/browser/viewportSemanticTokens.js' as string),
  () => import('monaco-editor/esm/vs/editor/contrib/colorPicker/browser/colorPickerContribution.js' as string),
];

export async function loadMonacoContributions(): Promise<void> {
  await Promise.all(contributionLoaders.map((load) => load()));
}
