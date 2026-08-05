import { describe, expect, it } from 'vitest';
import { createColumnPreviewScrollSession, planColumnPreviewScroll } from './preview-scroll';

describe('Column Navigator preview scroll', () => {
  it('plans the first expansion leftward while keeping the active column visible', () => {
    const plan = planColumnPreviewScroll({
      transition: 'expand',
      scrollLeft: 0,
      maxScrollLeft: 600,
      visibleWidth: 400,
      activeColumn: { left: 288, width: 288 },
      previewColumn: { left: 576, width: 288 },
    });

    expect(plan).toEqual({ scrollLeft: 376 });
  });

  it('does not plan another scroll for later preview changes at the same level', () => {
    const session = createColumnPreviewScrollSession();
    const expansion = {
      transition: 'expand' as const,
      activeDepth: 2,
      scrollLeft: 0,
      maxScrollLeft: 600,
      visibleWidth: 400,
      activeColumn: { left: 288, width: 288 },
      previewColumn: { left: 576, width: 288 },
    };

    expect(session.plan(expansion)).toEqual({ scrollLeft: 376 });
    expect(session.plan({
      transition: 'collapse',
      activeDepth: 2,
      scrollLeft: 376,
      visibleWidth: 400,
      activeColumn: { left: 288, width: 288 },
    })).toBeNull();
    expect(session.plan(expansion)).toBeNull();
  });

  it('restores one opportunity after a level change and plans collapse rightward', () => {
    const session = createColumnPreviewScrollSession();
    session.plan({
      transition: 'expand',
      activeDepth: 3,
      scrollLeft: 376,
      maxScrollLeft: 800,
      visibleWidth: 400,
      activeColumn: { left: 576, width: 288 },
      previewColumn: { left: 864, width: 288 },
    });

    expect(session.plan({
      transition: 'collapse',
      activeDepth: 2,
      scrollLeft: 500,
      visibleWidth: 400,
      activeColumn: { left: 288, width: 288 },
    })).toEqual({ scrollLeft: 376 });
  });

  it('reduces expansion scroll when the preview goal would hide the active column', () => {
    expect(planColumnPreviewScroll({
      transition: 'expand',
      scrollLeft: 0,
      maxScrollLeft: 800,
      visibleWidth: 200,
      activeColumn: { left: 288, width: 288 },
      previewColumn: { left: 900, width: 288 },
    })).toEqual({ scrollLeft: 575 });
  });

  it('uses the complete child width when it is smaller than half the visible width', () => {
    expect(planColumnPreviewScroll({
      transition: 'expand',
      scrollLeft: 0,
      maxScrollLeft: 600,
      visibleWidth: 600,
      activeColumn: { left: 288, width: 288 },
      previewColumn: { left: 576, width: 120 },
    })).toEqual({ scrollLeft: 96 });
  });

  it('uses the complete parent width when it is smaller than half the visible width', () => {
    expect(planColumnPreviewScroll({
      transition: 'collapse',
      scrollLeft: 500,
      visibleWidth: 600,
      activeColumn: { left: 288, width: 120 },
    })).toEqual({ scrollLeft: 288 });
  });

  it('consumes the level opportunity when the preview goal needs no movement', () => {
    const session = createColumnPreviewScrollSession();
    const expansion = {
      transition: 'expand' as const,
      activeDepth: 1,
      scrollLeft: 0,
      maxScrollLeft: 600,
      visibleWidth: 800,
      activeColumn: { left: 0, width: 288 },
      previewColumn: { left: 288, width: 288 },
    };

    expect(session.plan(expansion)).toEqual({ scrollLeft: 0 });
    expect(session.plan({ ...expansion, visibleWidth: 400 })).toBeNull();
  });

  it('does not reverse direction when an expansion cannot preserve active-column visibility', () => {
    expect(planColumnPreviewScroll({
      transition: 'expand',
      scrollLeft: 650,
      maxScrollLeft: 800,
      visibleWidth: 200,
      activeColumn: { left: 288, width: 288 },
      previewColumn: { left: 900, width: 288 },
    })).toEqual({ scrollLeft: 650 });
  });

  it('starts a new opportunity after the navigator workspace resets', () => {
    const session = createColumnPreviewScrollSession();
    const expansion = {
      transition: 'expand' as const,
      activeDepth: 1,
      scrollLeft: 0,
      maxScrollLeft: 600,
      visibleWidth: 400,
      activeColumn: { left: 0, width: 288 },
      previewColumn: { left: 288, width: 288 },
    };

    session.plan(expansion);
    session.reset();

    expect(session.plan(expansion)).toEqual({ scrollLeft: 88 });
  });
});
