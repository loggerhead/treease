<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { LogOut, RefreshCw, Settings, User as UserIcon } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { trackEvent } from '../analytics/ga4';
  import { authUser, authUserDetails, observeAuthUser } from '../auth/auth-user-store';
  import { presentSubscription } from '../billing/subscription-presentation';
  import {
    createBillingPortalLink,
    getCurrentSubscription,
    type CurrentSubscription,
  } from '../services/treease-server';
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

  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  let subscription: CurrentSubscription | null = null;
  let subscriptionUserId: string | null = null;
  let subscriptionLoading = false;
  let subscriptionRequest = 0;
  let managingPlan = false;
  let accountMenuOpen = false;
  let subscriptionViewTrackedForOpen = false;

  onMount(observeAuthUser);
  onDestroy(() => {
    subscriptionRequest += 1;
  });

  $: details = $authUser ? authUserDetails($authUser) : null;
  $: if ($authUser?.id !== subscriptionUserId) void loadSubscription($authUser?.id ?? null);
  $: planPresentation = subscription ? presentSubscription(subscription) : null;
  $: if (!accountMenuOpen) subscriptionViewTrackedForOpen = false;
  $: if (accountMenuOpen && subscription && !subscriptionViewTrackedForOpen) {
    subscriptionViewTrackedForOpen = true;
    trackSubscriptionViewed();
  }

  async function loadSubscription(userId: string | null): Promise<void> {
    subscriptionUserId = userId;
    subscription = null;
    subscriptionLoading = userId !== null;
    const request = ++subscriptionRequest;
    if (!userId) return;

    try {
      const nextSubscription = await getCurrentSubscription();
      if (request !== subscriptionRequest || subscriptionUserId !== userId) return;
      subscription = nextSubscription;
    } catch {
      if (request !== subscriptionRequest || subscriptionUserId !== userId) return;
      toast.error('套餐信息暂时无法同步，请稍后重试。');
    } finally {
      if (request === subscriptionRequest && subscriptionUserId === userId) subscriptionLoading = false;
    }
  }

  async function managePlan(): Promise<void> {
    if (!subscription || managingPlan) return;
    managingPlan = true;
    trackEvent('subscription_management_started', {
      plan: subscription.plan,
      status: subscription.status,
      surface: `account_menu_${variant}`,
    });

    try {
      const { url } = await createBillingPortalLink(window.location.href);
      window.location.assign(url);
    } catch {
      toast.error('无法打开套餐管理页，请稍后重试。');
      managingPlan = false;
    }
  }

  function trackSubscriptionViewed(): void {
    if (!subscription) return;
    trackEvent('subscription_viewed', {
      plan: subscription.plan,
      status: subscription.status,
      surface: `account_menu_${variant}`,
    });
  }
</script>

{#if variant === 'landing' && !details}
  <button class="landing-login" type="button" data-testid="account-login-button" on:click={onLogin}>Login</button>
{:else}
  <DropdownMenu bind:open={accountMenuOpen}>
    <DropdownMenuTrigger
      class={variant === 'landing'
        ? 'relative inline-grid h-9 w-9 cursor-pointer place-items-center rounded-full border border-slate-900/10 bg-white shadow-[0_8px_20px_rgba(15,23,42,0.08)] outline-none transition-[background-color,border-color,box-shadow] duration-150 hover:border-slate-900/20 hover:bg-slate-100 hover:shadow-[0_5px_14px_rgba(15,23,42,0.12)] data-[state=open]:border-slate-900/20 data-[state=open]:bg-slate-100 data-[state=open]:shadow-[0_5px_14px_rgba(15,23,42,0.12)] focus-visible:ring-2 focus-visible:ring-blue-600/30 focus-visible:ring-offset-2'
        : 'relative inline-grid h-6 w-6 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-[var(--text-primary)] outline-none transition-[background-color,box-shadow] duration-150 hover:bg-[var(--panel-bg-alt)] data-[state=open]:bg-[var(--panel-bg-alt)] focus-visible:ring-2 focus-visible:ring-blue-600/30'}
      aria-label={details ? `Account for ${details.name}` : 'Account'}
      title={details ? details.name : 'Account'}
      data-testid={details ? 'account-avatar-button' : 'account-menu-button'}
    >
      {#if details}
        <span class="avatar-frame">
          {#if details.avatarUrl}
            <img class="avatar-image" src={details.avatarUrl} alt="" referrerpolicy="no-referrer" />
          {:else}
            <span class="avatar-fallback" aria-hidden="true">{details.initial}</span>
          {/if}
        </span>
        {#if planPresentation}
          <span class:pro-plan-badge={subscription?.plan !== 'free'} class="plan-badge" aria-label={`当前套餐：${planPresentation.label}`}>{planPresentation.badge}</span>
        {/if}
      {:else}
        <UserIcon size={12} />
      {/if}
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" sideOffset={8} class={details ? 'w-[240px] rounded-[10px] p-2 shadow-[0_14px_38px_rgba(15,23,42,0.16)]' : undefined}>
      {#if details}
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
            {#if details.email}<span class="mt-0.5 block truncate text-[12px] text-[#64748b]">{details.email}</span>{/if}
          </span>
        </div>
        <div class="mx-2 mb-1 flex items-center justify-between rounded-[7px] bg-[#f1f5f9] px-2 py-1.5 text-[12px]" data-testid="account-plan">
          <span class="text-[#64748b]">套餐</span>
          {#if planPresentation}
            <strong class="text-[#0f172a]">{planPresentation.label}{planPresentation.cadence ? ` · ${planPresentation.cadence}` : ''}</strong>
          {:else if subscriptionLoading}
            <span class="text-[#64748b]">同步中…</span>
          {:else}
            <span class="text-[#64748b]">暂不可用</span>
          {/if}
        </div>
        {#if subscription}
          <DropdownMenuItem
            class="rounded-[7px] px-2 py-2 text-[13px]"
            data-testid="account-manage-plan-menu-item"
            disabled={managingPlan}
            onSelect={() => void managePlan()}
          >
            {subscription.plan === 'free' ? '升级套餐' : managingPlan ? '正在打开…' : '管理套餐'}
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
      {:else}
        <DropdownMenuItem data-testid="account-login-menu-item" onSelect={onLogin}>Login</DropdownMenuItem>
        {#if desktop}
          <DropdownMenuItem data-testid="account-check-updates-menu-item" onSelect={() => void onCheckForUpdates()}>Check for updates</DropdownMenuItem>
        {/if}
        <DropdownMenuItem data-testid="account-settings-menu-item" onSelect={onOpenSettings}>Settings</DropdownMenuItem>
      {/if}
    </DropdownMenuContent>
  </DropdownMenu>
{/if}

<style>
  .landing-login {
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
    top: -5px;
    right: -13px;
    z-index: 1;
    min-width: 22px;
    padding: 1px 3px;
    border: 1px solid #ffffff;
    border-radius: 999px;
    background: #64748b;
    color: #ffffff;
    font-size: 7px;
    font-weight: 800;
    letter-spacing: 0.04em;
    line-height: 1.25;
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
</style>
