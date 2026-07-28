import { describe, expect, it } from 'vitest';
import { TabStateStore } from './tab-state';

describe('TabStateStore', () => {
  it('keeps data isolated by tab', () => {
    const store = new TabStateStore();
    store.set(7, { status: 'invalid', message: 'bad', position: 2, pageTitle: 'A', pageOrigin: 'https://a.test' });
    expect(store.get(7).status).toBe('invalid');
    expect(store.get(8)).toEqual({ status: 'empty' });
  });

  it('releases expired JSON without persisting it', () => {
    const store = new TabStateStore();
    store.set(7, { status: 'ready', document: {
      text: '{"a":1}', sourceTag: 'pre', domPath: 'main > pre', sourceLength: 7, pageTitle: 'A', pageOrigin: 'https://a.test', rootType: 'object', language: 'json', candidateKind: 'whole', expiresAt: 10,
    } });
    expect(store.get(7, 10)).toEqual({ status: 'empty' });
  });
});
