// Responsibility: convert TreeNode values into displayable previews based on SemType and TreeKind.
import { PathSegTag, TreeKind, type PathSeg, type TreeNode } from '@core-wasm/index'
import { readWasmString } from '../../shared/tree-node-value';

export function readTreeNodeString(node: TreeNode | null | undefined): string {
  if (!node) return '';
  return readWasmString(node.value);
}

export function findNodeByPath(root: TreeNode, path: PathSeg[], preferKey = false): TreeNode | null {
  if (!path.length) return root;
  let current: TreeNode | null = root;
  for (let i = 0; i < path.length; i += 1) {
    if (!current) return null;
    const seg = path[i];
    const isLast = i === path.length - 1;
    if (current.kind === TreeKind.MAPPING) {
      if (seg.tag !== PathSegTag.KEY) return null;
      const key = readWasmString(seg.key);
      const children = current.children ?? [];
      let matched = false;
      for (let childIndex = 0; childIndex + 1 < children.length; childIndex += 2) {
        const keyNode = children[childIndex] as TreeNode;
        if (readTreeNodeString(keyNode) !== key) continue;
        matched = true;
        current = isLast && preferKey ? keyNode : (children[childIndex + 1] as TreeNode);
        break;
      }
      if (!matched) return null;
      continue;
    }
    if (current.kind === TreeKind.SEQUENCE) {
      if (seg.tag !== PathSegTag.INDEX || seg.index < 0) return null;
      const child = current.children?.[seg.index] as TreeNode | undefined;
      if (!child) return null;
      current = child;
      continue;
    }
    return null;
  }
  return current;
}

export function isPreviewableNode(node: TreeNode | null | undefined): boolean {
  if (!node) return false;
  return node.kind === TreeKind.SCALAR || node.kind === TreeKind.ALIAS;
}
