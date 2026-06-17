<script lang="ts">
  import '../app.css';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import { installTestBridge } from '$lib/test-bridge/bootstrap';

  if (typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test')) {
    installTestBridge();
  }
</script>

<slot />
<Toaster />
