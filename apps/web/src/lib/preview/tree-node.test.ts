// Responsibility: unit tests for tree-node preview conversion.
import { describe, expect, it } from 'vitest';
import { PathSegTag, TreeKind } from '@core-wasm/index'
import { valueToTreeNode } from '../../shared/tree-node-value';
import { findNodeByPath, isPreviewableNode, readTreeNodeString } from './tree-node';

describe('preview tree-node helpers', () => {
  it('finds nested value nodes by key and index path', () => {
    const root = valueToTreeNode({
      meta: {
        nested: ['first', { url: 'https://example.com/avatar.png' }],
      },
    });

    const node = findNodeByPath(
      root,
      [
        { tag: PathSegTag.KEY, key: 'meta', index: 0 } as any,
        { tag: PathSegTag.KEY, key: 'nested', index: 0 } as any,
        { tag: PathSegTag.INDEX, key: '', index: 1 } as any,
        { tag: PathSegTag.KEY, key: 'url', index: 0 } as any,
      ],
      false,
    );

    expect(readTreeNodeString(node)).toBe('https://example.com/avatar.png');
    expect(node?.kind).toBe(TreeKind.SCALAR);
  });

  it('returns the key node when preferKey is enabled on the last mapping segment', () => {
    const root = valueToTreeNode({ title: 'Treease' });

    const keyNode = findNodeByPath(root, [{ tag: PathSegTag.KEY, key: 'title', index: 0 } as any], true);

    expect(readTreeNodeString(keyNode)).toBe('title');
    expect(keyNode?.kind).toBe(TreeKind.SCALAR);
  });

  it('marks only scalar and alias nodes as previewable', () => {
    const mapping = valueToTreeNode({ title: 'Treease' });
    const scalar = findNodeByPath(mapping, [{ tag: PathSegTag.KEY, key: 'title', index: 0 } as any], false);

    expect(isPreviewableNode(mapping)).toBe(false);
    expect(isPreviewableNode(scalar)).toBe(true);
  });
});
