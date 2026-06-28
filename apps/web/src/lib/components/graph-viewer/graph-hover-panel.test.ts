import { describe, expect, it, vi } from 'vitest';
import {
  canOpenSubgraphPreviewForCell,
  isGraphHoverTargetOverflowing,
  resolveGraphHoverPreviewRule,
} from './graph-hover-panel';

describe('canOpenSubgraphPreviewForCell', () => {
  it('allows non-empty structured cells from scrollable headerless tables', () => {
    expect(
      canOpenSubgraphPreviewForCell(
        {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          isScrollableTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        'value',
      ),
    ).toBe(true);
  });

  it('rejects structured cells from non-scrollable headerless tables', () => {
    expect(
      canOpenSubgraphPreviewForCell(
        {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          isHeaderlessTable: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        'value',
      ),
    ).toBe(false);
  });
});

describe('resolveGraphHoverPreviewRule', () => {
  const canOpen = vi.fn(() => true);

  it('does not show pre for overflowing scalar value cells anymore', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: 'very long string',
          value: 'very long string',
          valueType: 'string',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('does not show pre for non-overflowing meta path', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '$.meta.short',
          value: '',
          valueType: 'object',
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'meta',
        __graphNodeKind: 'table',
        isOverflow: false,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('does not show subgraph preview for structured table cells anymore', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          path: [],
          editable: false,
          boxArgs: {} as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: false,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('does not show pre for overflowing structured values anymore', () => {
    const result = resolveGraphHoverPreviewRule(
      {
        __graphCell: {
          text: '{1}',
          value: '',
          valueType: 'object',
          isTableCell: true,
          path: [],
          editable: false,
          boxArgs: { width: 8 } as never,
          textArgs: {} as never,
        },
        __graphCellKind: 'value',
        __graphNodeKind: 'table',
        isOverflow: true,
      },
      canOpen,
    );
    expect(result).toBeNull();
  });

  it('only trusts Leafer isOverflow flag', () => {
    expect(isGraphHoverTargetOverflowing({ isOverflow: true } as never)).toBe(true);
    expect(
      isGraphHoverTargetOverflowing({
        width: 120,
        height: 18,
        isOverflow: false,
        getBounds: () => ({ width: 180, height: 18 }),
      } as never),
    ).toBe(false);
  });
});
