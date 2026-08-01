import { describe, expect, it, vi } from 'vitest';

import { initialFullEditUiState } from '../store/full-edit-ui-state';
import { initialTempModel } from '../store/graph-selection-store';
import { createEditorWorkspaceState, type EditorWorkspaceState } from '../store/editor-workspace';
import { createSharedWorkspaceLifecycle, type SharedWorkspaceMutationTarget } from './share-workspace-lifecycle';

function ephemeralWorkspace(): EditorWorkspaceState {
  return createEditorWorkspaceState({
    id: 'share-tab',
    role: 'primary',
    name: 'Shared document',
    documentKey: 'share-tab:0',
    languageId: 'json',
    sourceText: '{}',
    origin: 'import',
    revision: 1,
    graphAppliedRevision: 1,
    snapshotId: 4,
    tempModel: initialTempModel,
    fullEditUiState: initialFullEditUiState,
  });
}

function target(text = '{"edited":true}'): SharedWorkspaceMutationTarget {
  return {
    tabId: 'share-tab',
    documentKey: 'share-tab:0',
    readDocumentKey: () => 'share-tab:0',
    readText: () => text,
    isCurrent: () => true,
  };
}

describe('share workspace lifecycle', () => {
  it('single-flights concurrent promotion and publishes before saving', async () => {
    let workspace = ephemeralWorkspace();
    let resolveLoad!: (value: unknown) => void;
    const events: string[] = [];
    const loadSession = vi.fn(() => new Promise<unknown>((resolve) => { resolveLoad = resolve; }));
    const saveSession = vi.fn(async () => { events.push('save'); });
    const lifecycle = createSharedWorkspaceLifecycle({
      loadSession,
      saveSession,
      getWorkspace: () => workspace,
      publishWorkspace: (next) => { events.push('publish'); workspace = next; },
      onTopologyPublished: () => { events.push('ready'); },
      enableSessionPersistence: () => { events.push('subscribe'); },
      reportError: vi.fn(),
    });
    lifecycle.beginRestore();
    lifecycle.completeRestore();

    const first = lifecycle.ensurePromoted(target());
    const second = lifecycle.ensurePromoted(target());
    expect(first).toBe(second);
    resolveLoad({ version: 1, activeTabIndex: 0, tabs: [{ name: 'Old', languageId: 'json', sourceText: '{"old":1}' }] });

    await expect(first).resolves.toBe(true);
    await vi.waitFor(() => expect(lifecycle.getState().kind).toBe('promoted'));
    expect(loadSession).toHaveBeenCalledTimes(1);
    expect(workspace.tabOrder).toEqual(['session-tab-0', 'share-tab']);
    expect(workspace.tabsById['share-tab'].sourceText).toBe('{"edited":true}');
    expect(events).toEqual(['publish', 'ready', 'save', 'subscribe']);
  });

  it('does not publish when the target closes while session loading', async () => {
    let workspace = ephemeralWorkspace();
    let current = true;
    let resolveLoad!: (value: unknown) => void;
    const publishWorkspace = vi.fn((next: EditorWorkspaceState) => { workspace = next; });
    const lifecycle = createSharedWorkspaceLifecycle({
      loadSession: () => new Promise((resolve) => { resolveLoad = resolve; }),
      saveSession: vi.fn(),
      getWorkspace: () => workspace,
      publishWorkspace,
      onTopologyPublished: vi.fn(),
      enableSessionPersistence: vi.fn(),
      reportError: vi.fn(),
    });
    lifecycle.beginRestore();
    lifecycle.completeRestore();
    const promotion = lifecycle.ensurePromoted({ ...target(), isCurrent: () => current });
    current = false;
    resolveLoad(null);

    await expect(promotion).resolves.toBe(false);
    expect(publishWorkspace).not.toHaveBeenCalled();
  });

  it('blocks a waiting command when its target closes during topology publication', async () => {
    let workspace = ephemeralWorkspace();
    let current = true;
    const saveSession = vi.fn(async () => {});
    const lifecycle = createSharedWorkspaceLifecycle({
      loadSession: async () => null,
      saveSession,
      getWorkspace: () => workspace,
      publishWorkspace: (next) => {
        workspace = next;
        current = false;
      },
      onTopologyPublished: vi.fn(),
      enableSessionPersistence: vi.fn(),
      reportError: vi.fn(),
    });
    lifecycle.beginRestore();
    lifecycle.completeRestore();

    await expect(lifecycle.ensurePromoted({ ...target(), isCurrent: () => current })).resolves.toBe(false);
    expect(workspace.tabOrder).toEqual(['share-tab']);
    await vi.waitFor(() => expect(saveSession).toHaveBeenCalledTimes(1));
  });

  it('keeps promoted topology when persistence fails and retries a current projection', async () => {
    let workspace = ephemeralWorkspace();
    const retries: Array<() => void> = [];
    const saveSession = vi.fn()
      .mockRejectedValueOnce(new Error('disk busy'))
      .mockResolvedValueOnce(undefined);
    const enableSessionPersistence = vi.fn();
    const reportError = vi.fn();
    const lifecycle = createSharedWorkspaceLifecycle({
      loadSession: async () => null,
      saveSession,
      getWorkspace: () => workspace,
      publishWorkspace: (next) => { workspace = next; },
      onTopologyPublished: vi.fn(),
      enableSessionPersistence,
      reportError,
      scheduleRetry: (retry) => { retries.push(retry); return 1 as unknown as ReturnType<typeof setTimeout>; },
      cancelRetry: vi.fn(),
    });
    lifecycle.beginRestore();
    lifecycle.completeRestore();

    await expect(lifecycle.ensurePromoted(target())).resolves.toBe(true);
    await vi.waitFor(() => expect(lifecycle.getState().kind).toBe('promoted-pending-persist'));
    expect(workspace.activeTabId).toBe('share-tab');
    expect(enableSessionPersistence).not.toHaveBeenCalled();
    expect(reportError).toHaveBeenCalledWith(expect.stringContaining('disk busy'));

    retries[0]();
    await vi.waitFor(() => expect(lifecycle.getState().kind).toBe('promoted'));
    expect(saveSession).toHaveBeenCalledTimes(2);
    expect(enableSessionPersistence).toHaveBeenCalledTimes(1);
  });

  it('does not promote during restore and allows retry after a merge failure', async () => {
    let workspace = ephemeralWorkspace();
    const loadSession = vi.fn()
      .mockResolvedValueOnce({ version: 1, activeTabIndex: 0, tabs: [{ name: 'Broken' }] })
      .mockResolvedValueOnce(null);
    const lifecycle = createSharedWorkspaceLifecycle({
      loadSession,
      saveSession: vi.fn(),
      getWorkspace: () => workspace,
      publishWorkspace: (next) => { workspace = next; },
      onTopologyPublished: vi.fn(),
      enableSessionPersistence: vi.fn(),
      reportError: vi.fn(),
    });
    lifecycle.beginRestore();
    await expect(lifecycle.ensurePromoted(target())).resolves.toBe(true);
    expect(loadSession).not.toHaveBeenCalled();
    lifecycle.completeRestore();

    await expect(lifecycle.ensurePromoted(target())).resolves.toBe(false);
    expect(lifecycle.getState().kind).toBe('promotion-failed');
    await expect(lifecycle.ensurePromoted(target())).resolves.toBe(true);
    expect(loadSession).toHaveBeenCalledTimes(2);
  });
});
