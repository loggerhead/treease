import type { EditorAnalysisLike } from './editor-analysis-apply';
import { semanticTypeToColorKey } from '@treease/graph-viewer-runtime';

export type RootScalarHighlightKind = 'str' | 'int' | 'float' | 'boolean' | 'nil';

/** The analysis tree is Core output, unlike lexical Monaco tokens or source-text parsing. */
export function resolveRootScalarHighlightKindFromTree(tree: unknown): RootScalarHighlightKind | null {
  if (!tree || typeof tree !== 'object' || !('semType' in tree)) return null;
  return resolveRootScalarHighlightKindFromSemType((tree as { semType?: unknown }).semType as number | undefined);
}

/** This projection is the exact SemType returned by Core for the selected node. */
export function resolveRootScalarHighlightKindFromSemType(
  semType: number | null | undefined,
): RootScalarHighlightKind | null {
  const key = semanticTypeToColorKey(semType);
  return key === 'str' || key === 'int' || key === 'float' || key === 'boolean' || key === 'nil' ? key : null;
}

export function resolveRootScalarHighlightKind(
  analysis: EditorAnalysisLike | null | undefined,
): RootScalarHighlightKind | null {
  return resolveRootScalarHighlightKindFromTree(analysis?.tree);
}

/**
 * Seed Monaco's existing semantic-token provider from a Core snapshot when a
 * detached scalar pane mounts before its document analysis is available.
 */
export function buildRootScalarSemanticTokens(
  sourceText: string,
  semType: number | null | undefined,
  tokenTypes: readonly string[],
): ArrayBuffer | null {
  const kind = resolveRootScalarHighlightKindFromSemType(semType);
  if (!kind) return null;
  const tokenType = tokenTypes.indexOf(kind);
  if (tokenType < 0) return null;

  const data: number[] = [];
  let previousLine = 0;
  for (const [line, text] of sourceText.split(/\r?\n/).entries()) {
    if (!text) continue;
    data.push(line - previousLine, 0, text.length, tokenType, 0);
    previousLine = line;
  }
  return data.length === 0 ? null : new Uint32Array(data).buffer;
}
