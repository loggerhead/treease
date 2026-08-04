import {
  TabEditorNavigationFacade,
  type EditorNavigationRuntimePort,
  type EditorNavigationState,
} from './editor-navigation-facade';
import { GraphNavigationFacade, type GraphNavigationRuntimePort } from './graph-navigation-facade';
import { NavigationCoordinator } from './navigation-coordinator';
import { TabNavigatorNavigationFacade, type NavigatorNavigationPort, type NavigatorNavigationState } from './navigator-navigation-facade';
import { TabSearchNavigationFacade, type SearchNavigationState } from './search-navigation-facade';
import type { NavigationDispatchResult, NavigationUserEvent, NavigationTarget } from './navigation-contract';
import { createActiveTabProjection } from './active-tab-projection';
import { createTabNavigationStore, type NavigationEntitySlices, type TabNavigationTabInput } from './tab-navigation-store';
import { TabRuntimeRegistry } from './tab-runtime-registry';

export type WorkspaceNavigationSlices = NavigationEntitySlices & {
  editorState: EditorNavigationState;
  graphState: { readonly version: 0 };
  navigatorState: NavigatorNavigationState;
  searchState: SearchNavigationState;
};

export type WorkspaceNavigationRuntimePorts = Readonly<{
  editor: EditorNavigationRuntimePort;
  graph: GraphNavigationRuntimePort;
  navigator: NavigatorNavigationPort;
  isVisible: (target: NavigationTarget) => boolean;
  completeNavigationEnabled: () => boolean;
}>;

export type WorkspaceNavigationTab = TabNavigationTabInput;

const initialNavigatorState: NavigatorNavigationState = {
  activePath: [], history: [], historyIndex: -1, columnsMaterialized: false, expanded: false,
};

/** The sole workspace composition root; UI adapters provide only entity-local runtime ports. */
export function createWorkspaceNavigationRuntime(
  workspaceId: string,
  tabs: readonly TabNavigationTabInput[],
  activeTabId: string | undefined,
  ports: WorkspaceNavigationRuntimePorts,
) {
  const store = createTabNavigationStore<WorkspaceNavigationSlices>({
    workspaceId,
    initialTabs: tabs,
    activeTabId,
    createEntityState: () => ({
      editorState: { selection: null, lastNavigationSelection: null },
      graphState: { version: 0 },
      navigatorState: initialNavigatorState,
      searchState: { previewId: null },
    }),
  });
  const runtime = new TabRuntimeRegistry(store);
  const coordinator = new NavigationCoordinator({
    targetReader: store,
    getSettings: () => ({ completeNavigationEnabled: ports.completeNavigationEnabled() }),
    facades: {
      editor: new TabEditorNavigationFacade({
        writer: store.entity('editorState'), runtime: ports.editor, targetReader: store,
        isVisible: ports.isVisible, publish: (event) => { void coordinator.dispatch(event); },
      }),
      graph: new GraphNavigationFacade({ runtime: ports.graph, targetReader: store }),
      navigator: new TabNavigatorNavigationFacade({
        writer: store.entity('navigatorState'), targetReader: store, port: ports.navigator,
        readState: (target) => store.getTab(target.tabId)?.state.navigatorState ?? null,
      }),
      search: new TabSearchNavigationFacade({
        writer: store.entity('searchState'), targetReader: store,
        readState: (target) => store.getTab(target.tabId)?.state.searchState ?? null,
      }),
    },
  });
  return {
    store,
    runtime,
    active: createActiveTabProjection(store),
    sync: (nextTabs: readonly WorkspaceNavigationTab[], nextActiveTabId: string | undefined) => {
      const nextById = new Map(nextTabs.map((tab) => [tab.id, tab]));
      for (const tabId of store.getSnapshot().tabOrder) {
        if (!nextById.has(tabId)) store.close(tabId);
      }
      for (const tab of nextTabs) {
        const current = store.getTarget(tab.id);
        if (!current) store.create(tab);
        else if (current.documentKey !== tab.documentKey) {
          store.replaceDocument(tab.id, tab);
        } else if (current.revision !== tab.revision) {
          store.updateRevision(tab.id, tab.revision);
        }
      }
      if (nextActiveTabId) store.activate(nextActiveTabId);
    },
    dispatch: (event: NavigationUserEvent): Promise<NavigationDispatchResult> => coordinator.dispatch(event),
    target: (tabId: string) => store.getTarget(tabId),
  };
}
