<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import TutorialHubSection from '$lib/components/tutorials/TutorialHubSection.svelte';
  import { tutorialArticles } from '$lib/content/tutorials';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';

  const pageTitle = 'Treease Tutorials';
  const pageDescription =
    'Task-focused Treease tutorials for formatting JSON, comparing structured documents, loading source text from URLs, and understanding editor URL parameters.';
  const canonicalUrl = 'https://treease.com/tutorial';
</script>

<SeoHead title={`${pageTitle} | Structured Text Workspace`} description={pageDescription} canonical={canonicalUrl} />

<div class="tutorial-shell">
  <div class="tutorial-shell__inner">
    <SiteHeader navItems={[...homeHeaderNavItems]} />

    <main class="tutorial-shell__main" id="main-content" tabindex="-1">
      <TutorialHubSection
        eyebrow="Tutorial Library"
        title="JSON viewer, formatter, and compare tutorials"
        lede="Use Treease to view nested JSON, format compact documents, compare two structures, and prepare URL-backed editor workflows. Each guide includes a direct answer, practical steps, and an example you can open."
        articles={tutorialArticles}
      />
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .tutorial-shell {
    --accent: #2d63e2;
    --accent-strong: #1745b5;
    --bg: #f4f7fb;
    --surface: rgba(255, 255, 255, 0.92);
    --line: rgba(16, 25, 42, 0.1);
    --shadow: 0 26px 60px rgba(18, 29, 46, 0.08);
    --muted: #536273;
    --font-display: "Avenir Next", "SF Pro Display", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;

    min-height: 100svh;
    background:
      radial-gradient(circle at top right, rgba(45, 99, 226, 0.09), transparent 30%),
      linear-gradient(180deg, #fbfdff 0%, var(--bg) 100%);
    color: #10192a;
  }

  .tutorial-shell__inner {
    box-sizing: border-box;
    position: relative;
    z-index: 1;
    width: min(1220px, 100%);
    margin: 0 auto;
    padding: 28px 24px 72px;
  }

  .tutorial-shell__main {
    width: min(1120px, 100%);
  }
</style>
