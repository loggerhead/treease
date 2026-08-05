import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { EditorWorkspaceState } from './editor-workspace';

/**
 * Test-only identity remapping for fixtures that need stable historical ids.
 * Production topology never rebinds a sidecar after pair creation.
 */
export function remapPairedSidecarForFixture(
  workspace: EditorWorkspaceState,
  input: { id: string; name: string; languageId: SupportedEditorLanguageId; sourceText: string },
): EditorWorkspaceState {
  const main = workspace.tabsById[workspace.activeTabId];
  const previousId = main?.sidecarTabId;
  const previous = previousId ? workspace.tabsById[previousId] : null;
  if (!main || !previous || previous.role !== 'sidecar' || previous.ownerMainTabId !== main.id || previous.id === input.id) return workspace;
  const sidecar = {
    ...previous,
    id: input.id,
    name: input.name,
    documentKey: `sidecar:${input.id}:0`,
    languageId: input.languageId,
    sourceText: input.sourceText,
    tempModel: { ...previous.tempModel, scratchText: input.sourceText },
  };
  const tabsById = { ...workspace.tabsById };
  delete tabsById[previous.id];
  tabsById[main.id] = { ...main, sidecarTabId: sidecar.id };
  tabsById[sidecar.id] = sidecar;
  return { ...workspace, tabsById, paneTabIds: { ...workspace.paneTabIds, right: sidecar.id } };
}
