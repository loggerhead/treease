import { describe, expect, it } from 'vitest';
import { get } from 'svelte/store';

import { clearCompareState, compareState, setCompareOutcome } from './compare-state';

describe('compare state', () => {
  it('retains a successful compare with no differences', () => {
    setCompareOutcome({ equal: true, mode: 'tree' });

    expect(get(compareState)).toEqual({ kind: 'equal', mode: 'tree' });
  });

  it('retains a successful compare with differences', () => {
    setCompareOutcome({ equal: false, mode: 'text' });

    expect(get(compareState)).toEqual({ kind: 'different', mode: 'text' });
  });

  it('clears the compare judgement', () => {
    setCompareOutcome({ equal: false, mode: 'tree' });

    clearCompareState();

    expect(get(compareState)).toEqual({ kind: 'none' });
  });
});
