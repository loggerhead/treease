<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { LoaderCircle } from 'lucide-svelte';
  import {
    clearAuthSession,
    exchangeAuthCode,
    signInWithProviderIndependently,
    takePendingIdentityProvider,
  } from '../../../lib/auth/supabase-auth';
  import { resolveAuthReturnPath } from '../../../lib/auth/auth-redirect';

  let error = '';
  let returnPath = '/';

  onMount(async () => {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    returnPath = resolveAuthReturnPath(params.get('next'));
    const pendingProvider = takePendingIdentityProvider();
    if (!code) {
      if (pendingProvider) {
        await clearAuthSession();
        await signInWithProviderIndependently(pendingProvider);
        return;
      }
      await goto(returnPath);
      return;
    }
    try {
      await exchangeAuthCode(code);
      await goto(returnPath);
    } catch (cause) {
      if (pendingProvider) {
        await clearAuthSession();
        await signInWithProviderIndependently(pendingProvider);
        return;
      }
      error = cause instanceof Error ? cause.message : 'Unable to complete login.';
    }
  });
</script>

<main class="grid min-h-screen place-items-center bg-[var(--app-bg)] p-6 text-[var(--text-primary)]">
  {#if error}
    <section class="rounded-[16px] border border-red-200 bg-white p-8 text-center shadow-sm">
      <h1 class="text-xl font-semibold">Login failed</h1>
      <p class="mt-2 text-sm text-red-700">{error}</p>
      <a class="mt-5 inline-flex rounded-[9px] bg-[var(--accent)] px-4 py-2 text-sm text-white" href={returnPath}>Return to the previous page</a>
    </section>
  {:else}
    <LoaderCircle size={24} class="animate-spin text-[var(--accent)]" aria-label="Completing login" />
  {/if}
</main>
