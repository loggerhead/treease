export const semanticTypeColorKeys = ['map', 'key', 'seq', 'str', 'int', 'float', 'boolean', 'nil'] as const;

export type SemanticTypeColorKey = (typeof semanticTypeColorKeys)[number];
export type SemanticTypeColorPalette = Record<SemanticTypeColorKey, string>;

/** Core SemType is the only semantic classification used for value text. */
export function semanticTypeToColorKey(semType: number | null | undefined): SemanticTypeColorKey {
  switch (semType) {
    case 0:
      return 'map';
    case 1:
      return 'seq';
    case 3:
      return 'int';
    case 4:
      return 'float';
    case 5:
      return 'boolean';
    case 6:
      return 'nil';
    case 2:
    default:
      return 'str';
  }
}

export function resolveSemanticTypeColor(
  colors: SemanticTypeColorPalette,
  semType: number | null | undefined,
): string {
  return colors[semanticTypeToColorKey(semType)];
}
