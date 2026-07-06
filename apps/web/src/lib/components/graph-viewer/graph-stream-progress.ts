// 职责：GraphViewer stream 进度条 store：phase/delta/progress 可订阅状态
import { writable, type Readable } from 'svelte/store';
import type {
  BuildGraphProgressEvent,
  GraphStreamProgressEvent,
  StreamProgressPhase,
} from '../../../shared/worker-protocol/protocol';

export type GraphStreamProgressState = {
  visible: boolean;
  streamRunId: string;
  label: string;
  detail: string;
  value: number;
  phase: StreamProgressPhase | 'idle';
  startedAt: number | null;
  completedAt: number | null;
};

const SHOW_DELAY_MS = 150;
const HIDE_DELAY_MS = 250;
const MAX_SUPERSEDED_STREAMS = 32;

function createIdleState(): GraphStreamProgressState {
  return {
    visible: false,
    streamRunId: '',
    label: '',
    detail: '',
    value: 0,
    phase: 'idle',
    startedAt: null,
    completedAt: null,
  };
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function buildLabel(phase: StreamProgressPhase): string {
  if (phase === 'start') return 'Preparing graph';
  if (phase === 'streaming') return 'Building nodes and edges';
  if (phase === 'flushing' || phase === 'finishing') return 'Finalizing graph';
  if (phase === 'done') return 'Graph ready';
  return '';
}

type AnyProgressEvent = BuildGraphProgressEvent | GraphStreamProgressEvent;


function buildDetail(event: AnyProgressEvent): string {
  if (event.phase === 'streaming') {
    return `Processed ${formatBytes(event.processedBytes)} / ${formatBytes(event.totalBytes)}`;
  }
  if (event.phase === 'flushing') return 'Merging trailing updates';
  if (event.phase === 'finishing') return 'Applying final result';
  if (event.phase === 'done') return 'Graph view is ready';
  return '';
}

export type GraphStreamProgressController = Readable<GraphStreamProgressState> & {
  handleEvent: (event: AnyProgressEvent) => boolean;
  reset: () => void;
  completeIfActive: () => void;
  dispose: () => void;
  getSnapshot: () => GraphStreamProgressState;
};

export function createGraphStreamProgressController(): GraphStreamProgressController {
  const store = writable<GraphStreamProgressState>(createIdleState());
  let state = createIdleState();
  let showTimer: ReturnType<typeof setTimeout> | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let completionTimer: ReturnType<typeof setTimeout> | null = null;
  let lastEventSeq: number | null = null;
  const supersededStreamRunIds = new Set<string>();

  const setState = (nextState: GraphStreamProgressState) => {
    state = nextState;
    store.set(nextState);
  };

  const rememberSupersededStreamRunId = (streamRunId: string) => {
    if (!streamRunId) return;
    if (supersededStreamRunIds.has(streamRunId)) {
      supersededStreamRunIds.delete(streamRunId);
    }
    supersededStreamRunIds.add(streamRunId);
    if (supersededStreamRunIds.size <= MAX_SUPERSEDED_STREAMS) return;
    const oldestStreamRunId = supersededStreamRunIds.values().next().value;
    if (oldestStreamRunId) supersededStreamRunIds.delete(oldestStreamRunId);
  };

  const clearShowTimer = () => {
    if (!showTimer) return;
    clearTimeout(showTimer);
    showTimer = null;
  };

  const clearHideTimer = () => {
    if (!hideTimer) return;
    clearTimeout(hideTimer);
    hideTimer = null;
  };

  const clearCompletionTimer = () => {
    if (!completionTimer) return;
    clearTimeout(completionTimer);
    completionTimer = null;
  };

  const transitionToIdle = (streamRunId: string) => {
    clearShowTimer();
    clearHideTimer();
    clearCompletionTimer();
    lastEventSeq = null;
    rememberSupersededStreamRunId(streamRunId || state.streamRunId);
    setState(createIdleState());
  };

  const reset = () => {
    transitionToIdle(state.streamRunId);
  };

  const scheduleShow = (streamRunId: string) => {
    if (state.visible) return;
    if (showTimer) return;
    showTimer = setTimeout(() => {
      showTimer = null;
      if (state.streamRunId !== streamRunId || state.phase === 'idle' || state.phase === 'done' || state.phase === 'failed') {
        return;
      }
      setState({ ...state, visible: true });
    }, SHOW_DELAY_MS);
  };

  const scheduleHide = (streamRunId: string) => {
    clearHideTimer();
    hideTimer = setTimeout(() => {
      hideTimer = null;
      if (state.streamRunId !== streamRunId) return;
      transitionToIdle(streamRunId);
    }, HIDE_DELAY_MS);
  };

  const finalizeCompletion = (streamRunId: string) => {
    clearShowTimer();
    const doneState: GraphStreamProgressState = {
      ...state,
      visible: state.visible,
      label: 'Graph ready',
      detail: 'Graph view is ready',
      value: 100,
      phase: 'done',
      completedAt: Date.now(),
    };
    setState(doneState);
    if (state.visible) {
      scheduleHide(streamRunId);
    } else {
      transitionToIdle(streamRunId);
    }
  };

  const scheduleCompletionAfterFinishing = (streamRunId: string) => {
    if (completionTimer) return;
    completionTimer = setTimeout(() => {
      completionTimer = null;
      if (state.streamRunId !== streamRunId || state.phase !== 'finishing') return;
      finalizeCompletion(streamRunId);
    }, 16);
  };

  const handleEvent = (event: AnyProgressEvent): boolean => {
    const streamRunId = String(event.streamRunId ?? '');
    if (!streamRunId) return false;
    if (streamRunId !== state.streamRunId && supersededStreamRunIds.has(streamRunId)) {
      return false;
    }

    const eventSeq = Number.isFinite(event.eventSeq) ? event.eventSeq : null;
    if (state.streamRunId === streamRunId && eventSeq != null && lastEventSeq != null && eventSeq < lastEventSeq) {
      return false;
    }

    if (state.streamRunId !== streamRunId) {
      rememberSupersededStreamRunId(state.streamRunId);
      clearShowTimer();
      clearCompletionTimer();
      lastEventSeq = null;
    }
    if (eventSeq != null) {
      lastEventSeq = eventSeq;
    }

    const nextState: GraphStreamProgressState = {
      visible: state.visible,
      streamRunId,
      label: buildLabel(event.phase),
      detail: buildDetail(event),
      value: event.value,
      phase: event.phase,
      startedAt: state.streamRunId === streamRunId ? state.startedAt : Date.now(),
      completedAt: event.phase === 'done' ? Date.now() : null,
    };

    if (event.phase === 'failed') {
      rememberSupersededStreamRunId(streamRunId);
      reset();
      return true;
    }

    clearHideTimer();
    setState(nextState);

    if (event.phase === 'done') {
      clearShowTimer();
      if (state.visible) {
        setState({ ...nextState, visible: true });
        scheduleHide(streamRunId);
        return true;
      }
      transitionToIdle(streamRunId);
      return true;
    }

    scheduleShow(streamRunId);
    return true;
  };

  const completeIfActive = () => {
    if (state.phase === 'idle' || state.phase === 'done' || state.phase === 'failed') return;
    if (state.phase === 'finishing') {
      scheduleCompletionAfterFinishing(state.streamRunId);
      return;
    }
    clearCompletionTimer();
    finalizeCompletion(state.streamRunId);
  };

  return {
    subscribe: store.subscribe,
    handleEvent,
    reset,
    completeIfActive,
    dispose: reset,
    getSnapshot: () => state,
  };
}
