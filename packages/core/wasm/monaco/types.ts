import type { PathSeg, PathSpan, TreeNode } from '../serde-types';

// ── Serde-compatible types replacing C-ABI @thi.ng/wasm-api types ──────────
// These match the serde-deserialized output from wasm-bindgen functions.

export type DocumentTextEdit = {
  startByte: number;
  oldEndByte: number;
  newEndByte: number;
  startRow: number;
  startColumn: number;
  oldEndRow: number;
  oldEndColumn: number;
  newEndRow: number;
  newEndColumn: number;
  text: string;
};

export type JsonBlockSpan = {
  found: boolean;
  startByte: number;
  endByte: number;
  startRow: number;
  startColumn: number;
  endRow: number;
  endColumn: number;
};




export type ParseOptions = {
  nest?: boolean;
  /** Reject non-literal editor text instead of coercing it to a string. */
  strictSourceLiteral?: boolean;
};

export type BuilderConfig = {
  keyWidth: number;
  valueWidth: number;
  rowHeight: number;
  rowPaddingX: number;
  rowPaddingY: number;
  nodeBorderWidth: number;
  vGap: number;
  hGap: number;
  tableMaxHeight: number;
  tableRowHeight: number;
  tableHeaderHeight: number;
  tableColumnWidth: number;
  avgCharWidthX10: number;
  fontSize: number;
  metaPathMinSegments: number;
  metaPathMinChars: number;
  metaPathKeepTailSegments: number;
  cornerRadius?: number;
};

export const TREE_NODE_TOKEN_TYPES = [
  'map',
  'key',
  'seq',
  'str',
  'int',
  'float',
  'boolean',
  'nil',
] as const;

export const AUXILIARY_TOKEN_TYPES = [
  'punctuation',
  'comment',
  'operator',
  'function',
  'variable',
  'tag',
  'attribute',
] as const;

export const TOKEN_TYPES = [...TREE_NODE_TOKEN_TYPES, ...AUXILIARY_TOKEN_TYPES] as const;

export type TokenType = (typeof TOKEN_TYPES)[number];
export type TreeNodeTokenType = (typeof TREE_NODE_TOKEN_TYPES)[number];
export type AuxiliaryTokenType = (typeof AUXILIARY_TOKEN_TYPES)[number];

export const TOKEN_TYPE_LAYER: Record<TokenType, 'tree-node' | 'auxiliary'> = {
  map: 'tree-node',
  key: 'tree-node',
  seq: 'tree-node',
  str: 'tree-node',
  int: 'tree-node',
  float: 'tree-node',
  boolean: 'tree-node',
  nil: 'tree-node',
  punctuation: 'auxiliary',
  comment: 'auxiliary',
  operator: 'auxiliary',
  function: 'auxiliary',
  variable: 'auxiliary',
  tag: 'auxiliary',
  attribute: 'auxiliary',
};

export const TOKEN_TYPE_THEME_KEY: Record<TokenType, TreeNodeTokenType | AuxiliaryTokenType> = {
  map: 'map',
  key: 'key',
  seq: 'seq',
  str: 'str',
  int: 'int',
  float: 'float',
  boolean: 'boolean',
  nil: 'nil',
  punctuation: 'punctuation',
  comment: 'comment',
  operator: 'operator',
  function: 'function',
  variable: 'map',
  tag: 'map',
  attribute: 'key',
};


export type RawDiff = {
  // UTF-8 byte span in the original source text.
  byteOffset: number;
  byteLength: number;
  type: number;
  inlineDiffs: RawDiff[];
};

export type RawDiffPair = {
  hasLeft: number;
  left: RawDiff;
  hasRight: number;
  right: RawDiff;
};

export type RawDiffFillRange = {
  startLineNumber: number;
  endLineNumber: number;
};

export type RawDiffResult = {
  pairs: RawDiffPair[];
  leftFillRanges: RawDiffFillRange[];
  rightFillRanges: RawDiffFillRange[];
};
export type DiffResult = RawDiffResult;



export type WasmError = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

export type StoredDocumentAnalysis = {
  language: string;
  sourceByteLength: number;
  tree: TreeNode | null;
  valueJson: string;
  diagnostics: Uint32Array;
  semanticTokens: Uint32Array;
};

export type FormatOptions = {
  indent?: number;
  smart?: boolean;
  maxLineLength?: number;
  maxInlineComplexity?: number;
  maxArrayInlineItems?: number;
  alignObjectArrays?: boolean;
  nest?: boolean;
  sortKeys?: boolean;
};



