import { trackEvent } from '../analytics/ga4';
import {
  getUsageSummary,
  type RecordedUsageCapability,
  type UsageSummary,
} from '../services/treease-server';
import { getUsageClientId } from './client-id';
import { enqueueUsageEvent, getPendingUsageDelta } from './usage-queue';
import { isUsageCoolingDown } from './usage-rate-limit';

type GateSurface = 'graph_edit' | 'file_import';

export type UsageBlock = {
  capability: RecordedUsageCapability;
  used: number;
  limit: number;
  tier: UsageSummary['tier'];
};

let latestUsage: UsageSummary | null = null;
let usageRequest = 0;

export async function applyLocalUsage(summary: UsageSummary): Promise<UsageSummary> {
  const delta = await getPendingUsageDelta();
  const usage = { ...summary.usage };
  for (const [capability, quantity] of Object.entries(delta)) {
    if (quantity) usage[capability as RecordedUsageCapability] = (usage[capability as RecordedUsageCapability] ?? 0) + quantity;
  }
  return { ...summary, usage };
}

function limitFor(summary: UsageSummary, capability: RecordedUsageCapability) {
  return capability === 'bidirectional_edit'
    ? summary.limits.bidirectionalEditDocumentsMonthly
    : summary.limits.largeFileProcessingRunsMonthly;
}

export function usageBlockFor(
  summary: UsageSummary | null,
  capability: RecordedUsageCapability,
): UsageBlock | null {
  if (!summary) return null;
  const limit = limitFor(summary, capability);
  if (limit.kind !== 'limited' || (summary.usage[capability] ?? 0) < limit.limit) return null;
  return { capability, used: summary.usage[capability] ?? 0, limit: limit.limit, tier: summary.tier };
}

function featureFor(capability: RecordedUsageCapability): string {
  return capability === 'bidirectional_edit' ? 'bidirectional_edit' : 'large_file_processing';
}

function reportBlocked(block: UsageBlock, surface: GateSurface): void {
  trackEvent('entitlement_blocked', {
    plan: block.tier,
    feature: featureFor(block.capability),
    reason: 'quota_exhausted',
    surface,
  });
}

export async function refreshUsageGate(capability?: RecordedUsageCapability): Promise<UsageBlock | null> {
  if (isUsageCoolingDown()) return capability ? usageBlockFor(latestUsage, capability) : null;
  const request = ++usageRequest;
  try {
    const summary = await applyLocalUsage(await getUsageSummary(await getUsageClientId()));
    if (request === usageRequest) latestUsage = summary;
  } catch {
    if (request === usageRequest && !isUsageCoolingDown()) latestUsage = null;
  }
  return capability ? usageBlockFor(latestUsage, capability) : null;
}

export async function runPostpaidCapability<T>(input: {
  capability: RecordedUsageCapability;
  idempotencyKey: string;
  quantity?: number;
  metadata: Record<string, unknown>;
  surface: GateSurface;
  execute: () => Promise<T>;
  onBlocked: (block: UsageBlock) => void;
}): Promise<T> {
  const block = usageBlockFor(latestUsage, input.capability);
  const result = await input.execute();
  if (block) {
    reportBlocked(block, input.surface);
    input.onBlocked(block);
    return result;
  }

  void (async () => {
    try {
      await enqueueUsageEvent({
        capability: input.capability,
        idempotencyKey: input.idempotencyKey,
        quantity: input.quantity,
        metadata: input.metadata,
      });
      if (latestUsage) latestUsage = await applyLocalUsage(latestUsage);
      void refreshUsageGate();
    } catch {
      // Usage telemetry must never delay or roll back a local graph result.
    }
  })();
  return result;
}
