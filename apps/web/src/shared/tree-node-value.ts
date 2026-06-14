import { SemType, TreeKind, type TreeNode } from '@core-wasm/index';
import { toWasmTreeNode } from './brand-bridge';

const scalarKinds = new Set<SemType>([SemType.STR, SemType.INT, SemType.FLOAT, SemType.BOOLEAN, SemType.NIL]);

export function readWasmString(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof (value as { deref?: () => string }).deref === 'function') {
    const out = (value as { deref: () => string }).deref();
    if (typeof out === 'string') return out;
  }
  if (value && typeof (value as { toString?: () => string }).toString === 'function') {
    const out = (value as { toString: () => string }).toString();
    if (typeof out === 'string' && out !== '[object Object]') return out;
  }
  if (value && typeof (value as { valueOf?: () => unknown }).valueOf === 'function') {
    const out = (value as { valueOf: () => unknown }).valueOf();
    if (typeof out === 'string') return out;
  }
  return '';
}

export function clonePlainTreeNode(node: TreeNode): TreeNode {
  const children = Array.isArray(node.children) ? node.children : [];
  return toWasmTreeNode({
    kind: Number(node.kind) as TreeKind,
    semType: Number(node.semType) as SemType,
    tag: readWasmString(node.tag),
    value: readWasmString(node.value),
    children: children.map((child) => clonePlainTreeNode(child as TreeNode)),
  });
}

function semTypeToValue(value: string, semType: SemType): unknown {
  switch (semType) {
    case SemType.NIL:
      return null;
    case SemType.BOOLEAN: {
      const lowered = value.trim().toLowerCase();
      return lowered === 'true' || lowered === 'yes' || lowered === 'y';
    }
    case SemType.INT: {
      const parsed = Number.parseInt(value, 10);
      return Number.isFinite(parsed) ? parsed : value;
    }
    case SemType.FLOAT: {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : value;
    }
    default:
      return value;
  }
}

export function treeNodeToValue(node: TreeNode): unknown {
  if (node.kind === TreeKind.SEQUENCE) {
    const items = node.children ?? [];
    return items.map((child) => treeNodeToValue(child));
  }
  if (node.kind === TreeKind.MAPPING) {
    const out: Record<string, unknown> = {};
    const entries = node.children ?? [];
    for (let i = 0; i + 1 < entries.length; i += 2) {
      const keyNode = entries[i];
      const valueNode = entries[i + 1];
      const key = readWasmString((keyNode as TreeNode | undefined)?.value);
      out[key] = treeNodeToValue(valueNode);
    }
    return out;
  }
  if (node.kind === TreeKind.ALIAS) return readWasmString(node.value);
  if (node.kind === TreeKind.SCALAR) {
    if (scalarKinds.has(node.semType)) {
      return semTypeToValue(readWasmString(node.value), node.semType);
    }
    return readWasmString(node.value);
  }
  return readWasmString(node.value);
}

export function valueToTreeNode(value: unknown): TreeNode {
  if (value === null || value === undefined) {
    return toWasmTreeNode({ kind: TreeKind.SCALAR, semType: SemType.NIL, tag: '', value: '', children: [] as TreeNode[] });
  }
  if (Array.isArray(value)) {
    const children = value.map((child) => valueToTreeNode(child));
    return toWasmTreeNode({ kind: TreeKind.SEQUENCE, semType: SemType.SEQ, tag: '', value: '', children });
  }
  if (typeof value === 'object') {
    const children: TreeNode[] = [];
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      children.push(
        toWasmTreeNode({
          kind: TreeKind.SCALAR,
          semType: SemType.STR,
          tag: '',
          value: key,
          children: [] as TreeNode[],
        }),
        valueToTreeNode(child),
      );
    }
    return toWasmTreeNode({ kind: TreeKind.MAPPING, semType: SemType.MAP, tag: '', value: '', children });
  }
  if (typeof value === 'string') {
    return toWasmTreeNode({ kind: TreeKind.SCALAR, semType: SemType.STR, tag: '', value, children: [] as TreeNode[] });
  }
  if (typeof value === 'boolean') {
    return toWasmTreeNode({
      kind: TreeKind.SCALAR,
      semType: SemType.BOOLEAN,
      tag: '',
      value: value ? 'true' : 'false',
      children: [] as TreeNode[],
    });
  }
  if (typeof value === 'number') {
    const semType = Number.isInteger(value) ? SemType.INT : SemType.FLOAT;
    return toWasmTreeNode({ kind: TreeKind.SCALAR, semType, tag: '', value: String(value), children: [] as TreeNode[] });
  }
  return toWasmTreeNode({
    kind: TreeKind.SCALAR,
    semType: SemType.STR,
    tag: '',
    value: String(value),
    children: [] as TreeNode[],
  });
}

export type ParsedTreeData = {
  tree: TreeNode;
  value: unknown;
};

export function createParsedTreeData(node: TreeNode): ParsedTreeData {
  const tree = clonePlainTreeNode(node);
  return { tree, value: treeNodeToValue(tree) };
}
