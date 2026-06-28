import { describe, expect, it } from 'vitest';
import { createSidecarExternalSync } from './sidecar-external-sync';

describe('createSidecarExternalSync', () => {
  it('keeps the Monaco draft as owner while focused or dirty', () => {
    const sync = createSidecarExternalSync('"A"');

    sync.focus();
    sync.recordLocalText('"AB"');

    expect(sync.shouldApplyExternalText('"A"', '"AB"')).toBe(false);
    expect(sync.snapshot()).toMatchObject({
      acceptedText: '"A"',
      dirty: true,
      focused: true,
      pendingText: '"A"',
    });

    sync.blur();
    expect(sync.shouldApplyExternalText('"A"', '"AB"')).toBe(false);

    expect(sync.shouldApplyExternalText('"AB"', '"AB"')).toBe(false);
    expect(sync.snapshot()).toMatchObject({
      acceptedText: '"AB"',
      dirty: false,
      focused: false,
      pendingText: null,
    });
  });

  it('allows external text to initialize or replace only when the local draft is clean', () => {
    const sync = createSidecarExternalSync('"A"');

    expect(sync.shouldApplyExternalText('"B"', '"A"')).toBe(true);
    sync.acceptExternalText('"B"');

    expect(sync.snapshot()).toMatchObject({
      acceptedText: '"B"',
      dirty: false,
      pendingText: null,
    });
  });
});
