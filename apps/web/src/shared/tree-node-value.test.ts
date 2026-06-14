import { describe, expect, it } from 'vitest';
import { clonePlainTreeNode, createParsedTreeData, valueToTreeNode } from './tree-node-value';

describe('shared tree-node-value helpers', () => {
  it('clonePlainTreeNode preserves shape without sharing child references', () => {
    const source = valueToTreeNode({ profile: { name: 'Ada' } });

    const clone = clonePlainTreeNode(source);

    expect(clone).toEqual(source);
    expect(clone).not.toBe(source);
    expect(clone.children?.[0]).not.toBe(source.children?.[0]);
  });

  it('createParsedTreeData preserves the derived value shape', () => {
    const node = valueToTreeNode({ items: ['Ada', 2] });

    expect(createParsedTreeData(node)).toEqual({
      tree: clonePlainTreeNode(node),
      value: { items: ['Ada', 2] },
    });
  });
});
