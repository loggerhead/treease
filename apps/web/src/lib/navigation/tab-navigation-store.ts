import { get, writable, type Readable } from 'svelte/store';
import type { NavigationTarget, NavigationTargetReader, NavigationTargetStatus } from './navigation-contract';

export type NavigationEntitySlices = {
  editorState: unknown;
  graphState: unknown;
  navigatorState: unknown;
  searchState: unknown;
};

export type NavigationEntityKey = keyof NavigationEntitySlices;

export type TabNavigationTarget = NavigationTarget;

export type TabNavigationTab<Slices extends NavigationEntitySlices> = TabNavigationTarget & {
  readonly state: Slices;
};

export type TabNavigationState<Slices extends NavigationEntitySlices> = {
  readonly tabsById: Readonly<Record<string, TabNavigationTab<Slices>>>;
  readonly tabOrder: readonly string[];
  readonly activeTabId: string | null;
};

export type TabNavigationTabInput = {
  id: string;
  documentKey: string;
  /** Supplied by the workspace lifecycle; this projection must not infer it. */
  generation: number;
  revision: number;
};

export type TabNavigationDocumentReplacement = Pick<TabNavigationTabInput, 'documentKey' | 'generation' | 'revision'>;

export type TabNavigationWriteResult = { kind: 'applied' } | { kind: 'closed' } | { kind: 'stale' };

export type TabNavigationLifecycleEvent =
  | { kind: 'closed'; target: TabNavigationTarget }
  | { kind: 'replaced'; previous: TabNavigationTarget; next: TabNavigationTarget };

export type TabNavigationStoreOptions<Slices extends NavigationEntitySlices> = {
  workspaceId: string;
  initialTabs: readonly TabNavigationTabInput[];
  activeTabId?: string;
  createEntityState: (target: TabNavigationTarget) => Slices;
};

export type TabEntitySliceWriter<Slices extends NavigationEntitySlices, Key extends keyof Slices> = {
  update(target: TabNavigationTarget, updater: (current: Readonly<Slices[Key]>) => Slices[Key]): TabNavigationWriteResult;
};

export type TabNavigationStore<Slices extends NavigationEntitySlices> = Readable<TabNavigationState<Slices>> & NavigationTargetReader & {
  getSnapshot(): TabNavigationState<Slices>;
  getTab(tabId: string): TabNavigationTab<Slices> | null;
  getTarget(tabId: string): TabNavigationTarget | null;
  getActiveTarget(): TabNavigationTarget | null;
  isCurrent(target: TabNavigationTarget): TabNavigationWriteResult;
  entity<Key extends keyof Slices>(key: Key): TabEntitySliceWriter<Slices, Key>;
  create(input: TabNavigationTabInput): TabNavigationTarget;
  activate(tabId: string): boolean;
  close(tabId: string): boolean;
  replaceDocument(tabId: string, replacement: TabNavigationDocumentReplacement): TabNavigationTarget | null;
  updateRevision(tabId: string, revision: number): TabNavigationTarget | null;
  subscribeLifecycle(run: (event: TabNavigationLifecycleEvent) => void): () => void;
};

function freezeForRead<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const nested of Object.values(value as Record<string, unknown>)) freezeForRead(nested);
  return value;
}

function targetOf<Slices extends NavigationEntitySlices>(tab: TabNavigationTab<Slices>): TabNavigationTarget {
  return {
    workspaceId: tab.workspaceId,
    tabId: tab.tabId,
    documentKey: tab.documentKey,
    generation: tab.generation,
    revision: tab.revision,
  };
}

function createState<Slices extends NavigationEntitySlices>(
  tabs: Record<string, TabNavigationTab<Slices>>,
  tabOrder: string[],
  activeTabId: string | null,
): TabNavigationState<Slices> {
  return freezeForRead({ tabsById: tabs, tabOrder, activeTabId });
}

/**
 * Physical storage for tab-scoped navigation state. Entity facades receive a
 * slice writer rather than this store, so no facade can patch another entity.
 */
export function createTabNavigationStore<Slices extends NavigationEntitySlices>(
  options: TabNavigationStoreOptions<Slices>,
): TabNavigationStore<Slices> {
  const lifecycleSubscribers = new Set<(event: TabNavigationLifecycleEvent) => void>();
  const initialTabs: Record<string, TabNavigationTab<Slices>> = {};
  const initialOrder: string[] = [];

  for (const input of options.initialTabs) {
    if (initialTabs[input.id]) throw new Error(`Duplicate navigation tab id: ${input.id}`);
    const target: TabNavigationTarget = {
      workspaceId: options.workspaceId,
      tabId: input.id,
      documentKey: input.documentKey,
      generation: input.generation,
      revision: input.revision,
    };
    initialTabs[input.id] = freezeForRead({ ...target, state: options.createEntityState(target) });
    initialOrder.push(input.id);
  }

  const requestedActive = options.activeTabId ?? initialOrder[0] ?? null;
  if (requestedActive && !initialTabs[requestedActive]) throw new Error(`Unknown active navigation tab: ${requestedActive}`);
  const state = writable(createState(initialTabs, initialOrder, requestedActive));

  const updateState = (updater: (current: TabNavigationState<Slices>) => TabNavigationState<Slices>) => {
    state.update((current) => updater(current));
  };
  const emitLifecycle = (event: TabNavigationLifecycleEvent) => {
    for (const subscriber of lifecycleSubscribers) subscriber(event);
  };
  const getRaw = () => get(state);
  const isCurrent = (target: TabNavigationTarget): TabNavigationWriteResult => {
    const tab = getRaw().tabsById[target.tabId];
    if (!tab) return { kind: 'closed' };
    return tab.workspaceId === target.workspaceId
      && tab.documentKey === target.documentKey
      && tab.generation === target.generation
      && tab.revision === target.revision
      ? { kind: 'applied' }
      : { kind: 'stale' };
  };

  return {
    subscribe: state.subscribe,
    getSnapshot: getRaw,
    getTab: (tabId) => getRaw().tabsById[tabId] ?? null,
    getTarget: (tabId) => {
      const tab = getRaw().tabsById[tabId];
      return tab ? targetOf(tab) : null;
    },
    getActiveTarget: () => {
      const activeTabId = getRaw().activeTabId;
      const tab = activeTabId ? getRaw().tabsById[activeTabId] : null;
      return tab ? targetOf(tab) : null;
    },
    status: (target): NavigationTargetStatus => {
      const result = isCurrent(target);
      return result.kind === 'applied' ? 'current' : result.kind;
    },
    isCurrent,
    entity: (key) => ({
      update: (target, updater) => {
        const currentResult = isCurrent(target);
        if (currentResult.kind !== 'applied') return currentResult;
        updateState((current) => {
          const tab = current.tabsById[target.tabId];
          if (!tab
            || tab.workspaceId !== target.workspaceId
            || tab.documentKey !== target.documentKey
            || tab.generation !== target.generation
            || tab.revision !== target.revision) return current;
          const nextSlice = updater(tab.state[key]);
          const nextTab = freezeForRead({ ...tab, state: { ...tab.state, [key]: nextSlice } as Slices });
          return createState({ ...current.tabsById, [target.tabId]: nextTab }, [...current.tabOrder], current.activeTabId);
        });
        return isCurrent(target);
      },
    }),
    create: (input) => {
      if (getRaw().tabsById[input.id]) throw new Error(`Navigation tab already exists: ${input.id}`);
      const target: TabNavigationTarget = {
        workspaceId: options.workspaceId,
        tabId: input.id,
        documentKey: input.documentKey,
        generation: input.generation,
        revision: input.revision,
      };
      updateState((current) => createState(
        { ...current.tabsById, [input.id]: freezeForRead({ ...target, state: options.createEntityState(target) }) },
        [...current.tabOrder, input.id],
        current.activeTabId ?? input.id,
      ));
      return target;
    },
    activate: (tabId) => {
      if (!getRaw().tabsById[tabId]) return false;
      updateState((current) => current.activeTabId === tabId ? current : createState(
        { ...current.tabsById }, [...current.tabOrder], tabId,
      ));
      return true;
    },
    close: (tabId) => {
      const tab = getRaw().tabsById[tabId];
      if (!tab) return false;
      const target = targetOf(tab);
      updateState((current) => {
        const { [tabId]: _closed, ...remainingTabs } = current.tabsById;
        const nextOrder = current.tabOrder.filter((id) => id !== tabId);
        const activeTabId = current.activeTabId === tabId ? nextOrder[0] ?? null : current.activeTabId;
        return createState(remainingTabs, nextOrder, activeTabId);
      });
      emitLifecycle({ kind: 'closed', target });
      return true;
    },
    replaceDocument: (tabId, replacement) => {
      const previousTab = getRaw().tabsById[tabId];
      if (!previousTab) return null;
      const previous = targetOf(previousTab);
      const next: TabNavigationTarget = {
        workspaceId: options.workspaceId,
        tabId,
        documentKey: replacement.documentKey,
        generation: replacement.generation,
        revision: replacement.revision,
      };
      updateState((current) => createState(
        { ...current.tabsById, [tabId]: freezeForRead({ ...next, state: options.createEntityState(next) }) },
        [...current.tabOrder], current.activeTabId,
      ));
      emitLifecycle({ kind: 'replaced', previous, next });
      return next;
    },
    updateRevision: (tabId, revision) => {
      const previousTab = getRaw().tabsById[tabId];
      if (!previousTab) return null;
      if (previousTab.revision === revision) return targetOf(previousTab);
      const next = { ...targetOf(previousTab), revision };
      updateState((current) => createState(
        { ...current.tabsById, [tabId]: freezeForRead({ ...next, state: previousTab.state }) },
        [...current.tabOrder], current.activeTabId,
      ));
      return next;
    },
    subscribeLifecycle: (run) => {
      lifecycleSubscribers.add(run);
      return () => lifecycleSubscribers.delete(run);
    },
  };
}
