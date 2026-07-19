import { describe, expect, it } from 'vitest';

import { createShareResource, parseShareResource } from './share-resource';

const left = { text: '{"left":true}', languageId: 'json' as const };
const right = { text: '{"right":true}', languageId: 'json' as const };
const interaction = { treePath: [], focus: null, subgraphWorkspace: { panePaths: [] } };
const input = { left, right, layout: { viewMode: 'graph' as const, activePane: 'left' as const }, viewport: { left: { topLine: 24, scrollLeft: 0 }, right: { topLine: 31, scrollLeft: 0 } }, interaction };

describe('share resource', () => {
  it.each(['equal', 'different'] as const)('creates a compare resource for a successful %s compare', (compareKind) => {
    expect(createShareResource({ ...input, compareKind })).toEqual({
      type: 'compare',
      payload: { schemaVersion: 1, left, right, actions: [{ type: 'compare' }, { type: 'viewport_changed', payload: input.viewport }], interaction },
    });
  });

  it('creates an explicit single-sided snapshot when compare state is none', () => {
    expect(createShareResource({ ...input, compareKind: 'none', right: null })).toEqual({
      type: 'text_snapshot',
      payload: { schemaVersion: 1, left, right: null, layout: input.layout, interaction },
    });
  });

  it('rejects invalid public payloads', () => {
    expect(parseShareResource({ type: 'compare', payload: { schemaVersion: 1, left, right, actions: [{ type: 'viewport_changed', payload: { left: { topLine: 0, scrollLeft: 0 }, right: { topLine: 1, scrollLeft: 0 } } }] } })).toBeNull();
    expect(parseShareResource({ type: 'unknown', payload: {} })).toBeNull();
    expect(parseShareResource({ type: 'text_snapshot', payload: { schemaVersion: 1, left, right: null, layout: input.layout, cache: {} } })).toBeNull();
    expect(parseShareResource({ type: 'text_snapshot', payload: { schemaVersion: 1, left, right: null, layout: input.layout, interaction: { ...interaction, unknown: true } } })).toBeNull();
    expect(parseShareResource({ type: 'text_snapshot', payload: { schemaVersion: 1, left, right: null, layout: input.layout, interaction: { treePath: [{ type: 'key', key: 'a', extra: true }], focus: null, subgraphWorkspace: { panePaths: [] } } } })).toBeNull();
  });
});
