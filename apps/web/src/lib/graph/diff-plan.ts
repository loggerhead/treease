export type DiffChunk = {
  offset: number;
  length: number;
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

function buildLineOffsets(text: string): number[] {
  const offsets = [0];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '\n') offsets.push(i + 1);
  }
  return offsets;
}

function findLineIndex(offsets: number[], offset: number): number {
  let low = 0;
  let high = offsets.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const start = offsets[mid];
    const next = offsets[mid + 1] ?? Number.POSITIVE_INFINITY;
    if (offset >= start && offset < next) return mid;
    if (offset < start) high = mid - 1;
    else low = mid + 1;
  }
  return Math.max(0, offsets.length - 1);
}

function offsetToLineColumn(offsets: number[], offset: number): { line: number; column: number } {
  const lineIndex = findLineIndex(offsets, Math.max(0, offset));
  const lineStart = offsets[lineIndex] ?? 0;
  return { line: lineIndex + 1, column: Math.max(1, offset - lineStart + 1) };
}

function diffToRange(offsets: number[], diff: DiffChunk): DiffRange {
  const start = offsetToLineColumn(offsets, diff.offset);
  const end = offsetToLineColumn(offsets, diff.offset + Math.max(diff.length, 1));
  const isSingleNewline = diff.length === 1 && start.column === 1 && end.column === 1;
  return {
    startLineNumber: start.line,
    startColumn: start.column,
    endLineNumber: isSingleNewline ? start.line : end.line,
    endColumn: isSingleNewline ? start.column : end.column,
    type: diff.type,
    inlineDiffs: diff.inlineDiffs?.map((inline) => diffToRange(offsets, inline)) ?? [],
  };
}

function buildPairRanges(pairs: DiffPair[], leftText: string, rightText: string): DiffPairRanges[] {
  const leftOffsets = buildLineOffsets(leftText);
  const rightOffsets = buildLineOffsets(rightText);
  return pairs.map((pair) => {
    const left = pair.hasLeft ? diffToRange(leftOffsets, pair.left) : undefined;
    const right = pair.hasRight ? diffToRange(rightOffsets, pair.right) : undefined;
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
