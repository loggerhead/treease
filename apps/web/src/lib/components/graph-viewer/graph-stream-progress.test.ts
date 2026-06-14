import { afterEach, describe, expect, it, vi } from 'vitest';
import { createGraphStreamProgressController } from './graph-stream-progress';

describe('graph-stream-progress', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows after a delay and hides after completion delay', () => {
    vi.useFakeTimers();
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'streaming',
      processedBytes: 50,
      totalBytes: 100,
      value: 45,
      final: false,
    });

    expect(controller.getSnapshot().visible).toBe(false);
    vi.advanceTimersByTime(151);
    expect(controller.getSnapshot().visible).toBe(true);

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 2,
      phase: 'done',
      processedBytes: 100,
      totalBytes: 100,
      value: 100,
      final: true,
    });

    expect(controller.getSnapshot().visible).toBe(true);
    vi.advanceTimersByTime(251);
    expect(controller.getSnapshot().visible).toBe(false);
  });

  it('does not reset show timer on subsequent events', () => {
    vi.useFakeTimers();
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'start',
      processedBytes: 0,
      totalBytes: 100,
      value: 5,
      final: false,
    });

    for (let i = 2; i <= 10; i++) {
      controller.handleEvent({
        event: 'graphProgress',
        documentKey: 'cache-key',
        streamRunId: 'stream-id',
        eventSeq: i,
        phase: 'streaming',
        processedBytes: i * 10,
        totalBytes: 100,
        value: 5 + i * 8,
        final: false,
      });
    }

    expect(controller.getSnapshot().visible).toBe(false);
    vi.advanceTimersByTime(151);
    expect(controller.getSnapshot().visible).toBe(true);
  });
  it('ignores stale out-of-order events for the active stream', () => {
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 8,
      phase: 'streaming',
      processedBytes: 80,
      totalBytes: 100,
      value: 8,
      final: false,
    });

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 7,
      phase: 'streaming',
      processedBytes: 70,
      totalBytes: 100,
      value: 7,
      final: false,
    });

    expect(controller.getSnapshot()).toMatchObject({
      streamRunId: 'stream-id',
      phase: 'streaming',
      value: 8,
    });
  });

  it('does not let a superseded stream reclaim progress ownership', () => {
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-a',
      eventSeq: 8,
      phase: 'streaming',
      processedBytes: 80,
      totalBytes: 100,
      value: 8,
      final: false,
    });

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-b',
      eventSeq: 7,
      phase: 'streaming',
      processedBytes: 70,
      totalBytes: 100,
      value: 7,
      final: false,
    });

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-a',
      eventSeq: 9,
      phase: 'streaming',
      processedBytes: 90,
      totalBytes: 100,
      value: 9,
      final: false,
    });

    expect(controller.getSnapshot()).toMatchObject({
      streamRunId: 'stream-b',
      phase: 'streaming',
      value: 7,
    });
  });


  it('resets immediately on failed progress event', () => {
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'failed',
      processedBytes: 10,
      totalBytes: 100,
      value: 0,
      final: true,
    });

    expect(controller.getSnapshot()).toMatchObject({
      visible: false,
      streamRunId: '',
      phase: 'idle',
      value: 0,
    });
  });

  it('completeIfActive forces done state and schedules hide', () => {
    vi.useFakeTimers();
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'streaming',
      processedBytes: 50,
      totalBytes: 100,
      value: 45,
      final: false,
    });

    vi.advanceTimersByTime(151);
    expect(controller.getSnapshot().visible).toBe(true);

    controller.completeIfActive();

    expect(controller.getSnapshot()).toMatchObject({
      visible: true,
      phase: 'done',
      value: 100,
      label: 'Graph ready',
    });

    vi.advanceTimersByTime(251);
    expect(controller.getSnapshot().visible).toBe(false);
  });

  it('completeIfActive is no-op when already idle or done', () => {
    vi.useFakeTimers();
    const controller = createGraphStreamProgressController();

    controller.completeIfActive();
    expect(controller.getSnapshot().phase).toBe('idle');

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'done',
      processedBytes: 100,
      totalBytes: 100,
      value: 100,
      final: true,
    });

    controller.completeIfActive();
    expect(controller.getSnapshot().phase).toBe('idle');
  });

  it('completeIfActive hides immediately if not yet visible', () => {
    vi.useFakeTimers();
    const controller = createGraphStreamProgressController();

    controller.handleEvent({
      event: 'graphProgress',
      documentKey: 'cache-key',
      streamRunId: 'stream-id',
      eventSeq: 1,
      phase: 'streaming',
      processedBytes: 50,
      totalBytes: 100,
      value: 45,
      final: false,
    });

    expect(controller.getSnapshot().visible).toBe(false);
    controller.completeIfActive();
    expect(controller.getSnapshot().phase).toBe('idle');
  });
});
