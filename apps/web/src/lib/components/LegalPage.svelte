<script lang="ts">
  import SeoHead from './SeoHead.svelte';
  import SiteFooter from './SiteFooter.svelte';
  import SiteHeader from './SiteHeader.svelte';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';

  export let title: string
  export let description: string
  export let html: string
  export let canonicalUrl: string
</script>

<SeoHead title={`${title} | Treease`} description={description} canonical={canonicalUrl} />

<div class="legal-page">
  <div class="legal-shell">
    <SiteHeader navItems={[...homeHeaderNavItems, { href: '/changelog', label: 'Changelog' }]} ctaLabel="Open Editor" />

    <main class="legal-main" id="main-content" tabindex="-1">
      <article class="legal-card">
        <div class="legal-copy">
          {@html html}
        </div>
      </article>
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .legal-page {
    --accent: #2563eb;
    --accent-strong: #1d4ed8;
    --bg: #eff4fb;
    --bg-strong: #f7faff;
    --surface: rgba(255, 255, 255, 0.9);
    --ink: #0f172a;
    --muted: #4b5563;
    --muted-soft: #718196;
    --font-display: "Avenir Next", "SF Pro Display", sans-serif;
    --line: rgba(15, 23, 42, 0.1);
    --shadow: 0 30px 80px rgba(15, 23, 42, 0.12);

    min-height: 100svh;
    background:
      radial-gradient(circle at 12% 10%, rgba(125, 211, 252, 0.2), transparent 30%),
      radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.16), transparent 34%),
      linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    color: var(--ink);
  }

  .legal-shell {
    box-sizing: border-box;
    width: min(1120px, 100%);
    margin: 0 auto;
    padding: 24px 24px 56px;
  }

  .legal-card {
    border: 1px solid var(--line);
    border-radius: 32px;
    background: var(--surface);
    box-shadow: var(--shadow);
    padding: 40px;
  }

  .legal-copy {
    max-width: 78ch;
    margin: 0 auto;
    color: var(--muted);
    font-size: 16px;
    line-height: 1.75;
  }

  .legal-copy :global(h1),
  .legal-copy :global(h2),
  .legal-copy :global(p),
  .legal-copy :global(ul) {
    margin: 0;
  }

  .legal-copy :global(h1) {
    color: var(--ink);
    font-size: clamp(2.4rem, 4vw, 3.4rem);
    line-height: 1;
    letter-spacing: -0.04em;
  }

  .legal-copy :global(h2) {
    margin-top: 28px;
    color: var(--ink);
    font-size: 1.2rem;
    line-height: 1.3;
    letter-spacing: -0.02em;
  }

  .legal-copy :global(p),
  .legal-copy :global(ul) {
    margin-top: 14px;
  }

  .legal-copy :global(ul) {
    padding-left: 22px;
  }

  .legal-copy :global(li + li) {
    margin-top: 10px;
  }

  .legal-copy :global(a) {
    color: var(--accent);
    font-weight: 600;
    text-decoration: none;
  }

  .legal-copy :global(a:hover) {
    text-decoration: underline;
  }

  @media (max-width: 720px) {
    .legal-shell {
      padding: 16px 16px 32px;
    }

    .legal-card {
      border-radius: 24px;
      padding: 22px;
    }

  }
</style>
