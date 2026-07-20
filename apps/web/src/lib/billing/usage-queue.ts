import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import {
  claimUsageEvents,
  recordUsageEvent,
  TreeaseServerError,
  type RecordedUsageCapability,
} from '../services/treease-server';
import { getUsageClientId } from './client-id';
import { isUsageCoolingDown } from './usage-rate-limit';

type UsageQueueEvent = {
  eventId: string;
  clientId: string;
  capability: RecordedUsageCapability;
  metadata: Record<string, unknown>;
  createdAt: string;
  status: 'pending' | 'uploaded';
};

interface UsageDatabase extends DBSchema {
  events: {
    key: string;
    value: UsageQueueEvent;
    indexes: { 'by-status': string };
  };
}

const DB_NAME = 'treease-usage';
const DB_VERSION = 1;
let flushPromise: Promise<void> | null = null;

async function getDb(): Promise<IDBPDatabase<UsageDatabase>> {
  return openDB<UsageDatabase>(DB_NAME, DB_VERSION, {
    upgrade(db) {
      const store = db.createObjectStore('events', { keyPath: 'eventId' });
      store.createIndex('by-status', 'status');
    },
  });
}

export async function enqueueUsageEvent(input: {
  capability: RecordedUsageCapability;
  metadata: Record<string, unknown>;
}): Promise<void> {
  const clientId = await getUsageClientId();
  const db = await getDb();
  await db.put('events', {
    eventId: crypto.randomUUID(),
    clientId,
    capability: input.capability,
    metadata: input.metadata,
    createdAt: new Date().toISOString(),
    status: 'pending',
  });
  void flushUsageEvents();
}

export async function flushUsageEvents(): Promise<void> {
  // Timer and enqueue triggers share one promise so a pending batch is never flushed twice.
  if (flushPromise) return flushPromise;
  flushPromise = flushUsageEventsInternal().finally(() => {
    flushPromise = null;
  });
  return flushPromise;
}

async function flushUsageEventsInternal(): Promise<void> {
  if (isUsageCoolingDown()) return;
  const db = await getDb();
  const events = await db.getAllFromIndex('events', 'by-status', 'pending');
  const clientIds = [...new Set(events.map((event) => event.clientId))];
  for (const clientId of clientIds) {
    if (isUsageCoolingDown()) return;
    try {
      await claimUsageEvents(clientId);
    } catch (error) {
      if (error instanceof TreeaseServerError && error.status === 429) return;
      // A failed claim must not prevent the event queue from retrying later.
    }
  }
  for (const event of events) {
    if (isUsageCoolingDown()) return;
    try {
      await recordUsageEvent({
        clientId: event.clientId,
        capability: event.capability,
        idempotencyKey: event.eventId,
        metadata: { ...event.metadata, createdAt: event.createdAt },
      });
      await db.put('events', { ...event, status: 'uploaded' });
    } catch (error) {
      if (error instanceof TreeaseServerError && error.status === 429) return;
      // Keep the event pending; the next flush retries the same idempotency key.
    }
  }
}

if (typeof window !== 'undefined') {
  window.setInterval(() => void flushUsageEvents(), 60_000);
}
