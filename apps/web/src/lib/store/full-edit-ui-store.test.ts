import { afterEach, describe, expect, it } from 'vitest';
import {
  beginFullEditStream,
  finishFullEditStream,
  getFullEditUiStateRaw,
  registerFullEditUiCoordinator,
  resetFullEditUiState,
} from './full-edit-ui-store';

describe('full-edit UI state ownership', () => {
  afterEach(() => {
    registerFullEditUiCoordinator(null);
    resetFullEditUiState();
  });

  it('notifies the Workspace coordinator when an owned session finishes', () => {
    const observedPhases: string[] = [];
    registerFullEditUiCoordinator({
      onFullEditUiStateChange: (next) => observedPhases.push(next.phase),
    });

    beginFullEditStream({
      sessionId: 'session-1',
      ownerKey: 'owner-1',
      documentKey: 'document-1',
      revision: 1,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    finishFullEditStream({ sessionId: 'session-1', ownerKey: 'owner-1' });

    expect(observedPhases).toEqual(['streaming', 'idle']);
    expect(getFullEditUiStateRaw().phase).toBe('idle');
  });
});
