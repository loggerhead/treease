<script lang="ts">
  import LegalPage from '$lib/components/LegalPage.svelte'
  import source from '$lib/legal/privacy.html?raw'
</script>

<LegalPage
  title="Privacy Policy"
  description="Read Treease's privacy policy for information about data collection, account use, security, analytics, and your privacy rights."
  html={source}
  canonicalUrl="https://treease.com/privacy"
/>
