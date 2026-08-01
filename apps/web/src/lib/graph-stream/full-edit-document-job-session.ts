import type { EventBatch } from '@core-wasm/index';
import { getSharedWasmWorkerClient } from '../wasm/wasm-worker-singleton';
import {
  runTextDocumentJobForGraph,
  runReadableDocumentJobForGraph,
  type DocumentJobGraphResult,
  type ReadableGraphDocumentJobInput,
} from './document-job-runner';

export type FullEditDocumentJobSessionInput = Omit<ReadableGraphDocumentJobInput, 'readable'> & {
  readable?: ReadableGraphDocumentJobInput['readable'];
  text?: string;
  sessionId: string;
  revision: number;
  totalBytes?: number;
  streamRunId?: string;
};

export type FullEditDocumentJobSession = {
  sessionId: string;
  documentKey: string;
  language: string;
  revision: number;
  totalBytes: number;
  chunkSize?: number;
  streamRunId: string;
  jobHandle: number | null;
  result: Promise<DocumentJobGraphResult>;
  /** True only while a subscriber can replay every batch emitted by this job. */
  hasCompleteReplay: () => boolean;
  batches: () => AsyncIterable<EventBatch>;
  cancel: () => Promise<void>;
};

export type FullEditDocumentJobSessionRef = {
  sessionId: string;
  documentKey?: string;
  language?: string;
  revision?: number;
};

const fullEditDocumentJobSessions = new Map<string, FullEditDocumentJobSession>();
const MAX_REPLAY_BATCHES = 8;

export function getFullEditDocumentJobSession(
  ref: string | FullEditDocumentJobSessionRef | null | undefined,
): FullEditDocumentJobSession | null {
  if (!ref) return null;
  const sessionId = typeof ref === 'string' ? ref : ref.sessionId;
  if (!sessionId) return null;
  const session = fullEditDocumentJobSessions.get(sessionId) ?? null;
  if (!session || typeof ref === 'string') return session;
  if (ref.documentKey != null && session.documentKey !== ref.documentKey) return null;
  if (ref.language != null && session.language !== ref.language) return null;
  if (ref.revision != null && session.revision !== ref.revision) return null;
  return session;
}

export function clearFullEditDocumentJobSession(
  sessionId: string | null | undefined,
  expected?: FullEditDocumentJobSession | null,
): void {
  if (!sessionId) return;
  const current = fullEditDocumentJobSessions.get(sessionId);
  if (!current) return;
  if (expected && current !== expected) return;
  fullEditDocumentJobSessions.delete(sessionId);
}

class ReadableDocumentJobSession implements FullEditDocumentJobSession {
  readonly sessionId: string;
  readonly documentKey: string;
  readonly language: string;
  readonly revision: number;
  readonly totalBytes: number;
  readonly chunkSize?: number;
  readonly streamRunId: string;
  readonly result: Promise<DocumentJobGraphResult>;

  private readonly replayWindow: EventBatch[] = [];
  private replayStartIndex = 0;
  private readonly waiters = new Set<() => void>();
  private closed = false;
  private failure: unknown = null;
  private currentJobHandle: number | null = null;
  private cancelRequested = false;
  private cancelPromise: Promise<void> | null = null;

  constructor(input: FullEditDocumentJobSessionInput) {
    this.sessionId = input.sessionId;
    this.documentKey = input.documentKey;
    this.language = input.language;
    this.revision = input.revision;
    this.totalBytes = Math.max(0, Math.trunc(input.totalBytes ?? 0));
    this.chunkSize = input.chunkSize;
    this.streamRunId = input.streamRunId ?? input.sessionId;
    this.result = this.run(input);
  }

  get jobHandle(): number | null {
    return this.currentJobHandle;
  }

  hasCompleteReplay(): boolean {
    return this.replayStartIndex === 0;
  }

  async *batches(): AsyncIterable<EventBatch> {
    let index = this.replayStartIndex;
    while (true) {
      if (index < this.replayStartIndex) {
        index = this.replayStartIndex;
      }
      const replayEndIndex = this.replayStartIndex + this.replayWindow.length;
      if (index < replayEndIndex) {
        yield this.replayWindow[index - this.replayStartIndex];
        index += 1;
        continue;
      }
      if (this.closed) {
        if (this.failure) throw this.failure;
        return;
      }
      await new Promise<void>((resolve) => {
        const waiter = () => {
          this.waiters.delete(waiter);
          resolve();
        };
        this.waiters.add(waiter);
      });
    }
  }

  async cancel(): Promise<void> {
    this.cancelRequested = true;
    if (this.cancelPromise) return this.cancelPromise;
    const jobHandle = this.currentJobHandle;
    if (jobHandle == null) {
      this.close();
      return Promise.resolve();
    }
    this.cancelPromise = getSharedWasmWorkerClient()
      .then((client) => client.call<EventBatch>('cancelDocumentJob', { jobHandle }))
      .catch(() => undefined)
      .then(() => {
        if (this.currentJobHandle === jobHandle) this.currentJobHandle = null;
        this.close();
      });
    return this.cancelPromise;
  }

  private pushBatch(batch: EventBatch): void {
    this.replayWindow.push(batch);
    if (this.replayWindow.length > MAX_REPLAY_BATCHES) {
      this.replayWindow.shift();
      this.replayStartIndex += 1;
    }
    this.notify();
  }

  private notify(): void {
    for (const waiter of Array.from(this.waiters)) waiter();
  }

  private close(failure?: unknown): void {
    if (failure) this.failure = failure;
    if (this.closed) return;
    this.closed = true;
    this.notify();
  }

  private async run(input: FullEditDocumentJobSessionInput): Promise<DocumentJobGraphResult> {
    try {
      const hooks = {
        onJobHandle: (jobHandle) => {
          this.currentJobHandle = jobHandle;
          if (this.cancelRequested) void this.cancel();
        },
        onBatch: (batch) => {
          this.pushBatch(batch);
        },
      };
      const result = input.text != null
        ? await runTextDocumentJobForGraph({ ...input, text: input.text }, hooks)
        : input.readable != null
          ? await runReadableDocumentJobForGraph({ ...input, readable: input.readable }, hooks)
          : (() => {
              throw new Error('Full edit document job session requires text or readable input');
            })();
      if (this.currentJobHandle === result.jobHandle) this.currentJobHandle = null;
      this.close();
      return result;
    } catch (error) {
      this.close(error);
      throw error;
    }
  }
}

export function startFullEditDocumentJobSessionForGraph(
  input: FullEditDocumentJobSessionInput,
): FullEditDocumentJobSession {
  const previous = fullEditDocumentJobSessions.get(input.sessionId);
  if (previous) void previous.cancel();
  const session = new ReadableDocumentJobSession(input);
  fullEditDocumentJobSessions.set(input.sessionId, session);
  return session;
}
