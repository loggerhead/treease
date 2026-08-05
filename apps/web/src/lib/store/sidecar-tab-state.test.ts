import { describe, expect, it } from 'vitest';

import { initialFullEditUiState } from './full-edit-ui-store';
import { initialTempModel } from './graph-selection-store';
import { activateWorkspaceTab, closeWorkspaceTabTransition, createEditorWorkspaceState, createWorkspaceTabTransition, transitionWorkspaceTabDocument } from './editor-workspace';
import {
  isVisibleSidecarTarget,
  readSidecarTempModel,
  updateSidecarCompareOutcome,
  commitSidecarInput,
  updateSidecarNavigator,
  updateSidecarTempModel,
  updateSidecarViewport,
} from './sidecar-tab-state';
import { targetOfTab } from './tab-target';
import { getWorkspaceState, setWorkspaceState } from './workspace-store';

function workspace() {
  return createEditorWorkspaceState({
    id: 'main', role: 'primary', name: 'Main', documentKey: 'main:0', languageId: 'json', sourceText: '{}',
    revision: 0, graphAppliedRevision: 0, snapshotId: null, tempModel: initialTempModel, fullEditUiState: initialFullEditUiState,
  });
}

describe('sidecar tab-state facade', () => {
  it('writes a background pair outcome to its captured owner and rejects it after replacement', () => {
    const initial = workspace();
    const second = createWorkspaceTabTransition(initial, {
      id: 'second', name: 'Second', documentKey: 'second:0', languageId: 'json', sourceText: '{}',
    })!.workspace;
    const sidecar = second.tabsById[second.tabsById.second.sidecarTabId!];
    const target = targetOfTab(sidecar);
    setWorkspaceState(second);

    expect(updateSidecarCompareOutcome(target, { kind: 'different', mode: 'text' })).toBe(true);
    expect(getWorkspaceState().tabsById[sidecar.id].sidecarState?.compare.outcome).toEqual({ kind: 'different', mode: 'text' });
    const mainSidecarId = getWorkspaceState().tabsById.main.sidecarTabId!;
    expect(getWorkspaceState().tabsById[mainSidecarId].sidecarState?.compare.outcome).toEqual({ kind: 'none' });

    const replaced = transitionWorkspaceTabDocument(getWorkspaceState(), {
      tabId: 'second',
      expected: { documentKey: 'second:0', languageId: 'json', revision: 0 },
      next: { documentKey: 'second:1', languageId: 'json', revision: 1, sourceText: '{}' },
    })!;
    setWorkspaceState(replaced);
    expect(updateSidecarCompareOutcome(target, { kind: 'equal', mode: 'tree' })).toBe(false);
  });

  it('accepts background Graph and Navigator writes for the captured sidecar, but rejects them after close', () => {
    const initial = workspace();
    const withSecond = createWorkspaceTabTransition(initial, {
      id: 'second', name: 'Second', documentKey: 'second:0', languageId: 'json', sourceText: '{}',
    })!.workspace;
    const secondSidecar = withSecond.tabsById[withSecond.tabsById.second.sidecarTabId!];
    const target = targetOfTab(secondSidecar);
    setWorkspaceState(activateWorkspaceTab(withSecond, 'main'));
    expect(isVisibleSidecarTarget(target)).toBe(false);

    expect(updateSidecarViewport(target, { x: 10, y: 20, scaleX: 1.25, scaleY: 1.25 })).toBe(true);
    const itemsPath = [{ key: 'items' }] as any;
    expect(updateSidecarNavigator(target, {
      activePath: itemsPath, history: [itemsPath], historyIndex: 0,
      collapsed: false, expanded: true, columnsMaterialized: true,
    })).toBe(true);
    expect(updateSidecarTempModel(target, (current) => ({ ...current, treePath: itemsPath }))).toBe(true);
    expect(getWorkspaceState().tabsById[secondSidecar.id].sidecarState?.graph.viewport).toMatchObject({ x: 10, y: 20 });
    expect(getWorkspaceState().tabsById[secondSidecar.id].sidecarState?.navigator.activePath).toEqual(itemsPath);
    expect(readSidecarTempModel(target)?.treePath).toEqual(itemsPath);

    const closed = closeWorkspaceTabTransition(getWorkspaceState(), 'second', {
      id: 'blank', documentKey: 'blank:0', name: 'Untitled', languageId: 'json',
    })!;
    setWorkspaceState(closed.workspace);
    expect(updateSidecarViewport(target, { x: 0, y: 0, scaleX: 1, scaleY: 1 })).toBe(false);
    expect(updateSidecarNavigator(target, {
      activePath: [], history: [], historyIndex: -1, collapsed: false, expanded: false, columnsMaterialized: false,
    })).toBe(false);
    expect(updateSidecarTempModel(target, (current) => current)).toBe(false);
  });

  it('commits content only to the captured sidecar and never changes snapshot bindings', () => {
    const initial = workspace();
    const sidecarId = initial.tabsById.main.sidecarTabId!;
    const sidecar = initial.tabsById[sidecarId];
    const target = targetOfTab(sidecar);
    const withUnrelatedSnapshot = {
      ...initial,
      snapshotBindingsByDocumentKey: {
        ...initial.snapshotBindingsByDocumentKey,
        'main:0': { documentKey: 'main:0', snapshotId: 'main-snapshot' as any, revision: 0 },
      },
    };
    setWorkspaceState(withUnrelatedSnapshot);

    const committed = commitSidecarInput(target, { languageId: 'json', sourceText: '{\n  "right": true\n}' });
    expect(committed).toEqual({ ...target, revision: target.revision + 1 });
    expect(getWorkspaceState().tabsById.main.sourceText).toBe('{}');
    expect(getWorkspaceState().tabsById[sidecarId]).toMatchObject({
      sourceText: '{\n  "right": true\n}',
      revision: target.revision + 1,
      tempModel: { scratchText: '{\n  "right": true\n}' },
    });
    expect(getWorkspaceState().snapshotBindingsByDocumentKey['main:0']).toEqual({ documentKey: 'main:0', snapshotId: 'main-snapshot', revision: 0 });
    expect(commitSidecarInput(target, { languageId: 'json', sourceText: '{}' })).toBeNull();
  });
});
