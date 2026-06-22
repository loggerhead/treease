<script lang="ts">
  import '../app.css';
  import { assetUrl } from '$lib/assets';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import { installTestBridge } from '$lib/test-bridge/bootstrap';

  if (typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test')) {
    installTestBridge();
  }
</script>

<svelte:head>
  <link rel="icon" type="image/png" href={assetUrl('/treease-logo.png')} />
</svelte:head>

<slot />
<Toaster />
