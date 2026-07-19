import { trackEvent } from '../analytics/ga4';
import {
  getUsageSummary,
  type RecordedUsageCapability,
  type UsageSummary,
} from '../services/treease-server';
import { getUsageClientId } from './client-id';
import { enqueueUsageEvent } from './usage-queue';

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
    const summary = await getUsageSummary(await getUsageClientId());
    if (request === usageRequest) latestUsage = summary;
  } catch {
    if (request === usageRequest) latestUsage = null;
  }
  return capability ? usageBlockFor(latestUsage, capability) : null;
}

export async function runPostpaidCapability<T>(input: {
  capability: RecordedUsageCapability;
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
        metadata: input.metadata,
      });
      void refreshUsageGate();
    } catch {
      // Usage telemetry must never delay or roll back a local graph result.
    }
  })();
  return result;
}
