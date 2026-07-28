import type { PanelState } from '../shared/types';

export class TabStateStore {
  private readonly states = new Map<number, PanelState>();

  get(tabId: number, now = Date.now()): PanelState {
    const state = this.states.get(tabId) ?? { status: 'empty' };
    if (state.status === 'ready' && state.document.expiresAt <= now) {
      this.states.delete(tabId);
      return { status: 'empty' };
    }
    return state;
  }

  set(tabId: number, state: PanelState): void {
    this.states.set(tabId, state);
  }

  clear(tabId: number): void {
    this.states.delete(tabId);
  }
}
