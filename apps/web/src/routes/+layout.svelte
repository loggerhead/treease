<script lang="ts">
  import { onMount } from 'svelte';
  import '../app.css';
  import { Toaster } from '$lib/components/ui/sonner/index.js';

  onMount(() => {
    if (!(import.meta.env.DEV || import.meta.env.MODE === 'test')) return;

    void import('$lib/test-bridge/bootstrap');
  });
</script>

<slot />
<Toaster />
