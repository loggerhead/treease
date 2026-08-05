import { describe, expect, it } from 'vitest';
import { decideNavigationBehavior } from './navigation-behavior-policy';
import type { NavigationTarget, NavigationUserEvent } from './navigation-contract';

const target: NavigationTarget = {
  workspaceId: 'workspace', tabId: 'tab-a', documentKey: 'document', generation: 1, revision: 2,
};

function event(kind: NavigationUserEvent['kind']): NavigationUserEvent {
  switch (kind) {
    case 'editor-selection':
    case 'graph-cell':
    case 'navigator-column':
    case 'navigator-tree-path':
    case 'search-commit':
      return { kind, target, path: [], cellTarget: 'node', ...(kind === 'search-commit' ? { previewId: 'preview' } : {}) } as NavigationUserEvent;
    case 'search-preview':
      return { kind, target, path: [], cellTarget: 'node', previewId: 'preview' };
    case 'search-cancel':
      return { kind, target, previewId: 'preview' };
    case 'navigator-history':
      return { kind, target, direction: -1 };
    case 'navigator-visibility':
      return { kind, target, expanded: false };
    default:
      return { kind, target };
  }
}

describe('navigation behavior policy', () => {
  it.each([
    ['editor-selection', 'locate', 'navigate'],
    ['graph-cell', 'locate', 'navigate'],
    ['navigator-column', 'locate', 'navigate'],
    ['navigator-tree-path', 'navigate', 'navigate'],
    ['navigator-history', 'navigate', 'navigate'],
    ['navigator-visibility', 'navigate', 'navigate'],
    ['search-preview', 'graph-highlight-preview', 'graph-viewport-preview'],
    ['search-commit', 'navigate', 'navigate'],
    ['search-cancel', 'cancel-preview', 'cancel-preview'],
    ['graph-viewport-gesture', 'none', 'none'],
    ['editor-edit', 'none', 'none'],
    ['editor-scroll', 'none', 'none'],
    ['tab-activated', 'none', 'none'],
    ['state-restored', 'none', 'none'],
  ] as const)('maps %s uniquely', (kind, disabled, enabled) => {
    expect(decideNavigationBehavior(event(kind), { completeNavigationEnabled: false })).toBe(disabled);
    expect(decideNavigationBehavior(event(kind), { completeNavigationEnabled: true })).toBe(enabled);
  });
});
