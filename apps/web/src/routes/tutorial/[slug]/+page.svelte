<script lang="ts">
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import TutorialArticlePage from '$lib/components/tutorials/TutorialArticlePage.svelte';
  import type { TutorialArticle } from '$lib/content/tutorials/types';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';

  export let data: {
    article: TutorialArticle;
  };
</script>

<div class="tutorial-page-shell">
  <div class="tutorial-page-shell__inner">
    <SiteHeader navItems={[...homeHeaderNavItems]} />

    <main class="tutorial-page-shell__main">
      <TutorialArticlePage article={data.article} />
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .tutorial-page-shell {
    --accent: #2d63e2;
    --accent-strong: #1745b5;
    --surface: rgba(255, 255, 255, 0.92);
    --surface-strong: #ffffff;
    --surface-muted: #f0f4fa;
    --line: rgba(16, 25, 42, 0.1);
    --shadow: 0 26px 60px rgba(18, 29, 46, 0.08);
    --muted: #536273;
    --muted-soft: #718196;
    --font-display: "Avenir Next", "SF Pro Display", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;

    min-height: 100svh;
    background:
      radial-gradient(circle at top left, rgba(124, 160, 255, 0.16), transparent 25%),
      linear-gradient(180deg, #fbfdff 0%, #f4f7fb 100%);
    color: #10192a;
  }

  .tutorial-page-shell__inner {
    box-sizing: border-box;
    position: relative;
    z-index: 1;
    width: min(1220px, 100%);
    margin: 0 auto;
    padding: 28px 24px 72px;
  }

  .tutorial-page-shell__main {
    width: min(980px, 100%);
    margin: 0 auto;
  }
</style>
