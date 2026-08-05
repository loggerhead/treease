type ColumnRange = {
  left: number;
  width: number;
};

type ExpansionScrollInput = {
  transition: 'expand';
  scrollLeft: number;
  maxScrollLeft: number;
  visibleWidth: number;
  activeColumn: ColumnRange;
  previewColumn: ColumnRange;
};

type CollapseScrollInput = {
  transition: 'collapse';
  scrollLeft: number;
  visibleWidth: number;
  activeColumn: ColumnRange;
};

export type PreviewScrollSessionInput = (ExpansionScrollInput | CollapseScrollInput) & {
  activeDepth: number;
};

export type ColumnPreviewScrollPlan = {
  scrollLeft: number;
};

export function planColumnPreviewScroll(
  input: ExpansionScrollInput | CollapseScrollInput,
): ColumnPreviewScrollPlan {
  if (input.transition === 'collapse') {
    const activeVisibleWidth = Math.min(input.activeColumn.width, input.visibleWidth / 2);
    const targetScrollLeft = input.activeColumn.left + input.activeColumn.width - activeVisibleWidth;
    return {
      scrollLeft: Math.max(0, Math.min(input.scrollLeft, targetScrollLeft)),
    };
  }
  const previewWidth = Math.min(input.previewColumn.width, input.visibleWidth / 2);
  const targetScrollLeft = input.previewColumn.left + previewWidth - input.visibleWidth;
  const activeVisibleLimit = input.activeColumn.left + input.activeColumn.width - 1;
  if (input.scrollLeft > activeVisibleLimit) return { scrollLeft: input.scrollLeft };
  return {
    scrollLeft: Math.min(
      input.maxScrollLeft,
      activeVisibleLimit,
      Math.max(input.scrollLeft, targetScrollLeft),
    ),
  };
}

export function createColumnPreviewScrollSession() {
  let activeDepth: number | null = null;
  let consumed = false;

  return {
    reset(): void {
      activeDepth = null;
      consumed = false;
    },
    plan(input: PreviewScrollSessionInput): ColumnPreviewScrollPlan | null {
      if (input.activeDepth !== activeDepth) {
        activeDepth = input.activeDepth;
        consumed = false;
      }
      if (consumed) return null;
      consumed = true;
      return planColumnPreviewScroll(input);
    },
  };
}
