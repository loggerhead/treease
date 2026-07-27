import { buildUtf8ByteSegments, byteOffsetToUtf16Position, type Utf8ByteSegment } from '../../shared/document-anchor-utils';

export type DiffChunk = {
  byteOffset: number;
  byteLength: number;
  type: number;
  inlineDiffs: DiffChunk[];
};

export type DiffPair = {
  hasLeft: number;
  left: DiffChunk;
  hasRight: number;
  right: DiffChunk;
};

export type DiffRange = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  type: number;
  inlineDiffs: DiffRange[];
};

export type FillRange = {
  startLineNumber: number;
  endLineNumber: number;
};

export type DiffDecoration = {
  range: {
    startLineNumber: number;
    startColumn: number;
    endLineNumber: number;
    endColumn: number;
  };
  options: any;
};

export type DiffPlan = {
  decorations: DiffDecoration[];
  fillRanges: FillRange[];
  firstLine?: number;
};

type DiffPairRanges = {
  left?: DiffRange;
  right?: DiffRange;
};

function byteOffsetToLineColumn(
  segments: Utf8ByteSegment[],
  byteOffset: number,
  bias: 'start' | 'end',
): { line: number; column: number } {
  const position = byteOffsetToUtf16Position(segments, byteOffset, bias);
  return { line: position.row + 1, column: position.column + 1 };
}

function diffToRange(segments: Utf8ByteSegment[], diff: DiffChunk): DiffRange {
  const start = byteOffsetToLineColumn(segments, diff.byteOffset, 'start');
  const end = byteOffsetToLineColumn(segments, diff.byteOffset + Math.max(diff.byteLength, 1), 'end');
  const isSingleNewline = diff.byteLength === 1 && start.column === 1 && end.column === 1;
  return {
    startLineNumber: start.line,
    startColumn: start.column,
    endLineNumber: isSingleNewline ? start.line : end.line,
    endColumn: isSingleNewline ? start.column : end.column,
    type: diff.type,
    inlineDiffs: diff.inlineDiffs?.map((inline) => diffToRange(segments, inline)) ?? [],
  };
}

function buildPairRanges(pairs: DiffPair[], leftText: string, rightText: string): DiffPairRanges[] {
  const leftSegments = buildUtf8ByteSegments(leftText);
  const rightSegments = buildUtf8ByteSegments(rightText);
  return pairs.map((pair) => {
    const left = pair.hasLeft ? diffToRange(leftSegments, pair.left) : undefined;
    const right = pair.hasRight ? diffToRange(rightSegments, pair.right) : undefined;
    return { left, right };
  });
}

function mergeDecorations(decorations: DiffDecoration[]): DiffDecoration[] {
  if (decorations.length === 0) return [];
  const sorted = [...decorations].sort((a, b) => a.range.startLineNumber - b.range.startLineNumber);
  const merged = [sorted[0]];
  for (const decoration of sorted.slice(1)) {
    const prev = merged[merged.length - 1];
    if (decoration.range.startLineNumber <= prev.range.endLineNumber) {
      prev.range.endLineNumber = Math.max(decoration.range.endLineNumber, prev.range.endLineNumber);
    } else {
      merged.push(decoration);
    }
  }
  return merged;
}

function buildDecorations(monaco: any, pairs: DiffPairRanges[], side: 'left' | 'right'): DiffDecoration[] {
  const hunkDecorations: DiffDecoration[] = [];
  const inlineDecorations: DiffDecoration[] = [];
  for (const pair of pairs) {
    const diff = side === 'left' ? pair.left : pair.right;
    if (!diff) continue;
    const typeSuffix = diff.type === 0 ? 'ins' : 'del';
    hunkDecorations.push({
      range: diff,
      options: {
        isWholeLine: true,
        className: `diff-line-${typeSuffix}`,
        marginClassName: `diff-margin-${typeSuffix}`,
        minimap: {
          color: diff.type === 0 ? 'rgba(34,197,94,0.6)' : 'rgba(248,113,113,0.6)',
          position: monaco.editor.MinimapPosition.Inline,
        },
        overviewRuler: {
          color: diff.type === 0 ? 'rgba(34,197,94,0.8)' : 'rgba(248,113,113,0.8)',
          position: monaco.editor.OverviewRulerLane.Full,
        },
      },
    });
    for (const inline of diff.inlineDiffs ?? []) {
      inlineDecorations.push({
        range: inline,
        options: {
          inlineClassName: `diff-inline-${typeSuffix}`,
        },
      });
    }
  }
  return inlineDecorations.concat(mergeDecorations(hunkDecorations));
}

function findFirstLine(pairs: DiffPairRanges[], side: 'left' | 'right'): number | undefined {
  for (const pair of pairs) {
    const diff = side === 'left' ? pair.left : pair.right;
    if (diff) return diff.startLineNumber;
  }
  return undefined;
}

export function buildDiffPlans(
  monaco: any,
  pairs: DiffPair[],
  leftText: string,
  rightText: string,
  fillRanges: { left: FillRange[]; right: FillRange[] } = { left: [], right: [] },
): { left: DiffPlan; right: DiffPlan } {
  const pairRanges = buildPairRanges(pairs, leftText, rightText);
  return {
    left: {
      decorations: buildDecorations(monaco, pairRanges, 'left'),
      fillRanges: fillRanges.left,
      firstLine: findFirstLine(pairRanges, 'left'),
    },
    right: {
      decorations: buildDecorations(monaco, pairRanges, 'right'),
      fillRanges: fillRanges.right,
      firstLine: findFirstLine(pairRanges, 'right'),
    },
  };
}
