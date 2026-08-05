import type { SnapshotId } from '@core-wasm/index';
import type { ValueType } from '@treease/graph-viewer-runtime';
import type { SupportedEditorLanguageId } from '../../../monaco/language-support';
import { queryDirectChildren, queryPathValue } from '../../../services/SnapshotProjectionService';
import { buildPathKey } from '../../../graph/graph-viewer-path';
import { createViewRuntimeOperation, type ViewRuntimeOperation } from '../../../guards/view-runtime-operation';
import { getClampedPaneSize } from '../../ui/split-layout';
import { isPathSegIndex, type PathSeg } from '../../../store/tree-path';
import type { StructuredValueEditIntent } from '../graph-value-edit';
import {
  buildColumnNavigatorDirectItems,
  formatColumnNavigatorPath,
  shouldOpenColumnNavigatorContent,
} from '../column-navigator-graph';
import type {
  ColumnNavigatorColumnItem,
  ColumnNavigatorContentState,
  ColumnNavigatorNavigationState,
  ColumnNavigatorPaneState,
  ColumnNavigatorState,
  VisibleColumnNavigatorPaneState,
} from './types';

const COLUMN_NAVIGATOR_MIN_HEIGHT = 100;
const COLUMN_NAVIGATOR_MAX_HEIGHT_FRACTION = 0.75;
const ROOT_PATH_KEY = '$';

export type ColumnNavigatorProjectionInput = {
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  revision: number;
  graphAppliedRevision: number;
  snapshotId: SnapshotId | null;
  enableNest: boolean;
};

export type ColumnNavigatorControllerDeps = {
  defaultHeightPx: number;
  getWorkspaceSnapshotId: () => SnapshotId | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getRevision: () => number;
  getEnableNest: () => boolean;
  getReadonly: () => boolean;
  getShellHeight: () => number;
  clearSearchHighlight: () => void;
  clearActiveGraphSelection: () => void;
  publishNavigation: (path: PathSeg[], target: 'key' | 'value' | 'node', trigger: 'breadcrumb') => void;
  publishHistoryTraversal: (direction: -1 | 1) => void;
  publishExpanded: (expanded: boolean) => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
  applyStructuredValueEdit: (intent: StructuredValueEditIntent) => Promise<boolean>;
  waitForCommittedDocument: (documentKey: string, afterRevision: number) => Promise<boolean>;
  markSubgraphRequested: (payload: { requestId: number; pathKey: string; sourceRevision: number }) => void;
  markSubgraphMaterialized: (payload: {
    requestId: number;
    pathKey: string;
    sourceRevision: number;
    materializedRevision: number;
  }) => void;
  onState: (state: ColumnNavigatorState) => void;
  onPaneReady?: (pane: ColumnNavigatorPaneState) => void;
};

type PreparedWorkspace = {
  activePath: PathSeg[];
  chain: ColumnNavigatorPaneState[];
};

class SnapshotNotReadyError extends Error {
  constructor() {
    super('Snapshot is not ready for Column Navigator reads.');
  }
}

function clonePath(path: PathSeg[]): PathSeg[] {
  return path.map((segment) => ({ ...segment }));
}

function clonePane(pane: ColumnNavigatorPaneState): ColumnNavigatorPaneState {
  return {
    ...pane,
    path: clonePath(pane.path),
    items: pane.items.map((item) => ({ ...item, path: clonePath(item.path) })),
    content: pane.content ? { ...pane.content } : null,
  };
}

export function workspacePathKey(path: PathSeg[]): string {
  return buildPathKey(path) || ROOT_PATH_KEY;
}

export function buildWorkspacePathPrefixes(path: PathSeg[]): PathSeg[][] {
  return Array.from({ length: path.length + 1 }, (_, index) => clonePath(path.slice(0, index)));
}

function samePath(left: PathSeg[], right: PathSeg[]): boolean {
  return workspacePathKey(left) === workspacePathKey(right);
}

export function createColumnNavigatorController(deps: ColumnNavigatorControllerDeps) {
  const pendingEditTaskMap = new Map<string, Promise<void>>();
  const queuedEditMap = new Map<string, string>();
  let collapsed = true;
  let keepCollapsed = false;
  let isLoading = false;
  let activePath: PathSeg[] = [];
  let chain: ColumnNavigatorPaneState[] = [];
  let canGoBack = false;
  let canGoForward = false;
  let heightPx = deps.defaultHeightPx;
  let isDraggingDivider = false;
  let resizeState: { startClientY: number; startHeightPx: number } | null = null;
  let projectionSignature = '';
  let requestId = 0;
  let projectionEpoch = 0;
  let disposed = false;
  let navigationOperation: ViewRuntimeOperation | null = null;
  let refreshOperation: ViewRuntimeOperation | null = null;
  const pathValueCache = new Map<
    string,
    { signature: string; promise: Promise<{ content: ColumnNavigatorContentState; isContent: boolean } | null> }
  >();
  const directChildrenCache = new Map<
    string,
    { signature: string; promise: Promise<ColumnNavigatorColumnItem[]> }
  >();

  function visiblePanes(): VisibleColumnNavigatorPaneState[] {
    return chain.map((pane, absoluteIndex) => ({
      ...clonePane(pane),
      visibleIndex: absoluteIndex,
      absoluteIndex,
    }));
  }

  function emitState(): void {
    const panes = visiblePanes();
    deps.onState({
      collapsed,
      isLoading,
      activePath: clonePath(activePath),
      chain: chain.map(clonePane),
      visiblePanes: panes,
      canGoBack,
      canGoForward,
      heightPx,
      isDraggingDivider,
    });
    for (const pane of panes) deps.onPaneReady?.(pane);
  }

  function createWorkspaceOperation(): ViewRuntimeOperation {
    const context = () => ({
      documentKey: deps.getDocumentKey(),
      languageId: deps.getLanguageId(),
      revision: deps.getRevision(),
      sessionId: `${projectionEpoch}|${deps.getWorkspaceSnapshotId() ?? 'no-snapshot'}|${deps.getEnableNest() ? 'nest' : 'flat'}`,
    });
    return createViewRuntimeOperation({ captured: context(), getCurrent: context });
  }

  function loadingPane(path: PathSeg[], nextRequestId: number): ColumnNavigatorPaneState {
    return {
      requestId: nextRequestId,
      path: clonePath(path),
      pathKey: workspacePathKey(path),
      title: formatColumnNavigatorPath(path),
      kind: 'column',
      items: [],
      content: null,
      status: 'loading',
    };
  }

  async function readPath(path: PathSeg[]): Promise<{
    content: ColumnNavigatorContentState;
    isContent: boolean;
  } | null> {
    const snapshotId = deps.getWorkspaceSnapshotId();
    const pathKey = workspacePathKey(path);
    const signature = `${deps.getDocumentKey()}|${snapshotId ?? 'no-snapshot'}`;
    const cached = pathValueCache.get(pathKey);
    if (cached?.signature === signature) return cached.promise;

    const promise = queryPathValue({ documentKey: deps.getDocumentKey(), snapshotId, path }).then((pathValue) => {
      if (pathValue.status === 'snapshotNotReady') throw new SnapshotNotReadyError();
      if (!pathValue.data) return null;
      const content: ColumnNavigatorContentState = {
        tabId: `column-navigator-content:${pathKey}`,
        tabName: formatColumnNavigatorPath(path),
        sourceText: pathValue.data.sourceText || pathValue.data.displayText,
        valueType: pathValue.data.valueType as ValueType,
        semanticTokens: new Uint32Array(pathValue.data.semanticTokens.data).buffer,
        snapshotId,
      };
      return { content, isContent: shouldOpenColumnNavigatorContent(pathValue.data) };
    }).catch((error) => {
      if (pathValueCache.get(pathKey)?.promise === promise) pathValueCache.delete(pathKey);
      throw error;
    });
    pathValueCache.set(pathKey, { signature, promise });
    return promise;
  }

  function readDirectChildren(path: PathSeg[]): Promise<ColumnNavigatorColumnItem[]> {
    const snapshotId = deps.getWorkspaceSnapshotId();
    const pathKey = workspacePathKey(path);
    const signature = `${deps.getDocumentKey()}|${snapshotId ?? 'no-snapshot'}`;
    const cached = directChildrenCache.get(pathKey);
    if (cached?.signature === signature) return cached.promise;

    const promise = queryDirectChildren({ documentKey: deps.getDocumentKey(), snapshotId, path })
      .then((result) => {
        if (result.status === 'snapshotNotReady') throw new SnapshotNotReadyError();
        return buildColumnNavigatorDirectItems(path, result.data);
      })
      .catch((error) => {
        if (directChildrenCache.get(pathKey)?.promise === promise) directChildrenCache.delete(pathKey);
        throw error;
      });
    directChildrenCache.set(pathKey, { signature, promise });
    return promise;
  }

  async function prepareWorkspace(path: PathSeg[], nextRequestId: number): Promise<PreparedWorkspace> {
    const nextChain: ColumnNavigatorPaneState[] = [];
    const prefixes = buildWorkspacePathPrefixes(path);
    const reads = await Promise.all(prefixes.map((prefix) => readPath(prefix)));
    const firstContentIndex = reads.findIndex((read) => read?.isContent === true);
    const lastColumnIndex = firstContentIndex === -1 ? prefixes.length - 1 : firstContentIndex - 1;
    const directChildren = await Promise.all(
      prefixes.map((prefix, index) => {
        if (index > lastColumnIndex || !reads[index]) return Promise.resolve({ items: [] as ColumnNavigatorColumnItem[] });
        return readDirectChildren(prefix)
          .then((items) => ({ items }))
          .catch((error) => ({ error }));
      }),
    );
    for (let index = 0; index < prefixes.length; index += 1) {
      const prefix = prefixes[index]!;
      const read = reads[index];
      if (!read) return { activePath: prefix.slice(0, -1), chain: nextChain };
      if (read.isContent) {
        if (samePath(prefix, path)) {
          nextChain.push({
            requestId: nextRequestId,
            path: clonePath(prefix),
            pathKey: workspacePathKey(prefix),
            title: formatColumnNavigatorPath(prefix),
            kind: 'content',
            items: [],
            content: read.content,
            status: 'ready',
          });
        }
        return { activePath: clonePath(prefix), chain: nextChain };
      }
      try {
        const result = directChildren[index]!;
        if ('error' in result) throw result.error;
        const items = result.items;
        if (!items.length && samePath(prefix, path)) {
          nextChain.push({
            requestId: nextRequestId,
            path: clonePath(prefix),
            pathKey: workspacePathKey(prefix),
            title: formatColumnNavigatorPath(prefix),
            kind: 'content',
            items: [],
            content: read.content,
            status: 'ready',
          });
          return { activePath: clonePath(prefix), chain: nextChain };
        }
        nextChain.push({
          requestId: nextRequestId,
          path: clonePath(prefix),
          pathKey: workspacePathKey(prefix),
          title: formatColumnNavigatorPath(prefix),
          kind: 'column',
          items,
          content: null,
          status: 'ready',
        });
      } catch (error) {
        // Snapshot registration can briefly lag behind the graph milestone.
        // Let the navigation operation retain its loading state so the next
        // projection refresh retries without reporting an application error.
        if (error instanceof SnapshotNotReadyError) throw error;
        deps.handleError(error, {
          component: 'GraphViewer',
          operation: 'buildColumnNavigatorColumn',
          metadata: {
            documentKey: deps.getDocumentKey(),
            language: deps.getLanguageId(),
            pathKey: workspacePathKey(prefix),
          },
        });
        nextChain.push({
          requestId: nextRequestId,
          path: clonePath(prefix),
          pathKey: workspacePathKey(prefix),
          title: formatColumnNavigatorPath(prefix),
          kind: 'column',
          items: [],
          content: null,
          status: 'error',
          error: error instanceof Error ? error.message : String(error),
        });
        return { activePath: clonePath(prefix), chain: nextChain };
      }
    }
    const selectedRead = reads.at(-1);
    const selectedPane = nextChain.at(-1);
    if (selectedRead?.content && selectedRead.isContent === false && selectedPane?.pathKey === workspacePathKey(path)) {
      nextChain.push({
        requestId: nextRequestId,
        path: clonePath(path),
        pathKey: workspacePathKey(path),
        title: formatColumnNavigatorPath(path),
        kind: 'content',
        items: [],
        content: selectedRead.content,
        status: 'ready',
      });
    }
    return { activePath: clonePath(path), chain: nextChain };
  }

  function hasStableSiblingPaneStructure(item: ColumnNavigatorColumnItem): boolean {
    const currentIndex = activePath.at(-1);
    const nextIndex = item.path.at(-1);
    const parentPath = activePath.slice(0, -1);
    const currentContent = chain.at(-1);
    const parentColumn = chain.at(-2);
    return Boolean(
      !item.isContainer &&
      currentIndex &&
      nextIndex &&
      isPathSegIndex(currentIndex) &&
      isPathSegIndex(nextIndex) &&
      samePath(parentPath, item.path.slice(0, -1)) &&
      currentContent?.kind === 'content' &&
      samePath(currentContent.path, activePath) &&
      parentColumn?.kind === 'column' &&
      samePath(parentColumn.path, parentPath),
    );
  }

  async function selectSibling(path: PathSeg[]): Promise<void> {
    void navigationOperation?.cancel();
    void refreshOperation?.cancel();
    const operation = createWorkspaceOperation();
    navigationOperation = operation;
    const pendingEditTask = pendingEditTaskMap.get(workspacePathKey(activePath));
    if (pendingEditTask) {
      await pendingEditTask;
      if (!operation.isCurrent()) return;
    }
    activePath = clonePath(path);
    const nextRequestId = ++requestId;
    deps.markSubgraphRequested({
      requestId: nextRequestId,
      pathKey: workspacePathKey(path),
      sourceRevision: deps.getRevision(),
    });
    emitState();
    const result = await operation.run({
      execute: ({ step }) => step(async () => {
        const read = await readPath(path);
        if (!read?.isContent) throw new Error('Scalar sibling selection must resolve to navigator content.');
        return read.content;
      }),
      land: (content) => {
        if (disposed) return;
        const terminalPane = chain.at(-1);
        if (!terminalPane || terminalPane.kind !== 'content') return;
        chain[chain.length - 1] = {
          ...terminalPane,
          requestId: nextRequestId,
          path: clonePath(path),
          pathKey: workspacePathKey(path),
          title: formatColumnNavigatorPath(path),
          content,
        };
        deps.markSubgraphMaterialized({
          requestId: nextRequestId,
          pathKey: workspacePathKey(path),
          sourceRevision: deps.getRevision(),
          materializedRevision: deps.getRevision(),
        });
        emitState();
        deps.publishNavigation(path, 'value', 'breadcrumb');
      },
    });
    if (result.status === 'failed') {
      deps.handleError(result.error, {
        component: 'GraphViewer',
        operation: 'selectColumnNavigatorSibling',
        metadata: { pathKey: workspacePathKey(path) },
      });
    }
  }

  async function navigate(
    path: PathSeg[],
    options: { reveal: 'immediate' | 'debounced' | 'none' },
  ): Promise<void> {
    if (disposed) return;
    // Selection rebinding waits for the current Monaco draft transaction to
    // become terminal, so a late commit cannot land on a newly selected path.
    void navigationOperation?.cancel();
    void refreshOperation?.cancel();
    const operation = createWorkspaceOperation();
    navigationOperation = operation;
    const pendingEditTask = pendingEditTaskMap.get(workspacePathKey(activePath));
    if (pendingEditTask) {
      await pendingEditTask;
      if (!operation.isCurrent()) return;
    }
    const nextRequestId = ++requestId;
    const requestedPathKey = workspacePathKey(path);
    isLoading = true;
    activePath = clonePath(path);
    // A navigation operation owns the wait for the current Monaco draft.
    // Resetting the workspace invalidates it before this state can become visible.
    // Keep a readable chain until its replacement is ready.
    if (!chain.length) chain = [loadingPane(path, nextRequestId)];
    deps.markSubgraphRequested({
      requestId: nextRequestId,
      pathKey: requestedPathKey,
      sourceRevision: deps.getRevision(),
    });
    emitState();
    const result = await operation.run({
      execute: ({ step }) => step(() => prepareWorkspace(path, nextRequestId)),
      land: (prepared) => {
        if (disposed) return;
        activePath = clonePath(prepared.activePath);
        chain = prepared.chain;
        isLoading = false;
        deps.markSubgraphMaterialized({
          requestId: nextRequestId,
          pathKey: requestedPathKey,
          sourceRevision: deps.getRevision(),
          materializedRevision: deps.getRevision(),
        });
        emitState();
        if (options.reveal === 'immediate' && activePath.length) deps.publishNavigation(activePath, 'value', 'breadcrumb');
      },
    });
    if (result.status === 'failed' && result.error instanceof SnapshotNotReadyError) {
      // Retain the last materialized panes in their loading state. A later
      // snapshot refresh owns the retry; not-ready is never an empty column.
      return;
    }
    if (result.status === 'failed' && navigationOperation === operation) {
      isLoading = false;
      emitState();
    }
  }

  async function openPath(path: PathSeg[], _parentAbsoluteIndex = -1): Promise<void> {
    if (!keepCollapsed) collapsed = false;
    await navigate(path, { reveal: 'none' });
  }

  async function selectPath(path: PathSeg[]): Promise<void> {
    await navigate(path, { reveal: 'immediate' });
  }

  async function applyExternalPath(path: PathSeg[]): Promise<void> {
    if (!keepCollapsed) collapsed = false;
    await navigate(path, { reveal: 'none' });
  }

  function selectedItem(): ColumnNavigatorColumnItem | null {
    const parentKey = workspacePathKey(activePath.slice(0, -1));
    const selectedKey = workspacePathKey(activePath);
    const parent = chain.find((pane) => pane.kind === 'column' && pane.pathKey === parentKey);
    return parent?.items.find((item) => item.pathKey === selectedKey) ?? null;
  }

  async function moveSibling(delta: -1 | 1): Promise<void> {
    if (!activePath.length) return;
    const parent = chain.find(
      (pane) => pane.kind === 'column' && pane.pathKey === workspacePathKey(activePath.slice(0, -1)),
    );
    if (!parent?.items.length) return;
    const index = parent.items.findIndex((item) => item.pathKey === workspacePathKey(activePath));
    const currentIndex = index < 0 ? 0 : index;
    const nextIndex = (currentIndex + delta + parent.items.length) % parent.items.length;
    const item = parent.items[nextIndex];
    if (item && item.pathKey !== workspacePathKey(activePath)) {
      if (hasStableSiblingPaneStructure(item)) {
        await selectSibling(item.path);
        return;
      }
      await navigate(item.path, { reveal: 'immediate' });
    }
  }

  async function enterSelected(): Promise<void> {
    const selected = selectedItem();
    const column = chain.find(
      (pane) => pane.kind === 'column' && pane.pathKey === workspacePathKey(activePath),
    );
    if (selected?.isContainer && column?.items[0]) {
      await selectPath(column.items[0].path);
      return;
    }
    if (!selected && column?.items[0]) await selectPath(column.items[0].path);
  }

  async function navigateParent(): Promise<void> {
    if (!activePath.length) return;
    await selectPath(activePath.slice(0, -1));
  }

  async function runValueEditLoop(pane: ColumnNavigatorPaneState, initialText: string): Promise<void> {
    let nextText = initialText;
    let currentPane = pane;
    while (nextText !== currentPane.content?.sourceText) {
      if (currentPane.content?.snapshotId !== deps.getWorkspaceSnapshotId()) {
        await refresh();
        const refreshed = chain.find((entry) => entry.kind === 'content' && entry.pathKey === pane.pathKey);
        if (!refreshed?.content) return;
        currentPane = refreshed;
      }
      const revisionBeforeCommit = deps.getRevision();
      const applied = await deps.applyStructuredValueEdit({
        path: currentPane.path,
        text: currentPane.content!.sourceText,
        valueType: currentPane.content!.valueType,
        snapshotId: currentPane.content!.snapshotId,
        kind: 'value',
        raw: nextText,
        preserveSourceFormatting:
          currentPane.content!.valueType === 'object' || currentPane.content!.valueType === 'array',
      });
      if (!applied || !(await deps.waitForCommittedDocument(deps.getDocumentKey(), revisionBeforeCommit))) {
        queuedEditMap.delete(pane.pathKey);
        return;
      }
      const queuedText = queuedEditMap.get(pane.pathKey);
      queuedEditMap.delete(pane.pathKey);
      if (queuedText == null || queuedText === nextText) return;
      nextText = queuedText;
      const refreshed = chain.find((entry) => entry.kind === 'content' && entry.pathKey === pane.pathKey);
      if (refreshed?.content) currentPane = refreshed;
    }
  }

  async function commitValueEdit(pane: ColumnNavigatorPaneState, draft?: string): Promise<void> {
    if (disposed || deps.getReadonly() || pane.kind !== 'content' || !pane.content) return;
    const nextText = draft ?? pane.content.sourceText;
    if (nextText === pane.content.sourceText) return;
    const pending = pendingEditTaskMap.get(pane.pathKey);
    if (pending) {
      queuedEditMap.set(pane.pathKey, nextText);
      await pending;
      return;
    }
    const task = runValueEditLoop(pane, nextText);
    pendingEditTaskMap.set(pane.pathKey, task);
    try {
      await task;
    } finally {
      if (pendingEditTaskMap.get(pane.pathKey) === task) pendingEditTaskMap.delete(pane.pathKey);
    }
  }

  async function refresh(): Promise<void> {
    if (disposed) return;
    void refreshOperation?.cancel();
    const operation = createWorkspaceOperation();
    refreshOperation = operation;
    const path = clonePath(activePath);
    // Projection refreshes belong to the navigation request that opened this
    // path; changing the id here would orphan readiness and stale Monaco work.
    const nextRequestId =
      chain.find((pane) => pane.pathKey === workspacePathKey(path))?.requestId ??
      requestId;
    await operation.run({
      execute: ({ step }) => step(() => prepareWorkspace(path, nextRequestId)),
      land: (prepared) => {
        if (disposed) return;
        activePath = clonePath(prepared.activePath);
        chain = prepared.chain;
        isLoading = false;
        emitState();
      },
    });
  }

  async function syncProjection(input: ColumnNavigatorProjectionInput): Promise<void> {
    const nextSignature = [
      input.documentKey,
      input.languageId,
      input.revision,
      input.graphAppliedRevision,
      input.snapshotId ?? 'no-snapshot',
      input.enableNest ? 'nest' : 'flat',
    ].join('|');
    if (nextSignature === projectionSignature) return;
    projectionSignature = nextSignature;
    projectionEpoch += 1;
    void navigationOperation?.cancel();
    void refreshOperation?.cancel();
    pathValueCache.clear();
    directChildrenCache.clear();
    await refresh();
  }

  function clampHeight(nextHeightPx: number): number {
    return getClampedPaneSize(
      nextHeightPx,
      deps.getShellHeight(),
      COLUMN_NAVIGATOR_MIN_HEIGHT,
      COLUMN_NAVIGATOR_MAX_HEIGHT_FRACTION,
    );
  }

  function startDividerDrag(clientY: number): void {
    isDraggingDivider = true;
    resizeState = { startClientY: clientY, startHeightPx: heightPx };
    emitState();
  }

  function moveDividerDrag(clientY: number): void {
    if (!resizeState) return;
    heightPx = clampHeight(resizeState.startHeightPx + resizeState.startClientY - clientY);
    emitState();
  }

  function endDividerDrag(): void {
    isDraggingDivider = false;
    resizeState = null;
    emitState();
  }

  function setHeight(nextHeightPx: number): void {
    const next = clampHeight(nextHeightPx);
    if (next === heightPx) return;
    heightPx = next;
    emitState();
  }

  function syncHeightToShell(): void {
    if (deps.getShellHeight() <= 0) return;
    const next = clampHeight(heightPx);
    if (next === heightPx) return;
    heightPx = next;
    emitState();
  }

  function collapse(): void {
    if (collapsed) return;
    collapsed = true;
    emitState();
    deps.publishExpanded(false);
  }

  function expand(): void {
    if (!collapsed && !keepCollapsed) return;
    keepCollapsed = false;
    collapsed = false;
    emitState();
    deps.publishExpanded(true);
  }

  function pinCollapsed(): void {
    keepCollapsed = true;
    collapsed = true;
    emitState();
    deps.publishExpanded(false);
  }

  function reset(): void {
    projectionEpoch += 1;
    void navigationOperation?.cancel();
    void refreshOperation?.cancel();
    requestId += 1;
    collapsed = true;
    isLoading = false;
    activePath = [];
    chain = [];
    canGoBack = false;
    canGoForward = false;
    pendingEditTaskMap.clear();
    queuedEditMap.clear();
    deps.clearSearchHighlight();
    deps.clearActiveGraphSelection();
    pathValueCache.clear();
    directChildrenCache.clear();
    emitState();
  }

  async function restoreNavigationState(state: ColumnNavigatorNavigationState): Promise<void> {
    reset();
    canGoBack = state.canGoBack;
    canGoForward = state.canGoForward;
    keepCollapsed = state.collapsed;
    collapsed = state.collapsed;
    if (state.activePath.length) {
      await openPath(state.activePath);
      return;
    }
    emitState();
  }

  function projectNavigationState(state: ColumnNavigatorNavigationState): void {
    activePath = clonePath(state.activePath);
    canGoBack = state.canGoBack;
    canGoForward = state.canGoForward;
    keepCollapsed = state.collapsed;
    collapsed = state.collapsed;
    emitState();
  }

  function dispose(): void {
    if (disposed) return;
    reset();
    disposed = true;
  }

  emitState();

  return {
    getChain: () => chain,
    getActivePath: () => activePath,
    getVisiblePanes: visiblePanes,
    openPath,
    selectPath,
    applyExternalPath,
    moveSibling,
    enterSelected,
    navigateParent,
    goBack: () => deps.publishHistoryTraversal(-1),
    goForward: () => deps.publishHistoryTraversal(1),
    collapse,
    pinCollapsed,
    expand,
    commitValueEdit,
    syncProjection,
    restoreNavigationState,
    projectNavigationState,
    reset,
    dispose,
    syncHeightToShell,
    setHeight,
    startDividerDrag,
    moveDividerDrag,
    endDividerDrag,
  };
}
