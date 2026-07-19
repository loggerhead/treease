import { SemType } from '@core-wasm/index';

export const semanticTypeColorKeys = ['map', 'key', 'seq', 'str', 'int', 'float', 'boolean', 'nil'] as const;

export type SemanticTypeColorKey = (typeof semanticTypeColorKeys)[number];
export type SemanticTypeColorPalette = Record<SemanticTypeColorKey, string>;

/** Core SemType is the only semantic classification used for value text. */
export function semanticTypeToColorKey(semType: SemType | number | null | undefined): SemanticTypeColorKey {
  switch (semType) {
    case SemType.MAP:
      return 'map';
    case SemType.SEQ:
      return 'seq';
    case SemType.INT:
      return 'int';
    case SemType.FLOAT:
      return 'float';
    case SemType.BOOLEAN:
      return 'boolean';
    case SemType.NIL:
      return 'nil';
    case SemType.STR:
    default:
      return 'str';
  }
}

export function resolveSemanticTypeColor(
  colors: SemanticTypeColorPalette,
  semType: SemType | number | null | undefined,
): string {
  return colors[semanticTypeToColorKey(semType)];
}
