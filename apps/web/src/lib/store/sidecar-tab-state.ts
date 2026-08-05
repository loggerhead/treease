import type { PathSeg } from './tree-path';
import type { CompareOutcomeState, SidecarPaneState } from './editor-workspace';
import type { TempModel } from './editor-store-types';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { getWorkspaceState, updateWorkspaceTab } from './workspace-store';
import { isVisibleTabTarget, tabTargetStatus, targetOfTab, type TabTarget } from './tab-target';

type NavigatorUpdate = Pick<SidecarPaneState['navigator'], 'activePath' | 'history' | 'historyIndex' | 'collapsed' | 'expanded' | 'columnsMaterialized'>;
type Viewport = SidecarPaneState['graph']['viewport'];
type ScrollPosition = Pick<SidecarPaneState['compare'], 'scrollTop' | 'scrollLeft'>;

function pairedSidecar(target: TabTarget) {
  const workspace = getWorkspaceState();
  const sidecar = workspace.tabsById[target.tabId];
  if (!sidecar || sidecar.role !== 'sidecar' || tabTargetStatus(workspace, target) !== 'current') return null;
  const owner = sidecar.ownerMainTabId ? workspace.tabsById[sidecar.ownerMainTabId] : null;
  return owner?.sidecarTabId === sidecar.id && sidecar.sidecarState ? { workspace, sidecar } : null;
}

/** Captures the right-side entity currently projected by the active main tab. */
export function captureActiveSidecarTarget(): TabTarget | null {
  const workspace = getWorkspaceState();
  const main = workspace.tabsById[workspace.activeTabId];
  return main?.sidecarTabId ? captureSidecarTarget(main.sidecarTabId) : null;
}

/** Captures one explicit sidecar entity without resolving it through activeTabId. */
export function captureSidecarTarget(tabId: string): TabTarget | null {
  const workspace = getWorkspaceState();
  const sidecar = workspace.tabsById[tabId];
  const owner = sidecar?.ownerMainTabId ? workspace.tabsById[sidecar.ownerMainTabId] : null;
  return sidecar?.role === 'sidecar' && owner?.sidecarTabId === sidecar.id ? targetOfTab(sidecar) : null;
}

export function isVisibleSidecarTarget(target: TabTarget): boolean {
  return isVisibleTabTarget(getWorkspaceState(), target);
}

export function updateSidecarNavigator(target: TabTarget, navigator: NavigatorUpdate): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { sidecarState: { ...current.sidecar.sidecarState!, navigator } });
  return true;
}

export function updateSidecarNavigatorProjection(
  target: TabTarget,
  navigator: NavigatorUpdate,
  treePath: PathSeg[],
): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, {
    sidecarState: { ...current.sidecar.sidecarState!, navigator },
    tempModel: { ...current.sidecar.tempModel, treePath },
  });
  return true;
}

export function updateSidecarViewport(target: TabTarget, viewport: Viewport): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { sidecarState: { ...current.sidecar.sidecarState!, graph: { viewport } } });
  return true;
}

export function updateSidecarCompareScroll(target: TabTarget, compare: ScrollPosition): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, {
    sidecarState: { ...current.sidecar.sidecarState!, compare: { ...current.sidecar.sidecarState!.compare, ...compare } },
  });
  return true;
}

export function updateSidecarSurfaceMode(target: TabTarget, surfaceMode: SidecarPaneState['surfaceMode']): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { sidecarState: { ...current.sidecar.sidecarState!, surfaceMode } });
  return true;
}

/** Compare input is right-pane local state; it never replaces the main document authority. */
export function updateSidecarCompareText(target: TabTarget, sourceText: string): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { sourceText });
  return true;
}

export function updateSidecarCompareLanguage(target: TabTarget, languageId: SupportedEditorLanguageId): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { languageId });
  return true;
}

export function commitSidecarCompareEdit(
  target: TabTarget,
  input: { languageId: SupportedEditorLanguageId; sourceText: string },
): number | null {
  const current = pairedSidecar(target);
  if (!current) return null;
  const revision = current.sidecar.revision + 1;
  updateWorkspaceTab(current.sidecar.id, {
    languageId: input.languageId,
    sourceText: input.sourceText,
    revision,
    tempModel: { ...current.sidecar.tempModel, scratchText: input.sourceText },
  });
  return revision;
}

/**
 * The only persistence entrance for a paired-sidecar content transaction.
 * It intentionally does not create, clear, or bind a DocumentSnapshot.
 */
export function commitSidecarInput(
  target: TabTarget,
  input: { languageId: SupportedEditorLanguageId; sourceText: string },
): TabTarget | null {
  const current = pairedSidecar(target);
  if (!current) return null;
  const revision = current.sidecar.revision + 1;
  updateWorkspaceTab(current.sidecar.id, {
    languageId: input.languageId,
    sourceText: input.sourceText,
    revision,
    tempModel: { ...current.sidecar.tempModel, scratchText: input.sourceText },
  });
  return { ...target, revision };
}

export function updateSidecarCompareOutcome(target: TabTarget, outcome: CompareOutcomeState): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, {
    sidecarState: { ...current.sidecar.sidecarState!, compare: { ...current.sidecar.sidecarState!.compare, outcome } },
  });
  return true;
}

export function updateSidecarTreePath(target: TabTarget, path: PathSeg[]): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { tempModel: { ...current.sidecar.tempModel, treePath: path } });
  return true;
}

/** Graph runtime writes are target-bound; callers cannot redirect them by reading activeTabId later. */
export function updateSidecarTempModel(target: TabTarget, updater: (current: Readonly<TempModel>) => TempModel): boolean {
  const current = pairedSidecar(target);
  if (!current) return false;
  updateWorkspaceTab(current.sidecar.id, { tempModel: updater(current.sidecar.tempModel) });
  return true;
}

/** Returns a defensive snapshot of one sidecar's graph-owned state. */
export function readSidecarTempModel(target: TabTarget): TempModel | null {
  const current = pairedSidecar(target);
  if (!current) return null;
  const value = current.sidecar.tempModel;
  return {
    ...value,
    treePath: value.treePath.map((segment) => ({ ...segment })),
    graphHighlight: value.graphHighlight
      ? { ...value.graphHighlight, path: value.graphHighlight.path.map((segment) => ({ ...segment })) }
      : null,
    diagnostics: value.diagnostics.map((diagnostic) => ({
      ...diagnostic,
      context: diagnostic.context.map((line) => ({ ...line })),
    })),
  };
}
