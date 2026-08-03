import { describe, expect, it } from 'vitest';
import { validateWorkspaceSession, workspaceTabInputFromSession } from './workspace-session';

describe('workspace session cloud sync state', () => {
  it('restores a valid tab sync state', () => {
    const session = {
      version: 1 as const,
      activeTabIndex: 0,
      tabs: [{
        name: 'draft.json',
        languageId: 'json',
        sourceText: '{}',
        syncStatus: 'pending' as const,
      }],
    };

    const validation = validateWorkspaceSession(session);
    expect(validation.kind).toBe('valid');
    expect(workspaceTabInputFromSession(session.tabs[0], 'tab-1').syncStatus).toBe('pending');
  });

  it('rejects an unknown tab sync state', () => {
    expect(validateWorkspaceSession({
      version: 1,
      activeTabIndex: 0,
      tabs: [{ name: 'draft.json', languageId: 'json', sourceText: '{}', syncStatus: 'stalled' }],
    })).toEqual({ kind: 'invalid', reason: 'Workspace session tab 0 has invalid fields.' });
  });
});
