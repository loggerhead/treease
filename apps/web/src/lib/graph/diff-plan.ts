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

function countRange(range?: DiffRange): number {
  if (!range) return 0;
  return Math.max(0, range.endLineNumber - range.startLineNumber + 1);
}

function rangeMinus(range: FillRange, n: number): FillRange {
  return { startLineNumber: range.startLineNumber - n, endLineNumber: range.endLineNumber - n };
}

function buildFillRanges(pairs: DiffPairRanges[]): { left: FillRange[]; right: FillRange[] } {
  const leftRanges: FillRange[] = [];
  const rightRanges: FillRange[] = [];
  let lAggr = 0;
  let rAggr = 0;

  const addLeftFill = (fill: FillRange) => {
    const lineCount = countRange(fill as DiffRange);
    if (lineCount <= 0) return;
    leftRanges.push(rangeMinus(fill, lAggr));
    lAggr += lineCount;
  };

  const addRightFill = (fill: FillRange) => {
    const lineCount = countRange(fill as DiffRange);
    if (lineCount <= 0) return;
    rightRanges.push(rangeMinus(fill, rAggr));
    rAggr += lineCount;
  };

  for (let index = 0; index < pairs.length; index += 1) {
    const { left, right } = pairs[index];
    const prev = pairs[index - 1];
    const next = pairs[index + 1];
    const lStart = left?.startLineNumber ?? 0;
    const lEnd = left?.endLineNumber ?? 0;
    const rStart = right?.startLineNumber ?? 0;
    const rEnd = right?.endLineNumber ?? 0;
    const lFillStart = left ? lStart + lAggr : 0;
    const lFillEnd = left ? lEnd + lAggr : 0;
    const rFillStart = right ? rStart + rAggr : 0;
    const rFillEnd = right ? rEnd + rAggr : 0;
    const internalLeftOnly = left && !right && prev?.left && prev?.right && next?.left && next?.right;
    const internalRightOnly = !left && right && prev?.left && prev?.right && next?.left && next?.right;

    if (lFillEnd < rFillStart || rFillEnd < lFillStart) {
      if (left) {
        const fill = { startLineNumber: lStart + lAggr, endLineNumber: lEnd + lAggr };
        if (internalLeftOnly) rAggr += countRange(fill as DiffRange);
        else addRightFill(fill);
      }
      if (right) {
        const fill = { startLineNumber: rStart + rAggr, endLineNumber: rEnd + rAggr };
        if (internalRightOnly) lAggr += countRange(fill as DiffRange);
        else addLeftFill(fill);
      }
    } else if (lFillEnd <= rFillEnd) {
      addLeftFill({ startLineNumber: Math.max(lFillEnd + 1, rFillStart), endLineNumber: rFillEnd });
    } else if (rFillEnd < lFillEnd) {
      addRightFill({ startLineNumber: Math.max(rFillEnd + 1, lFillStart), endLineNumber: lFillEnd });
    }
  }

  return { left: leftRanges, right: rightRanges };
}

function findFirstLine(pairs: DiffPairRanges[], side: 'left' | 'right'): number | undefined {
  for (const pair of pairs) {
    const diff = side === 'left' ? pair.left : pair.right;
    if (diff) return diff.startLineNumber;
  }
  return undefined;
}

export function buildDiffPlans(monaco: any, pairs: DiffPair[], leftText: string, rightText: string): { left: DiffPlan; right: DiffPlan } {
  const pairRanges = buildPairRanges(pairs, leftText, rightText);
  const fillRanges = buildFillRanges(pairRanges);
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
