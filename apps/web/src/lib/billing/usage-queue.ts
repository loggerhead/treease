import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import {
  claimUsageEvents,
  recordUsageEvent,
  type RecordedUsageCapability,
} from '../services/treease-server';
import { getUsageClientId } from './client-id';

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
const inFlight = new Set<string>();

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
  const db = await getDb();
  const events = await db.getAllFromIndex('events', 'by-status', 'pending');
  const clientIds = [...new Set(events.map((event) => event.clientId))];
  for (const clientId of clientIds) {
    try {
      await claimUsageEvents(clientId);
    } catch {
      // A failed claim must not prevent the event queue from retrying later.
    }
  }
  await Promise.all(events.map(async (event) => {
    if (inFlight.has(event.eventId)) return;
    inFlight.add(event.eventId);
    try {
      await recordUsageEvent({
        clientId: event.clientId,
        capability: event.capability,
        idempotencyKey: event.eventId,
        metadata: { ...event.metadata, createdAt: event.createdAt },
      });
      await db.put('events', { ...event, status: 'uploaded' });
    } catch {
      // Keep the event pending; the next flush retries the same idempotency key.
    } finally {
      inFlight.delete(event.eventId);
    }
  }));
}

if (typeof window !== 'undefined') {
  window.setInterval(() => void flushUsageEvents(), 60_000);
}
