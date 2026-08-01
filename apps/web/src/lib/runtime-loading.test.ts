import { describe, expect, it } from 'vitest';
import {
  computeSynchronizedRuntimeLoading,
  resolveEditorRuntimeOverlay,
  shouldShowGraphRuntimeLoading,
} from './runtime-loading';

describe('runtime-loading', () => {
  it('keeps the shared loading gate active until editor and graph are both ready in graph mode', () => {
    expect(
      computeSynchronizedRuntimeLoading({
        viewMode: 'graph',
        editorRuntimeLoading: true,
        graphRuntimeLoading: false,
      }),
    ).toBe(true);

    expect(
      computeSynchronizedRuntimeLoading({
        viewMode: 'graph',
        editorRuntimeLoading: false,
        graphRuntimeLoading: true,
      }),
    ).toBe(true);

    expect(
      computeSynchronizedRuntimeLoading({
        viewMode: 'graph',
        editorRuntimeLoading: false,
        graphRuntimeLoading: false,
      }),
    ).toBe(false);
  });

  it('does not keep waiting for graph runtime after the viewer switches to text mode', () => {
    expect(
      computeSynchronizedRuntimeLoading({
        viewMode: 'text',
        editorRuntimeLoading: false,
        graphRuntimeLoading: true,
      }),
    ).toBe(false);
  });

  it('does not gate the editor overlay on graph runtime loading', () => {
    expect(
      resolveEditorRuntimeOverlay({
        editorRuntimeReady: true,
        editorRuntimePhase: '',
        synchronizedRuntimeLoading: true,
      }),
    ).toEqual({ loading: false, phase: '' });
  });

  it('turns a runtime failure into a visible settled state', () => {
    expect(
      resolveEditorRuntimeOverlay({
        editorRuntimeReady: false,
        editorRuntimeError: true,
        editorRuntimePhase: 'Editor failed to load',
      }),
    ).toEqual({ loading: false, phase: 'Editor failed to load' });
  });

  it('lets graph loading stay visible while a peer runtime is still blocking the shared gate', () => {
    expect(
      shouldShowGraphRuntimeLoading({
        graphRuntimeReady: true,
        synchronizedRuntimeLoading: true,
        errorMessage: '',
      }),
    ).toBe(true);
  });

  it('shows graph errors instead of the loading skeleton', () => {
    expect(
      shouldShowGraphRuntimeLoading({
        graphRuntimeReady: false,
        synchronizedRuntimeLoading: true,
        errorMessage: 'Leafer failed',
      }),
    ).toBe(false);
  });
});
