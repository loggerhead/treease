import type * as Monaco from 'monaco-editor';

import type { EditorAnalysisLike } from './editor-analysis-apply';
import type { RootValueKind } from '../../services/SnapshotProjectionService';

export type RootScalarHighlightKind = 'str' | 'int' | 'float' | 'boolean' | 'nil';

export function resolveRootScalarHighlightKindFromSnapshotKind(
  value: RootValueKind | null | undefined,
): RootScalarHighlightKind | null {
  if (value === 'string') return 'str';
  if (value === 'int') return 'int';
  if (value === 'float') return 'float';
  if (value === 'boolean') return 'boolean';
  if (value === 'null') return 'nil';
  return null;
}

export function resolveRootScalarHighlightKindFromValue(value: unknown): RootScalarHighlightKind | null {
  if (typeof value === 'string') return 'str';
  if (typeof value === 'number') return Number.isInteger(value) ? 'int' : 'float';
  if (typeof value === 'boolean') return 'boolean';
  if (value === null) return 'nil';
  return null;
}

export function resolveRootScalarHighlightKind(
  analysis: EditorAnalysisLike | null | undefined,
): RootScalarHighlightKind | null {
  return resolveRootScalarHighlightKindFromValue(analysis?.value);
}

export function resolveJsonRootScalarHighlightKindFromText(
  text: string,
  language: string | null | undefined,
): RootScalarHighlightKind | null {
  if (language !== 'json') return null;
  try {
    return resolveRootScalarHighlightKindFromValue(JSON.parse(text));
  } catch {
    return null;
  }
}

export function buildRootScalarHighlightDecorations(
  monaco: typeof import('monaco-editor') | undefined,
  model: Monaco.editor.ITextModel | null,
  kind: RootScalarHighlightKind | null,
): Monaco.editor.IModelDeltaDecoration[] {
  if (!monaco || !model || !kind) return [];
  return [
    {
      range: model.getFullModelRange(),
      options: {
        inlineClassName: `treease-root-scalar-${kind}`,
      },
    },
  ];
}
