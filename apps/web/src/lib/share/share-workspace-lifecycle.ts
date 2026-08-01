import type { EditorWorkspaceState } from '../store/editor-workspace';
import type { WorkspaceSession } from '../workspace-host';
import { workspaceSessionFromWorkspace } from '../workspace-host/workspace-session';
import { promoteSharedWorkspaceTopology } from './share-workspace-topology';

export type SharedWorkspaceMutationTarget = {
  tabId: string;
  documentKey: string;
  readDocumentKey(): string;
  readText(): string;
  isCurrent(): boolean;
};

export type SharedWorkspaceLifecycleState =
  | { kind: 'inactive' }
  | { kind: 'restoring' }
  | { kind: 'restore-failed'; error: string }
  | { kind: 'ephemeral-ready' }
  | { kind: 'promoting' }
  | { kind: 'promotion-failed'; error: string }
  | { kind: 'promoted-pending-persist'; attempt: number; error: string | null }
  | { kind: 'promoted' }
  | { kind: 'disposed' };

type RetryHandle = ReturnType<typeof setTimeout>;

type SharedWorkspaceLifecycleDeps = {
  loadSession(): Promise<unknown>;
  saveSession(session: WorkspaceSession): Promise<void>;
  getWorkspace(): EditorWorkspaceState;
  publishWorkspace(workspace: EditorWorkspaceState): void;
  onTopologyPublished(): void;
  enableSessionPersistence(): void;
  reportError(message: string): void;
  scheduleRetry?: (retry: () => void, delayMs: number) => RetryHandle;
  cancelRetry?: (handle: RetryHandle) => void;
};

export type SharedWorkspaceLifecycle = ReturnType<typeof createSharedWorkspaceLifecycle>;

const persistenceRetryDelays = [500, 1_500, 5_000] as const;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function createSharedWorkspaceLifecycle(deps: SharedWorkspaceLifecycleDeps) {
  let state: SharedWorkspaceLifecycleState = { kind: 'inactive' };
  let promoteFlight: Promise<boolean> | null = null;
  let retryHandle: RetryHandle | null = null;
  let persistenceEnabled = false;
  let generation = 0;
  const scheduleRetry = deps.scheduleRetry ?? ((retry, delayMs) => setTimeout(retry, delayMs));
  const cancelRetry = deps.cancelRetry ?? ((handle) => clearTimeout(handle));

  function setState(next: SharedWorkspaceLifecycleState): void {
    state = next;
  }

  function beginRestore(): void {
    generation += 1;
    setState({ kind: 'restoring' });
  }

  function completeRestore(): void {
    if (state.kind === 'restoring') setState({ kind: 'ephemeral-ready' });
  }

  function failRestore(error: unknown): void {
    if (state.kind !== 'restoring') return;
    setState({ kind: 'restore-failed', error: errorMessage(error) });
  }

  function enablePersistenceOnce(): void {
    if (persistenceEnabled) return;
    persistenceEnabled = true;
    deps.enableSessionPersistence();
  }

  function isCancelled(expectedGeneration: number): boolean {
    return state.kind === 'disposed' || expectedGeneration !== generation;
  }

  async function persistPublishedWorkspace(expectedGeneration: number, attempt: number): Promise<void> {
    if (isCancelled(expectedGeneration)) return;
    try {
      await deps.saveSession(workspaceSessionFromWorkspace(deps.getWorkspace()));
      if (isCancelled(expectedGeneration)) return;
      setState({ kind: 'promoted' });
      enablePersistenceOnce();
    } catch (error) {
      if (isCancelled(expectedGeneration)) return;
      const message = errorMessage(error);
      setState({ kind: 'promoted-pending-persist', attempt, error: message });
      deps.reportError(`Local workspace recovery data has not been saved: ${message}`);
      const delay = persistenceRetryDelays[attempt];
      if (delay === undefined) return;
      retryHandle = scheduleRetry(() => {
        retryHandle = null;
        void persistPublishedWorkspace(expectedGeneration, attempt + 1);
      }, delay);
    }
  }

  async function runPromote(target: SharedWorkspaceMutationTarget, expectedGeneration: number): Promise<boolean> {
    try {
      const persistedSession = await deps.loadSession();
      if (isCancelled(expectedGeneration) || !target.isCurrent()) return false;
      const result = promoteSharedWorkspaceTopology({
        workspace: deps.getWorkspace(),
        persistedSession,
        target: {
          tabId: target.tabId,
          documentKey: target.readDocumentKey(),
          sourceText: target.readText(),
        },
      });
      if (result.kind === 'rejected') {
        const detail = result.detail ? ` ${result.detail}` : '';
        throw new Error(`Unable to merge the shared workspace (${result.reason}).${detail}`);
      }
      if (!target.isCurrent()) return false;
      // Publish the complete topology once. Persistence is only a projection of
      // this authority and must never become a second live Tab topology.
      deps.publishWorkspace(result.workspace);
      deps.onTopologyPublished();
      const targetStillCurrent = target.isCurrent();
      setState({ kind: 'promoted-pending-persist', attempt: 0, error: null });
      void persistPublishedWorkspace(expectedGeneration, 0);
      return targetStillCurrent;
    } catch (error) {
      if (isCancelled(expectedGeneration)) return false;
      const message = errorMessage(error);
      setState({ kind: 'promotion-failed', error: message });
      deps.reportError(`Shared workspace could not be added to local recovery: ${message}`);
      return false;
    }
  }

  function ensurePromoted(target: SharedWorkspaceMutationTarget): Promise<boolean> {
    if (state.kind === 'inactive' || state.kind === 'restoring' || state.kind === 'restore-failed') {
      return Promise.resolve(true);
    }
    if (state.kind === 'promoted' || state.kind === 'promoted-pending-persist') return Promise.resolve(true);
    if (state.kind === 'disposed') return Promise.resolve(false);
    if (promoteFlight) return promoteFlight;
    const expectedGeneration = generation;
    setState({ kind: 'promoting' });
    const flight = runPromote(target, expectedGeneration).finally(() => {
      if (promoteFlight === flight) promoteFlight = null;
    });
    promoteFlight = flight;
    return flight;
  }

  function observeDirectDraftMutation(target: SharedWorkspaceMutationTarget): void {
    void ensurePromoted(target);
  }

  function dispose(): void {
    generation += 1;
    if (retryHandle !== null) cancelRetry(retryHandle);
    retryHandle = null;
    promoteFlight = null;
    setState({ kind: 'disposed' });
  }

  return {
    getState: () => state,
    beginRestore,
    completeRestore,
    failRestore,
    ensurePromoted,
    observeDirectDraftMutation,
    dispose,
  };
}
