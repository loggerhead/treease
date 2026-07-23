<script lang="ts">
  import LegalPage from '$lib/components/LegalPage.svelte'
  import source from '$lib/legal/terms.html?raw'
</script>

<LegalPage
  title="Terms of Service"
  description="Read Treease's terms of service for the rules, responsibilities, licenses, account terms, and limitations that govern use of the service."
  html={source}
  canonicalUrl="https://treease.com/terms"
/>
