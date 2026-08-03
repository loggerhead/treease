<script lang="ts">
  import { ChevronDown, Gauge, Info, LogOut, RefreshCw, Settings, Sparkles, User as UserIcon } from 'lucide-svelte'
  import type { AccountSummary, UsageSummary } from '../services/treease-server'
  import type { SubscriptionPresentation } from '../billing/subscription-presentation'
  import Tooltip from './Tooltip.svelte'
  import {
    DropdownMenuItem,
    DropdownMenuSeparator,
  } from './ui/dropdown-menu'
  import { LARGE_FILE_PROCESSING_INFO } from '../config/large-file'

  type AccountDetails = { name: string; email: string; avatarUrl: string | null; initial: string }

  export let menu = false
  export let signedInUser = false
  export let details: AccountDetails
  export let planPresentation: SubscriptionPresentation | null = null
  export let subscription: AccountSummary['subscription'] | null = null
  export let subscriptionLoading = false
  export let usage: UsageSummary | null = null
  export let usageLoading = false
  export let usageExpanded = true
  export let desktop = false
  export let showSettings = true
  export let managingPlan = false
  export let checkoutBusy = false
  export let onLogin: () => void = () => {}
  export let onLogout: () => Promise<void> = async () => {}
  export let onCheckForUpdates: () => Promise<void> = async () => {}
  export let onOpenSettings: () => void = () => {}
  export let onPlanAction: () => Promise<void> = async () => {}
  export let onRefreshUsage: () => Promise<void> = async () => {}
  export let onCopyAnonymousId: () => Promise<void> = async () => {}

  function monthlyLimit(limit: { kind: 'limited'; limit: number } | { kind: 'unlimited' }, used: number | undefined): string {
    return limit.kind === 'unlimited' ? `${used ?? 0} / ♾️` : `${used ?? 0} / ${limit.limit}`
  }

  function usagePercent(limit: { kind: 'limited'; limit: number } | { kind: 'unlimited' }, used: number | undefined): number | null {
    if (limit.kind === 'unlimited' || limit.limit === 0) return null
    return Math.min(100, Math.round(((used ?? 0) / limit.limit) * 100))
  }

  function resetDate(periodKey: string): string {
    const [year, month] = periodKey.split('-').map(Number)
    const date = new Date(Date.UTC(year, month, 0))
    return new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' }).format(date)
  }

  function subscriptionExpiryDate(value: string | null): string {
    if (!value) return 'Not set'
    const date = new Date(value)
    if (Number.isNaN(date.getTime())) return 'Not set'
    return date.toISOString().slice(0, 10)
  }
</script>

<div class="account-panel" data-testid="account-details">
  {#if signedInUser}
    <div class="account-panel__identity">
      <span class="account-panel-avatar" aria-hidden="true">
        {#if details.avatarUrl}
          <img class="avatar-image" src={details.avatarUrl} alt="" referrerpolicy="no-referrer" />
        {:else}
          <span class="avatar-fallback">{details.initial}</span>
        {/if}
      </span>
      <span class="min-w-0">
        <strong class="block truncate text-[14px] font-semibold text-[#111827]">{details.name}</strong>
        {#if details.email}<span class="mt-0.5 block truncate text-[12px] text-[#64748b]">{details.email}</span>{/if}
      </span>
    </div>
  {:else}
    <div class="account-panel__identity">
      <span class="account-panel-avatar" aria-hidden="true"><span class="avatar-fallback">A</span></span>
      <span class="min-w-0">
        <strong class="block truncate text-[14px] font-semibold text-[#111827]">Anonymous user</strong>
        <button class="account-panel__fingerprint" type="button" title="Copy full ID" aria-label="Copy full ID" data-testid="account-fingerprint-id" on:click={() => void onCopyAnonymousId()}>{details.email}</button>
      </span>
    </div>
  {/if}

  <div class="account-panel__plan" data-testid="account-plan">
    <span>Plan</span>
    {#if planPresentation}
      <strong>{planPresentation.label}{planPresentation.cadence ? ` · ${planPresentation.cadence}` : ''}</strong>
    {:else if subscriptionLoading}
      <span>Syncing…</span>
    {:else if usage}
      <strong>{usage.tier === 'pro' ? 'Pro' : 'Free'}</strong>
    {:else}
      <span>Unavailable</span>
    {/if}
  </div>

  <div class="account-panel__limits" data-testid="account-plan-limits">
    <div class="usage-header">
      <button class="usage-summary" type="button" on:click={() => usageExpanded = !usageExpanded} aria-expanded={usageExpanded} aria-busy={usageLoading}>
        <span class="flex min-w-0 items-center gap-2"><Gauge size={16} /><span>Usage</span></span>
        <span class:usage-chevron-collapsed={!usageExpanded}><ChevronDown size={16} /></span>
      </button>
      <button class:usage-refreshing={usageLoading} class="usage-refresh" type="button" on:click|stopPropagation={() => void onRefreshUsage()} disabled={usageLoading} aria-label="Refresh usage" data-testid="usage-refresh-button">
        <RefreshCw size={14} />
      </button>
    </div>
    {#if usageExpanded && usageLoading}
      <div class="usage-details usage-details--loading" data-testid="usage-loading" role="status" aria-label="Loading usage">
        <div class="usage-loading-line usage-loading-line--wide"></div><div class="usage-loading-line"></div><div class="usage-loading-line"></div>
      </div>
    {:else if usage && usageExpanded}
      <div class="usage-details">
        <div class="usage-cycle">
          {#if subscription?.tier === 'pro'}
            <span>{subscription.status === 'canceled' ? 'Expired at' : 'Next renewal'}</span><span>{subscriptionExpiryDate(subscription.currentPeriodEnd)}</span>
          {:else}
            <span>Monthly allowance</span><span>Resets {resetDate(usage.periodKey)}</span>
          {/if}
        </div>
        <div class="usage-item">
          <div class="usage-item-label"><span>Graph views</span><span>{monthlyLimit(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view)}</span></div>
          {#if usagePercent(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view) !== null}<div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view)}%`}></span></div>{/if}
        </div>
        <div class="usage-item">
          <div class="usage-item-label"><span class="usage-item-label__name"><span>Large files</span><Tooltip content={LARGE_FILE_PROCESSING_INFO} side="right" className="usage-item-info"><span aria-hidden="true"><Info size={12} strokeWidth={2.1} /></span></Tooltip></span><span>{monthlyLimit(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing)}</span></div>
          {#if usagePercent(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing) !== null}<div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing)}%`}></span></div>{/if}
        </div>
        <div class="usage-item" data-testid="usage-ai-suggestions">
          <div class="usage-item-label"><span>AI processing</span><span>{monthlyLimit(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion)}</span></div>
          {#if usagePercent(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion) !== null}<div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion)}%`}></span></div>{/if}
        </div>
      </div>
    {/if}
  </div>

  {#if signedInUser}
    {#if subscription}
      {#if menu}
        <DropdownMenuItem class="rounded-[7px] px-2 py-2 text-[13px]" data-testid="account-manage-plan-menu-item" disabled={managingPlan || checkoutBusy} onSelect={() => void onPlanAction()}><Sparkles size={14} />{subscription.tier === 'free' ? (checkoutBusy ? 'Opening checkout…' : 'Upgrade to Pro') : managingPlan ? 'Opening…' : 'Manage plan'}</DropdownMenuItem>
      {:else}
        <button class="account-panel__action" type="button" data-testid="account-manage-plan-menu-item" disabled={managingPlan || checkoutBusy} on:click={() => void onPlanAction()}><Sparkles size={14} />{subscription.tier === 'free' ? (checkoutBusy ? 'Opening checkout…' : 'Upgrade to Pro') : managingPlan ? 'Opening…' : 'Manage plan'}</button>
      {/if}
    {/if}
    {#if menu}<DropdownMenuSeparator class="my-1" />{:else}<div class="account-panel__separator"></div>{/if}
    {#if menu}
      <DropdownMenuItem variant="destructive" class="rounded-[7px] px-2 py-2 text-[13px]" data-testid="account-logout-menu-item" onSelect={() => void onLogout()}><LogOut size={14} />Log out</DropdownMenuItem>
    {:else}
      <button class="account-panel__action account-panel__action--danger" type="button" data-testid="account-logout-menu-item" on:click={() => void onLogout()}><LogOut size={14} />Log out</button>
    {/if}
  {:else if menu}
    <DropdownMenuItem data-testid="account-login-menu-item" onSelect={onLogin}><UserIcon size={14} />Login</DropdownMenuItem>
  {:else}
    <button class="account-panel__action" type="button" data-testid="account-login-menu-item" on:click={onLogin}><UserIcon size={14} />Login</button>
  {/if}

  {#if showSettings}
    {#if menu}
      <DropdownMenuSeparator class="my-1" />
      {#if desktop}<DropdownMenuItem class="rounded-[7px] text-[13px]" data-testid="account-check-updates-menu-item" onSelect={() => void onCheckForUpdates()}><RefreshCw size={14} />Check for updates</DropdownMenuItem>{/if}
      <DropdownMenuItem class="rounded-[7px] text-[13px]" data-testid="account-settings-menu-item" onSelect={onOpenSettings}><Settings size={14} />Settings</DropdownMenuItem>
    {:else}
      <div class="account-panel__separator"></div>
      {#if desktop}<button class="account-panel__action" type="button" data-testid="account-check-updates-menu-item" on:click={() => void onCheckForUpdates()}><RefreshCw size={14} />Check for updates</button>{/if}
      <button class="account-panel__action" type="button" data-testid="account-settings-menu-item" on:click={onOpenSettings}><Settings size={14} />Settings</button>
    {/if}
  {/if}
</div>

<style>
  .account-panel { min-width: 0; color: var(--text-primary); }
  .account-panel__identity { display: flex; align-items: center; gap: 12px; padding: 8px; }
  .account-panel-avatar { display: inline-grid; width: 38px; height: 38px; flex: 0 0 auto; overflow: hidden; border-radius: 999px; }
  .avatar-image, .avatar-fallback { width: 100%; height: 100%; border-radius: inherit; }
  .avatar-image { display: block; object-fit: cover; }
  .avatar-fallback { display: grid; place-items: center; color: #1d4ed8; background: #dbeafe; font-size: 15px; font-weight: 700; }
  .account-panel__fingerprint { display: block; max-width: 100%; overflow: hidden; border: 0; padding: 0; color: #64748b; background: transparent; font-size: 12px; text-align: left; text-decoration: underline; text-decoration-style: dotted; text-underline-offset: 2px; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
  .account-panel__plan { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 0 2px 8px; border-radius: 7px; padding: 8px; color: #64748b; background: #f1f5f9; font-size: 12px; }
  .account-panel__plan strong { color: #0f172a; }
  .account-panel__limits { margin: 0 2px 8px; }
  .account-panel__action { display: flex; width: 100%; min-height: 32px; align-items: center; gap: 8px; border: 0; border-radius: 7px; padding: 7px 8px; color: var(--text-primary); background: transparent; font-size: 13px; text-align: left; cursor: pointer; }
  .account-panel__action:hover { background: var(--panel-bg-alt); }
  .account-panel__action:disabled { cursor: wait; opacity: .65; }
  .account-panel__action--danger { color: var(--danger); }
  .account-panel__separator { height: 1px; margin: 4px 0; background: var(--border-muted); }
  .usage-summary { display: flex; width: 100%; align-items: center; justify-content: space-between; gap: 8px; border: 0; border-radius: 7px; background: #f1f5f9; padding: 7px 8px; color: #0f172a; font-size: 12px; font-weight: 600; text-align: left; cursor: pointer; }
  .usage-header { display: flex; align-items: stretch; gap: 4px; }
  .usage-refresh { display: inline-grid; width: 30px; flex: 0 0 auto; place-items: center; border: 0; border-radius: 7px; background: #f1f5f9; color: #64748b; cursor: pointer; }
  .usage-refresh:hover:not(:disabled) { background: #e8eef5; color: #2563eb; }
  .usage-refresh:disabled { cursor: wait; opacity: .7; }
  .usage-refreshing :global(svg) { animation: usage-spin 700ms linear infinite; }
  .usage-summary:hover { background: #e8eef5; }
  .usage-chevron-collapsed { transform: rotate(-90deg); }
  .usage-details { padding: 8px 8px 5px; }
  .usage-details--loading { display: grid; gap: 8px; }
  .usage-loading-line { width: 76%; height: 10px; border-radius: 999px; background: linear-gradient(90deg, #e2e8f0 25%, #f8fafc 50%, #e2e8f0 75%); background-size: 200% 100%; animation: usage-shimmer 1.2s ease-in-out infinite; }
  .usage-loading-line--wide { width: 100%; height: 8px; }
  .usage-cycle, .usage-item-label { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .usage-cycle { margin-bottom: 7px; color: #64748b; font-size: 11px; }
  .usage-item { margin-bottom: 7px; }
  .usage-item-label { color: #475569; font-size: 12px; }
  .usage-item-label > span:last-child { flex: 0 0 auto; color: #64748b; }
  .usage-item-label__name { display: inline-flex; min-width: 0; align-items: center; gap: 4px; }
  :global(.usage-item-info) { display: inline-flex; flex: 0 0 auto; color: #94a3b8; cursor: help; opacity: .85; }
  .usage-progress { height: 4px; margin-top: 4px; overflow: hidden; border-radius: 999px; background: #e2e8f0; }
  .usage-progress span { display: block; height: 100%; border-radius: inherit; background: #2563eb; }
  @keyframes usage-spin { to { transform: rotate(360deg); } }
  @keyframes usage-shimmer { to { background-position: -200% 0; } }
</style>
