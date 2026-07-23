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

  if (typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test')) {
    installTestBridge();
  }

  const adsEnabled = !import.meta.env.DEV && import.meta.env.MODE !== 'test';

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
  {#if adsEnabled}
    <script
      async
      src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-6579013241267492"
      crossorigin="anonymous"
    ></script>
  {/if}
</svelte:head>

<slot />
<Toaster />
