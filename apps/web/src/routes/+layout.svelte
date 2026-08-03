<script lang="ts">
  import '../app.css';
  import { assetUrl, r2Assets } from '$lib/assets';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import { installTestBridge } from '$lib/test-bridge/bootstrap';
  import { afterNavigate } from '$app/navigation';
  import { onMount } from 'svelte';
  import { installFeedbackConsoleLogBuffer } from '$lib/feedback/console-log-buffer';
  import { initializeAnalytics, trackPageView } from '$lib/analytics/ga4';
  import 'virtual:wdio-plugin';

  const testRuntime = import.meta.env.MODE === 'test';

  if (typeof window !== 'undefined' && (import.meta.env.DEV || testRuntime || import.meta.env.PUBLIC_WDIO_TEST === '1')) {
    installTestBridge();
  }

  onMount(() => {
    installFeedbackConsoleLogBuffer();

    void initializeAnalytics().then(() => {
      trackPageView(window.location.pathname);
    });
  });

  afterNavigate(({ to }) => {
    trackPageView(to?.url.pathname ?? window.location.pathname);
  });
</script>

<svelte:head>
  <link rel="icon" type="image/png" href={assetUrl(r2Assets.treeaseLogo)} />
</svelte:head>

<slot />
<Toaster />
