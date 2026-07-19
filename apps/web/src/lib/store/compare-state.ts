import { writable } from 'svelte/store';

export type CompareState =
  | { kind: 'none' }
  | { kind: 'equal'; mode: 'tree' | 'text' }
  | { kind: 'different'; mode: 'tree' | 'text' };

type CompareOutcome = {
  equal: boolean;
  mode: 'tree' | 'text';
};

export const initialCompareState: CompareState = { kind: 'none' };

export const compareState = writable<CompareState>(initialCompareState);

export function setCompareOutcome(outcome: CompareOutcome): void {
  compareState.set(outcome.equal ? { kind: 'equal', mode: outcome.mode } : { kind: 'different', mode: outcome.mode });
}

export function clearCompareState(): void {
  compareState.set(initialCompareState);
}
