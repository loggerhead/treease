<script lang="ts">
  import type { TutorialArticle } from '$lib/content/tutorials/types';

  export let eyebrow = 'Tutorials';
  export let title = 'Learn Treease through task-focused guides.';
  export let lede =
    'Browse structured tutorials that explain what Treease does, how the editor URL works, and how to open focused workflows for formatting, compare, and remote document loading.';
  export let articles: TutorialArticle[] = [];
  export let compact = false;
</script>

<section class:compact class="tutorial-hub" aria-labelledby="tutorial-hub-title">
  <div class="tutorial-hub__intro">
    <p class="tutorial-hub__eyebrow">{eyebrow}</p>
    <h1 id="tutorial-hub-title">{title}</h1>
    <p>{lede}</p>
  </div>

  <div class="tutorial-hub__grid">
    {#each articles as article}
      <article class="tutorial-card">
        <div class="tutorial-card__meta">
          <span>{article.eyebrow}</span>
          <span>{article.readingMinutes} min read</span>
        </div>
        <h3>
          <a href={`/tutorial/${article.slug}`}>{article.title}</a>
        </h3>
        <p>{article.description}</p>
        <a class="tutorial-card__cta" href={`/tutorial/${article.slug}`}>Read tutorial</a>
      </article>
    {/each}
  </div>
</section>

<style>
  .tutorial-hub {
    display: grid;
    gap: 28px;
  }

  .tutorial-hub.compact {
    gap: 22px;
  }

  .tutorial-hub__intro {
    display: grid;
    gap: 10px;
    max-width: 760px;
  }

  .tutorial-hub__eyebrow {
    color: var(--accent, #2d63e2);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .tutorial-hub__intro h1 {
    margin: 0;
    font-family: var(--font-display, inherit);
    font-size: clamp(1.9rem, 3vw, 2.8rem);
    font-weight: 700;
    letter-spacing: -0.04em;
  }

  .tutorial-hub__intro p {
    margin: 0;
    color: var(--muted, #536273);
    font-size: 1rem;
    line-height: 1.7;
  }

  .tutorial-hub__grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 18px;
  }

  .tutorial-card {
    display: grid;
    gap: 14px;
    min-height: 100%;
    padding: 22px;
    border: 1px solid var(--line, rgba(16, 25, 42, 0.1));
    border-radius: 24px;
    background: var(--surface, rgba(255, 255, 255, 0.92));
    box-shadow: var(--shadow, 0 26px 60px rgba(18, 29, 46, 0.08));
  }

  .tutorial-card__meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    color: var(--muted-soft, #718196);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .tutorial-card h3 {
    margin: 0;
    font-size: 1.15rem;
    line-height: 1.35;
  }

  .tutorial-card h3 a {
    color: inherit;
    text-decoration: none;
  }

  .tutorial-card p {
    margin: 0;
    color: var(--muted, #536273);
    line-height: 1.7;
  }

  .tutorial-card__cta {
    color: var(--accent-strong, #1745b5);
    font-weight: 700;
    text-decoration: none;
  }
</style>
