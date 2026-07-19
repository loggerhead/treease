import { trackEvent } from '../analytics/ga4';
import {
  getUsageSummary,
  recordUsageEvent,
  type RecordedUsageCapability,
  type UsageSummary,
} from '../services/treease-server';

type GateSurface = 'graph_edit' | 'file_import';

export type UsageBlock = {
  capability: RecordedUsageCapability;
  limit: number;
  tier: UsageSummary['tier'];
};

let latestUsage: UsageSummary | null = null;
let usageRequest = 0;

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
  return { capability, limit: limit.limit, tier: summary.tier };
}

function featureFor(capability: RecordedUsageCapability): string {
  return capability === 'bidirectional_edit' ? 'bidirectional_edit' : 'large_file_processing';
}

function reportThreshold(summary: UsageSummary, capability: RecordedUsageCapability, surface: GateSurface): void {
  const limit = limitFor(summary, capability);
  if (limit.kind !== 'limited' || limit.limit === 0) return;
  const used = summary.usage[capability] ?? 0;
  const threshold = used >= limit.limit ? 100 : used / limit.limit >= 0.8 ? 80 : 0;
  if (threshold) trackEvent('quota_threshold_reached', { plan: summary.tier, feature: featureFor(capability), threshold, surface });
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
  const request = ++usageRequest;
  try {
    const summary = await getUsageSummary();
    if (request === usageRequest) latestUsage = summary;
  } catch {
    if (request === usageRequest) latestUsage = null;
  }
  return capability ? usageBlockFor(latestUsage, capability) : null;
}

export async function runPostpaidCapability<T>(input: {
  capability: RecordedUsageCapability;
  createIdempotencyKey: () => Promise<string>;
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

  const request = ++usageRequest;
  void (async () => {
    try {
      const summary = await recordUsageEvent({
        capability: input.capability,
        idempotencyKey: await input.createIdempotencyKey(),
        metadata: input.metadata,
      });
      if (!summary) return;
      if (request !== usageRequest) return;
      latestUsage = summary;
      reportThreshold(summary, input.capability, input.surface);
    } catch {
      // Usage telemetry must never delay or roll back a local graph result.
    }
  })();
  return result;
}
