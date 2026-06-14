import type { EventBatch } from '@core-wasm/index';
import { getSharedWasmWorkerClient } from '../wasm/wasm-worker-singleton';
import {
  runReadableDocumentJobForGraph,
  type DocumentJobGraphResult,
  type ReadableGraphDocumentJobInput,
} from './document-job-runner';

export type ReadableFullEditDocumentJobSessionInput = ReadableGraphDocumentJobInput & {
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

  private readonly history: EventBatch[] = [];
  private readonly waiters = new Set<() => void>();
  private closed = false;
  private failure: unknown = null;
  private currentJobHandle: number | null = null;

  constructor(input: ReadableFullEditDocumentJobSessionInput) {
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

  async *batches(): AsyncIterable<EventBatch> {
    let index = 0;
    while (true) {
      while (index < this.history.length) {
        yield this.history[index];
        index += 1;
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
    const jobHandle = this.currentJobHandle;
    if (jobHandle == null) {
      this.close();
      return;
    }
    try {
      const client = await getSharedWasmWorkerClient();
      await client.call<EventBatch>('cancelDocumentJob', { jobHandle });
    } catch {
      // Best-effort cancellation for abandoned full-edit sessions.
    } finally {
      if (this.currentJobHandle === jobHandle) this.currentJobHandle = null;
      this.close();
    }
  }

  private pushBatch(batch: EventBatch): void {
    this.history.push(batch);
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

  private async run(input: ReadableFullEditDocumentJobSessionInput): Promise<DocumentJobGraphResult> {
    try {
      const result = await runReadableDocumentJobForGraph(input, {
        onJobHandle: (jobHandle) => {
          this.currentJobHandle = jobHandle;
        },
        onBatch: (batch) => {
          this.pushBatch(batch);
        },
      });
      if (this.currentJobHandle === result.jobHandle) this.currentJobHandle = null;
      this.close();
      return result;
    } catch (error) {
      this.close(error);
      throw error;
    }
  }
}

export function startReadableDocumentJobSessionForGraph(
  input: ReadableFullEditDocumentJobSessionInput,
): FullEditDocumentJobSession {
  const previous = fullEditDocumentJobSessions.get(input.sessionId);
  if (previous) void previous.cancel();
  const session = new ReadableDocumentJobSession(input);
  fullEditDocumentJobSessions.set(input.sessionId, session);
  return session;
}
