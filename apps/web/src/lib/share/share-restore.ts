import type { EditorSelection, ShareResource, ViewportAnchor } from './share-resource';

type EditorPort = {
  ensureReady(): Promise<void>;
  replaceActiveFromFile(payload: { text: string; languageId: ShareResource['payload']['left']['languageId'] }): Promise<void>;
  waitForIdle(): Promise<void>;
  restoreViewportAnchor(anchor: ViewportAnchor): void;
  restoreSelection(selection: EditorSelection): void;
};
type ViewerPort = {
  showTextPreview(text: string, languageId: ShareResource['payload']['left']['languageId']): Promise<void>;
  runCompare(): Promise<void>;
  restoreViewportAnchor(anchor: ViewportAnchor): void;
};

function finalViewportAction(resource: Extract<ShareResource, { type: 'compare' }>): { left: ViewportAnchor; right: ViewportAnchor } | null {
  for (let index = resource.payload.actions.length - 1; index >= 0; index -= 1) {
    const action = resource.payload.actions[index];
    if (action.type === 'viewport_changed') return action.payload;
  }
  return null;
}

export async function restoreShareResource(resource: ShareResource, ports: {
  editor: EditorPort;
  viewer: ViewerPort;
  setViewMode(mode: 'graph' | 'text'): void;
  clearCompareState(): void;
  restoreTreePath(path: ShareResource['payload']['interaction']['treePath']): boolean;
  restoreGraphFocus(path: ShareResource['payload']['interaction']['treePath'], target: 'key' | 'value' | 'node'): Promise<boolean>;
  waitForGraphReady(): Promise<boolean>;
  restoreColumnNavigator(activePath: ShareResource['payload']['interaction']['columnNavigator']['activePath']): Promise<boolean>;
  reportNavigationWarning(): void;
}): Promise<void> {
  await ports.editor.ensureReady();
  await ports.editor.replaceActiveFromFile({ text: resource.payload.left.text, languageId: resource.payload.left.languageId });
  await ports.editor.waitForIdle();
  if (resource.type === 'text_snapshot') {
    ports.clearCompareState();
    if (resource.payload.right) await ports.viewer.showTextPreview(resource.payload.right.text, resource.payload.right.languageId);
    ports.setViewMode(resource.payload.layout.viewMode);
    await restoreInteraction(resource, ports);
    return;
  }
  ports.setViewMode('text');
  await ports.viewer.showTextPreview(resource.payload.right.text, resource.payload.right.languageId);
  await ports.viewer.runCompare();
  const viewport = finalViewportAction(resource);
  if (viewport) {
    ports.editor.restoreViewportAnchor(viewport.left);
    ports.viewer.restoreViewportAnchor(viewport.right);
  }
  await restoreInteraction(resource, ports);
}

async function restoreInteraction(resource: ShareResource, ports: {
  editor: EditorPort;
  restoreTreePath(path: ShareResource['payload']['interaction']['treePath']): boolean;
  restoreGraphFocus(path: ShareResource['payload']['interaction']['treePath'], target: 'key' | 'value' | 'node'): Promise<boolean>;
  waitForGraphReady(): Promise<boolean>;
  restoreColumnNavigator(activePath: ShareResource['payload']['interaction']['columnNavigator']['activePath']): Promise<boolean>;
  reportNavigationWarning(): void;
}): Promise<void> {
  const interaction = resource.payload.interaction;
  const columnNavigatorRestored = await ports.restoreColumnNavigator(interaction.columnNavigator.activePath);
  const treeRestored = ports.restoreTreePath(interaction.treePath);
  let focusRestored = true;
  if (interaction.focus?.type === 'editor') ports.editor.restoreSelection(interaction.focus.selection);
  else if (interaction.focus?.type === 'graph') {
    ports.editor.restoreSelection(interaction.focus.editorSelection);
    focusRestored = await ports.waitForGraphReady() && await ports.restoreGraphFocus(interaction.focus.path, interaction.focus.target);
  }
  if (!columnNavigatorRestored || !treeRestored || !focusRestored) ports.reportNavigationWarning();
}
