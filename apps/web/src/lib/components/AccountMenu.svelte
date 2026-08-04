<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { ChevronDown, Gauge, Info, LogOut, RefreshCw, Settings, Sparkles, User as UserIcon } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { trackEvent } from '../analytics/ga4';
  import { authUser, authUserDetails, observeAuthUser } from '../auth/auth-user-store';
  import { presentSubscription } from '../billing/subscription-presentation';
  import {
    createBillingPortalLink,
    getAccountSummary,
    getUsageSummary,
    type AccountSummary,
    type UsageSummary,
  } from '../services/treease-server';
  import { getUsageClientId } from '../billing/client-id';
  import { applyLocalUsage } from '../billing/entitlement-gate';
  import { runBillingCheckout } from '../billing/checkout-flow';
  import { LARGE_FILE_PROCESSING_INFO } from '../config/large-file';
  import Tooltip from './Tooltip.svelte';
  import AccountPanel from './AccountPanel.svelte';
  import ContextItem from './ContextItem.svelte';
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
  } from './ui/dropdown-menu';

  export let variant: 'landing' | 'editor' = 'editor';
  export let onLogin: () => void = () => {};
  export let onLogout: () => Promise<void> = async () => {};
  export let onCheckForUpdates: () => Promise<void> = async () => {};
  export let onOpenSettings: () => void = () => {};
  export let showTriggerTitle = true;
  export let showProfileTrigger = false;
  export let contextPanel = false;

  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  let account: AccountSummary['user'] | null = null;
  let accountUserId: string | null = null;
  let anonymousClientId: string | null = null;
  let subscription: AccountSummary['subscription'] | null = null;
  let subscriptionLoading = false;
  let usage: UsageSummary | null = null;
  let usageLoading = false;
  let accountRequest = 0;
  let usageRequest = 0;
  let managingPlan = false;
  let checkoutBusy = false;
  let accountMenuOpen = false;
  let subscriptionViewTrackedForOpen = false;
  let usageExpanded = true;

  onMount(() => {
    const stopAuthObserver = observeAuthUser();
    void loadAccount(null);
    return stopAuthObserver;
  });
  onDestroy(() => {
    accountRequest += 1;
    usageRequest += 1;
  });

  $: signedInUser = $authUser;
  $: details = $authUser ? {
    ...authUserDetails($authUser),
    email: signedInUser && account?.id === signedInUser.id ? account.email ?? '' : authUserDetails($authUser).email,
    avatarUrl: signedInUser && account?.id === signedInUser.id ? account.avatarUrl : authUserDetails($authUser).avatarUrl,
  } : {
    name: 'Anonymous user',
    email: anonymousClientId ? formatAnonymousId(anonymousClientId) : 'ID: loading…',
    avatarUrl: null,
    initial: 'A',
  };
  $: if (signedInUser?.id !== accountUserId) void loadAccount(signedInUser?.id ?? null);
  $: planPresentation = subscription ? presentSubscription(subscription) : null;
  $: if (!accountMenuOpen) subscriptionViewTrackedForOpen = false;
  $: if (accountMenuOpen && subscription && !subscriptionViewTrackedForOpen) {
    subscriptionViewTrackedForOpen = true;
    trackSubscriptionViewed();
  }

  async function loadAccount(userId: string | null): Promise<void> {
    accountUserId = userId;
    account = null;
    subscription = null;
    usage = null;
    subscriptionLoading = userId !== null;
    usageLoading = true;
    const request = ++accountRequest;
    usageRequest += 1;
    if (!userId && variant === 'landing') {
      usageLoading = false;
      return;
    }

    try {
      if (!userId) {
        const clientId = await getUsageClientId();
        if (request !== accountRequest || accountUserId !== userId) return;
        anonymousClientId = clientId;
        usage = await applyLocalUsage(await getUsageSummary(clientId));
        return;
      }
      const nextAccount = await getAccountSummary();
      if (request !== accountRequest || accountUserId !== userId) return;
      account = nextAccount.user;
      subscription = nextAccount.subscription;
      usage = await applyLocalUsage(nextAccount.usage);
    } catch {
      if (request !== accountRequest || accountUserId !== userId) return;
      toast.error('Account information is temporarily unavailable. Please try again later.');
    } finally {
      if (request === accountRequest && accountUserId === userId) {
        subscriptionLoading = false;
        usageLoading = false;
      }
    }
  }

  async function refreshUsage(): Promise<void> {
    const userId = accountUserId;
    if (usageLoading) return;

    usageLoading = true;
    const request = ++usageRequest;
    try {
      const clientId = userId ? undefined : anonymousClientId ?? await getUsageClientId();
      if (!userId && clientId) anonymousClientId = clientId;
      const nextUsage = await applyLocalUsage(await getUsageSummary(clientId));
      if (request !== usageRequest || accountUserId !== userId) return;
      usage = nextUsage;
    } catch {
      if (request !== usageRequest || accountUserId !== userId) return;
      toast.error('Usage information is temporarily unavailable. Please try again later.');
    } finally {
      if (request === usageRequest && accountUserId === userId) usageLoading = false;
    }
  }

  async function managePlan(): Promise<void> {
    if (!subscription || managingPlan) return;
    managingPlan = true;
    trackEvent('subscription_management_started', {
      plan: subscription.tier,
      status: subscription.status,
      surface: `account_menu_${variant}`,
    });

    try {
      const { url } = await createBillingPortalLink();
      window.open(url, '_blank', 'noopener,noreferrer');
    } catch {
      toast.error('Unable to open plan management. Please try again later.');
      managingPlan = false;
    }
  }

  async function upgradeToPro(): Promise<void> {
    if (!subscription || subscription.tier !== 'free' || checkoutBusy) return;

    checkoutBusy = true;

    try {
      await runBillingCheckout({
        priceId: 'monthly',
        returnUrl: { successUrl: new URL('/editor', window.location.origin).toString() },
        onLoginRequired: onLogin,
        onFailed: (message) => toast.error(message),
      });
    } finally {
      checkoutBusy = false;
    }
  }

  async function handlePlanAction(): Promise<void> {
    if (!subscription) return;
    if (subscription.tier === 'free') {
      await upgradeToPro();
      return;
    }
    await managePlan();
  }

  function trackSubscriptionViewed(): void {
    if (!subscription) return;
    trackEvent('subscription_viewed', {
      plan: subscription.tier,
      status: subscription.status,
      surface: `account_menu_${variant}`,
    });
  }

  function monthlyLimit(limit: { kind: 'limited'; limit: number } | { kind: 'unlimited' }, used: number | undefined): string {
    return limit.kind === 'unlimited' ? `${used ?? 0} / ♾️` : `${used ?? 0} / ${limit.limit}`;
  }

  function usagePercent(limit: { kind: 'limited'; limit: number } | { kind: 'unlimited' }, used: number | undefined): number | null {
    if (limit.kind === 'unlimited' || limit.limit === 0) return null;
    return Math.min(100, Math.round(((used ?? 0) / limit.limit) * 100));
  }

  function resetDate(periodKey: string): string {
    const [year, month] = periodKey.split('-').map(Number);
    const date = new Date(Date.UTC(year, month, 0));
    return new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' }).format(date);
  }

  function subscriptionExpiryDate(value: string | null): string {
    if (!value) return 'Not set';
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return 'Not set';
    return date.toISOString().slice(0, 10);
  }

  function formatAnonymousId(clientId: string): string {
    const fingerprintId = clientId.includes(':') ? clientId.slice(clientId.indexOf(':') + 1) : clientId;
    return `ID: ${fingerprintId.slice(0, 8)}`;
  }

  async function copyAnonymousId(): Promise<void> {
    if (!anonymousClientId) return;
    try {
      await navigator.clipboard.writeText(anonymousClientId);
      toast.success('ID copied.');
    } catch {
      toast.error('Unable to copy ID.');
    }
  }
</script>

{#if variant === 'landing' && !signedInUser}
  <button class="landing-login" type="button" data-testid="account-login-button" on:click={onLogin}>Login</button>
{:else if contextPanel}
  <ContextItem
    label="Account"
    ariaLabel={details ? `Account for ${details.name}` : 'Account'}
    tooltip="Account"
    expanded={showProfileTrigger}
    placement="right-end"
    bind:open={accountMenuOpen}
    customTrigger
    triggerClass={showProfileTrigger ? 'account-profile-trigger' : 'account-compact-trigger'}
    testId={signedInUser ? 'account-avatar-button' : 'account-menu-button'}
  >
    <span slot="trigger" class="account-trigger-content">
      {#if signedInUser}
        <span class="avatar-frame">
          {#if details.avatarUrl}
            <img class="avatar-image" src={details.avatarUrl} alt="" referrerpolicy="no-referrer" />
          {:else}
            <span class="avatar-fallback" aria-hidden="true">{details.initial}</span>
          {/if}
        </span>
        {#if showProfileTrigger}
          <span class="account-trigger-copy"><strong>{details.name}</strong>{#if details.email}<span>{details.email}</span>{/if}</span>
        {/if}
      {:else if showProfileTrigger}
        <span class="avatar-frame avatar-frame--anonymous"><UserIcon size={18} /></span>
        <span class="account-trigger-copy"><strong>Anonymous user</strong><span>{details.email}</span></span>
      {:else}
        <UserIcon size={12} />
      {/if}
    </span>
    <div slot="panel" class="account-context-panel">
      <AccountPanel
        signedInUser={Boolean(signedInUser)}
        {details}
        {planPresentation}
        {subscription}
        {subscriptionLoading}
        {usage}
        {usageLoading}
        bind:usageExpanded
        {desktop}
        showSettings={false}
        {managingPlan}
        {checkoutBusy}
        {onLogin}
        {onLogout}
        {onCheckForUpdates}
        {onOpenSettings}
        onPlanAction={handlePlanAction}
        onRefreshUsage={refreshUsage}
        onCopyAnonymousId={copyAnonymousId}
      />
    </div>
  </ContextItem>
{:else}
  <div class:editor-account-anchor={variant === 'editor'} class:editor-account-anchor--profile={variant === 'editor' && showProfileTrigger}>
  <DropdownMenu bind:open={accountMenuOpen}>
    <DropdownMenuTrigger
      class={variant === 'landing'
        ? 'relative inline-grid h-9 w-9 cursor-pointer place-items-center rounded-full border border-slate-900/10 bg-white shadow-[0_8px_20px_rgba(15,23,42,0.08)] outline-none transition-[background-color,border-color,box-shadow] duration-150 hover:border-slate-900/20 hover:bg-slate-100 hover:shadow-[0_5px_14px_rgba(15,23,42,0.12)] data-[state=open]:border-slate-900/20 data-[state=open]:bg-slate-100 data-[state=open]:shadow-[0_5px_14px_rgba(15,23,42,0.12)] focus-visible:ring-2 focus-visible:ring-blue-600/30 focus-visible:ring-offset-2'
        : showProfileTrigger
          ? 'account-profile-trigger'
          : 'relative inline-grid h-6 w-6 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-[var(--text-primary)] outline-none transition-[background-color,box-shadow] duration-150 hover:bg-[var(--panel-bg-alt)] data-[state=open]:bg-[var(--panel-bg-alt)] focus-visible:ring-2 focus-visible:ring-blue-600/30'}
      aria-label={details ? `Account for ${details.name}` : 'Account'}
      title={showTriggerTitle ? (details ? details.name : 'Account') : undefined}
      data-testid={signedInUser ? 'account-avatar-button' : 'account-menu-button'}
    >
      {#if signedInUser}
        <span class="avatar-frame">
          {#if details.avatarUrl}
            <img class="avatar-image" src={details.avatarUrl} alt="" referrerpolicy="no-referrer" />
          {:else}
            <span class="avatar-fallback" aria-hidden="true">{details.initial}</span>
          {/if}
        </span>
        {#if showProfileTrigger}
          <span class="account-trigger-copy">
            <strong>{details.name}</strong>
            {#if details.email}<span>{details.email}</span>{/if}
          </span>
        {/if}
        {#if planPresentation && subscription?.tier === 'pro'}
          <span class="plan-badge pro-plan-badge" aria-hidden="true"></span>
        {/if}
      {:else if showProfileTrigger}
        <span class="avatar-frame avatar-frame--anonymous"><UserIcon size={18} /></span>
        <span class="account-trigger-copy">
          <strong>Anonymous user</strong>
          <span>{details.email}</span>
        </span>
      {:else}
        <UserIcon size={12} />
      {/if}
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" sideOffset={8} class="w-[240px] rounded-[10px] p-2 shadow-[0_14px_38px_rgba(15,23,42,0.16)]">
      {#if signedInUser}
        <div class="flex items-center gap-3 px-2 py-2" data-testid="account-details">
          <span class="account-panel-avatar" aria-hidden="true">
            {#if details.avatarUrl}
              <img class="avatar-image" src={details.avatarUrl} alt="" referrerpolicy="no-referrer" />
            {:else}
              <span class="avatar-fallback">{details.initial}</span>
            {/if}
          </span>
          <span class="min-w-0">
            <strong class="block truncate text-[14px] font-semibold text-[#111827]">{details.name}</strong>
            {#if details.email}
              <span class="mt-0.5 block truncate text-[12px] text-[#64748b]">{details.email}</span>
            {/if}
          </span>
        </div>
      {:else}
        <div class="flex items-center gap-3 px-2 py-2" data-testid="account-details">
          <span class="account-panel-avatar" aria-hidden="true"><span class="avatar-fallback">A</span></span>
          <span class="min-w-0">
            <strong class="block truncate text-[14px] font-semibold text-[#111827]">Anonymous user</strong>
            <button
              class="mt-0.5 block max-w-full truncate border-0 bg-transparent p-0 text-left text-[12px] text-[#64748b] underline decoration-dotted underline-offset-2 hover:text-[#2563eb]"
              type="button"
              title="Copy full ID"
              aria-label="Copy full ID"
              data-testid="account-fingerprint-id"
              on:click={() => void copyAnonymousId()}
            >{details.email}</button>
          </span>
        </div>
      {/if}
        <div class="mx-2 mb-1 flex items-center justify-between rounded-[7px] bg-[#f1f5f9] px-2 py-1.5 text-[12px]" data-testid="account-plan">
          <span class="text-[#64748b]">Plan</span>
          {#if planPresentation}
            <strong class="text-[#0f172a]">{planPresentation.label}{planPresentation.cadence ? ` · ${planPresentation.cadence}` : ''}</strong>
          {:else if subscriptionLoading}
            <span class="text-[#64748b]">Syncing…</span>
          {:else if usage}
            <strong class="text-[#0f172a]">{usage.tier === 'pro' ? 'Pro' : 'Free'}</strong>
          {:else}
            <span class="text-[#64748b]">Unavailable</span>
          {/if}
        </div>
        <div class="mx-2 mb-1" data-testid="account-plan-limits">
          <div class="usage-header">
            <button class="usage-summary" type="button" on:click={() => usageExpanded = !usageExpanded} aria-expanded={usageExpanded} aria-busy={usageLoading}>
              <span class="flex min-w-0 items-center gap-2"><Gauge size={16} /><span>Usage</span></span>
              <span class:usage-chevron-collapsed={!usageExpanded}><ChevronDown size={16} /></span>
            </button>
            <button class:usage-refreshing={usageLoading} class="usage-refresh" type="button" on:click|stopPropagation={() => void refreshUsage()} disabled={usageLoading} aria-label="Refresh usage" data-testid="usage-refresh-button">
              <RefreshCw size={14} />
            </button>
          </div>
          {#if usageExpanded && usageLoading}
            <div class="usage-details usage-details--loading" data-testid="usage-loading" role="status" aria-label="Loading usage">
              <div class="usage-loading-line usage-loading-line--wide"></div>
              <div class="usage-loading-line"></div>
              <div class="usage-loading-line"></div>
            </div>
          {:else if usage && usageExpanded}
            <div class="usage-details">
              <div class="usage-cycle">
                {#if subscription?.tier === 'pro'}
                  <span>{subscription.status === 'canceled' ? 'Expired at' : 'Next renewal'}</span>
                  <span>{subscriptionExpiryDate(subscription.currentPeriodEnd)}</span>
                {:else}
                  <span>Monthly allowance</span><span>Resets {resetDate(usage.periodKey)}</span>
                {/if}
              </div>
              <div class="usage-item">
                <div class="usage-item-label"><span>Graph views</span><span>{monthlyLimit(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view)}</span></div>
                {#if usagePercent(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view) !== null}
                  <div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.graphViewDocumentsMonthly, usage.usage.graph_view)}%`}></span></div>
                {/if}
              </div>
              <div class="usage-item">
                <div class="usage-item-label">
                  <span class="usage-item-label__name">
                    <span>Large files</span>
                    <Tooltip content={LARGE_FILE_PROCESSING_INFO} side="right" className="usage-item-info">
                      <span aria-hidden="true"><Info size={12} strokeWidth={2.1} /></span>
                    </Tooltip>
                  </span>
                  <span>{monthlyLimit(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing)}</span>
                </div>
                {#if usagePercent(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing) !== null}
                  <div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.largeFileProcessingRunsMonthly, usage.usage.large_file_processing)}%`}></span></div>
                {/if}
              </div>
              <div class="usage-item" data-testid="usage-ai-suggestions">
                <div class="usage-item-label"><span>AI processing</span><span>{monthlyLimit(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion)}</span></div>
                {#if usagePercent(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion) !== null}
                  <div class="usage-progress"><span style={`width: ${usagePercent(usage.limits.aiProcessingMonthly, usage.usage.ai_suggestion)}%`}></span></div>
                {/if}
              </div>
            </div>
          {/if}
        </div>
        {#if signedInUser}
          {#if subscription}
          <DropdownMenuItem
            class="rounded-[7px] px-2 py-2 text-[13px]"
            data-testid="account-manage-plan-menu-item"
            disabled={managingPlan || checkoutBusy}
            onSelect={() => void handlePlanAction()}
          >
            <Sparkles size={14} />
            {subscription.tier === 'free' ? (checkoutBusy ? 'Opening checkout…' : 'Upgrade to Pro') : managingPlan ? 'Opening…' : 'Manage plan'}
          </DropdownMenuItem>
          {/if}
          <DropdownMenuSeparator class="my-1" />
          <DropdownMenuItem
            variant="destructive"
            class="rounded-[7px] px-2 py-2 text-[13px]"
            data-testid="account-logout-menu-item"
            onSelect={() => void onLogout()}
          >
            <LogOut size={14} />Log out
          </DropdownMenuItem>
        {:else}
        <DropdownMenuItem data-testid="account-login-menu-item" onSelect={onLogin}>
          {#if variant === 'editor'}
            <UserIcon size={14} />
          {/if}
          Login
        </DropdownMenuItem>
        {/if}
        {#if variant === 'editor'}
          <DropdownMenuSeparator class="my-1" />
          {#if desktop}
            <DropdownMenuItem class="rounded-[7px] text-[13px]" data-testid="account-check-updates-menu-item" onSelect={() => void onCheckForUpdates()}>
              <RefreshCw size={14} />Check for updates
            </DropdownMenuItem>
          {/if}
          <DropdownMenuItem class="rounded-[7px] text-[13px]" data-testid="account-settings-menu-item" onSelect={onOpenSettings}>
            <Settings size={14} />Settings
          </DropdownMenuItem>
        {/if}
    </DropdownMenuContent>
  </DropdownMenu>
  </div>
{/if}

<style>
  .landing-login {
    display: inline-flex;
    align-items: center;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: color 160ms ease;
  }

  .landing-login:hover {
    color: var(--accent-strong);
  }

  .editor-account-anchor {
    position: relative;
    display: inline-flex;
    width: 24px;
    height: 24px;
    flex: 0 0 24px;
    align-items: center;
    justify-content: center;
    overflow: visible;
  }

  .editor-account-anchor--profile {
    width: 100%;
    height: 58px;
    flex: 0 0 58px;
    align-items: stretch;
  }

  :global(.account-profile-trigger) {
    display: flex;
    width: 100%;
    min-width: 0;
    height: 58px;
    box-sizing: border-box;
    align-items: center;
    gap: 10px;
    overflow: hidden;
    border: 0;
    border-radius: 8px;
    padding: 6px 4px;
    color: var(--text-primary);
    background: transparent;
    text-align: left;
    outline: none;
    transition: background-color 150ms ease;
  }

  :global(.account-profile-trigger:hover),
  :global(.account-profile-trigger[data-state='open']) {
    background: var(--panel-bg-alt);
  }

  :global(.account-profile-trigger:focus-visible) {
    box-shadow: 0 0 0 2px rgb(37 99 235 / 30%);
  }

  :global(.account-profile-trigger .avatar-frame) {
    width: 32px;
    height: 32px;
    flex: 0 0 32px;
    border-radius: 50%;
  }

  :global(.account-trigger-content) {
    display: flex;
    width: 100%;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }

  :global(.account-compact-trigger) {
    width: 32px;
    height: 32px;
    justify-content: center;
  }

  .avatar-frame--anonymous {
    display: grid;
    place-items: center;
    color: var(--text-muted);
    background: var(--panel-bg-alt);
  }

  .account-trigger-copy {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    overflow: hidden;
  }

  .account-trigger-copy strong,
  .account-trigger-copy span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .account-trigger-copy strong {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 650;
    line-height: 1.2;
  }

  .account-trigger-copy span {
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.2;
  }

  .avatar-image,
  .avatar-fallback {
    width: 100%;
    height: 100%;
    border-radius: inherit;
  }

  .avatar-frame {
    display: inline-grid;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: inherit;
  }

  .plan-badge {
    position: absolute;
    top: auto;
    right: -2px;
    bottom: 1px;
    z-index: 1;
    width: 8px;
    height: 8px;
    min-width: 8px;
    padding: 0;
    border: 1px solid #ffffff;
    border-radius: 999px;
    background: #64748b;
    color: #ffffff;
    box-shadow: 0 1px 4px rgba(15, 23, 42, 0.2);
  }

  .plan-badge.pro-plan-badge {
    background: #2563eb;
  }

  .avatar-image {
    display: block;
    object-fit: cover;
  }

  .avatar-fallback {
    display: grid;
    place-items: center;
    background: #dbeafe;
    color: #1d4ed8;
    font-size: 12px;
    font-weight: 700;
  }

  .account-panel-avatar {
    display: inline-grid;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    overflow: hidden;
    border-radius: 999px;
  }

  .usage-summary {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border: 0;
    border-radius: 7px;
    background: #f1f5f9;
    padding: 7px 8px;
    color: #0f172a;
    font-size: 12px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  .usage-header {
    display: flex;
    align-items: stretch;
    gap: 4px;
  }

  .usage-refresh {
    display: inline-grid;
    width: 30px;
    flex: 0 0 auto;
    place-items: center;
    border: 0;
    border-radius: 7px;
    background: #f1f5f9;
    color: #64748b;
    cursor: pointer;
  }

  .usage-refresh:hover:not(:disabled) {
    background: #e8eef5;
    color: #2563eb;
  }

  .usage-refresh:disabled {
    cursor: wait;
    opacity: 0.7;
  }

  .usage-refreshing :global(svg) {
    animation: usage-spin 700ms linear infinite;
  }

  .usage-summary:hover {
    background: #e8eef5;
  }

  .usage-chevron-collapsed {
    transform: rotate(-90deg);
  }

  .usage-details--loading {
    display: grid;
    gap: 8px;
  }

  .usage-loading-line {
    width: 76%;
    height: 10px;
    border-radius: 999px;
    background: linear-gradient(90deg, #e2e8f0 25%, #f8fafc 50%, #e2e8f0 75%);
    background-size: 200% 100%;
    animation: usage-shimmer 1.2s ease-in-out infinite;
  }

  .usage-loading-line--wide {
    width: 100%;
    height: 8px;
  }

  @keyframes usage-spin {
    to { transform: rotate(360deg); }
  }

  @keyframes usage-shimmer {
    to { background-position: -200% 0; }
  }

  .usage-details {
    padding: 8px 8px 5px;
  }

  .usage-cycle,
  .usage-item-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .usage-cycle {
    margin-bottom: 7px;
    color: #64748b;
    font-size: 11px;
  }

  .usage-item {
    margin-bottom: 7px;
  }

  .usage-item-label {
    color: #475569;
    font-size: 12px;
  }

  .usage-item-label > span:last-child {
    flex: 0 0 auto;
    color: #64748b;
  }

  .usage-item-label__name {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 4px;
  }

  :global(.usage-item-info) {
    display: inline-flex;
    flex: 0 0 auto;
    color: #94a3b8;
    cursor: help;
    opacity: .85;
    transition: color 140ms ease, opacity 140ms ease;
  }

  :global(.usage-item-info:hover) {
    color: #475569;
    opacity: 1;
  }

  .usage-progress {
    height: 4px;
    margin-top: 4px;
    overflow: hidden;
    border-radius: 999px;
    background: #e2e8f0;
  }

  .usage-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: #2563eb;
  }

</style>
