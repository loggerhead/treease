<script lang="ts">
  import '../app.css';
  import { assetUrl } from '$lib/assets';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import { installTestBridge } from '$lib/test-bridge/bootstrap';
  import { afterNavigate } from '$app/navigation';
  import { onMount } from 'svelte';
  import { initializeAnalytics, trackPageView } from '$lib/analytics/ga4';
  import { installAssetLoadRecovery } from '$lib/runtime/asset-load-recovery';
  import 'virtual:wdio-plugin';

  if (typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test')) {
    installTestBridge();
  }

  onMount(() => {
    const uninstallAssetLoadRecovery = installAssetLoadRecovery(window);
    void initializeAnalytics().then(() => {
      trackPageView(window.location.pathname);
    });

    return uninstallAssetLoadRecovery;
  });

  afterNavigate(({ to }) => {
    trackPageView(to?.url.pathname ?? window.location.pathname);
  });
</script>

<svelte:head>
  <link rel="icon" type="image/png" href={assetUrl('/treease-logo.png')} />
</svelte:head>

<slot />
<Toaster />
