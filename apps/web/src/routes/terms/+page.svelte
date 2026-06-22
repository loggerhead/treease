<script lang="ts">
  import LegalPage from '$lib/components/LegalPage.svelte'
  import source from '$lib/legal/terms.html?raw'
</script>

<LegalPage title="Terms of Service" html={source} />
