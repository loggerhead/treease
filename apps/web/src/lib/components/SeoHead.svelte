<script lang="ts">
  import { defaultSocialImage } from '$lib/seo/site-seo';

  export let title: string;
  export let description: string;
  export let canonical: string;
  export let image = defaultSocialImage;
  export let type: 'website' | 'article' = 'website';
  export let robots: string | null = null;
  export let jsonLd: readonly string[] = [];
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={description} />
  {#if robots}
    <meta name="robots" content={robots} />
  {/if}
  <link rel="canonical" href={canonical} />
  <meta property="og:type" content={type} />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={canonical} />
  <meta property="og:image" content={image} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={image} />
  {#each jsonLd as item}
    {@html `<script type="application/ld+json">${item}</script>`}
  {/each}
</svelte:head>
