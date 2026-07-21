<script lang="ts">
  import TutorialCodeLink from '$lib/components/tutorials/TutorialCodeLink.svelte';
  import { assetUrl, r2Assets } from '$lib/assets';
  import { getRelatedTutorialArticles } from '$lib/content/tutorials';
  import type { TutorialArticle } from '$lib/content/tutorials/types';

  export let article: TutorialArticle;

  const siteOrigin = 'https://treease.com';
  function sectionId(title: string): string {
    return title.toLowerCase().replaceAll(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  }

  $: relatedArticles = getRelatedTutorialArticles(article);
  $: articleUrl = `${siteOrigin}/tutorial/${article.slug}`;
  $: articleJsonLd = JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'TechArticle',
    headline: article.title,
    description: article.description,
    dateModified: article.updatedOn,
    author: {
      '@type': 'Organization',
      name: 'Treease',
    },
    publisher: {
      '@type': 'Organization',
      name: 'Treease',
      logo: {
        '@type': 'ImageObject',
        url: assetUrl(r2Assets.treeaseLogo),
      },
    },
    mainEntityOfPage: articleUrl,
    image: assetUrl(r2Assets.heroDemoGraphPoster),
    articleSection: article.eyebrow,
    isPartOf: {
      '@type': 'CollectionPage',
      name: 'Treease Tutorials',
      url: `${siteOrigin}/tutorial`,
    },
  });
  $: breadcrumbJsonLd = JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Treease', item: siteOrigin },
      { '@type': 'ListItem', position: 2, name: 'Tutorials', item: `${siteOrigin}/tutorial` },
      { '@type': 'ListItem', position: 3, name: article.title, item: articleUrl },
    ],
  });
</script>

<svelte:head>
  <title>{article.title} | Treease Tutorial | Structured Text Workspace</title>
  <meta name="description" content={article.description} />
  <link rel="canonical" href={articleUrl} />
  <meta property="og:type" content="article" />
  <meta property="og:title" content={article.title} />
  <meta property="og:description" content={article.description} />
  <meta property="og:url" content={articleUrl} />
  <meta property="og:image" content={assetUrl(r2Assets.heroDemoGraphPoster)} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={article.title} />
  <meta name="twitter:description" content={article.description} />
  <meta name="twitter:image" content={assetUrl(r2Assets.heroDemoGraphPoster)} />
  <script type="application/ld+json">{articleJsonLd}</script>
  <script type="application/ld+json">{breadcrumbJsonLd}</script>
</svelte:head>

<article class="tutorial-article">
  <header class="tutorial-article__hero">
    <p class="tutorial-article__eyebrow">{article.eyebrow}</p>
    <h1>{article.title}</h1>
    <p class="tutorial-article__lede">{article.lede}</p>
    <div class="tutorial-article__meta">
      <span>{article.readingMinutes} min read</span>
      <span>Updated {article.updatedOn}</span>
    </div>
  </header>

  <nav class="tutorial-article__toc" aria-label="Tutorial sections">
    <span>On this page</span>
    <div>
      {#each article.sections as section}
        <a href={`#${sectionId(section.title)}`}>{section.title}</a>
      {/each}
    </div>
  </nav>

  <div class="tutorial-article__body">
    {#each article.sections as section}
      <section class="tutorial-section" id={sectionId(section.title)}>
        <h2>{section.title}</h2>
        {#if section.summary}
          <p class="tutorial-section__summary">{section.summary}</p>
        {/if}
        {#if section.paragraphs}
          {#each section.paragraphs as paragraph}
            <p>{paragraph}</p>
          {/each}
        {/if}
        {#if section.bullets}
          <ul>
            {#each section.bullets as bullet}
              <li>{bullet}</li>
            {/each}
          </ul>
        {/if}
        {#if section.parameters}
          <div class="tutorial-parameter-grid">
            {#each section.parameters as parameter}
              <article class="tutorial-parameter-card">
                <div class="tutorial-parameter-card__name">{parameter.name}</div>
                <p>{parameter.purpose}</p>
                <TutorialCodeLink
                  href={parameter.href}
                  label={parameter.displayHref ?? parameter.href}
                  ariaLabel={`Open ${parameter.name} example`}
                />
                {#if parameter.note}
                  <p class="tutorial-parameter-card__note">{parameter.note}</p>
                {/if}
              </article>
            {/each}
          </div>
        {/if}
        {#if section.examples}
          <div class="tutorial-example-grid">
            {#each section.examples as example}
              <article class="tutorial-example-card">
                <h3>{example.label}</h3>
                <p>{example.description}</p>
                <TutorialCodeLink
                  href={example.href}
                  label={example.displayHref ?? example.href}
                  ariaLabel={`Open ${example.label} example`}
                />
              </article>
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  </div>

  <section class="tutorial-faq" aria-labelledby="tutorial-faq-title">
    <div class="tutorial-faq__intro">
      <h2 id="tutorial-faq-title">FAQ</h2>
      <p>Short answers that search and generative engines can quote directly.</p>
    </div>
    <div class="tutorial-faq__list">
      {#each article.faq as item}
        <details>
          <summary>{item.question}</summary>
          <p>{item.answer}</p>
        </details>
      {/each}
    </div>
  </section>

  {#if relatedArticles.length > 0}
    <section class="tutorial-related" aria-labelledby="tutorial-related-title">
      <div class="tutorial-faq__intro">
        <h2 id="tutorial-related-title">Related tutorials</h2>
        <p>Use the same Treease workflow as a starting point for nearby tasks.</p>
      </div>
      <div class="tutorial-example-grid">
        {#each relatedArticles as related}
          <article class="tutorial-example-card">
            <h3>{related.title}</h3>
            <p>{related.description}</p>
            <TutorialCodeLink
              href={`/tutorial/${related.slug}`}
              label={`/tutorial/${related.slug}`}
              ariaLabel={`Open related tutorial ${related.title}`}
            />
          </article>
        {/each}
      </div>
    </section>
  {/if}
</article>

<style>
  .tutorial-article {
    display: grid;
    gap: 28px;
  }

  .tutorial-article__hero {
    display: grid;
    gap: 14px;
    padding: 32px;
    border: 1px solid var(--line, rgba(16, 25, 42, 0.1));
    border-radius: 28px;
    background:
      radial-gradient(circle at top right, rgba(45, 99, 226, 0.12), transparent 28%),
      var(--surface, rgba(255, 255, 255, 0.92));
    box-shadow: var(--shadow, 0 26px 60px rgba(18, 29, 46, 0.08));
  }

  .tutorial-article__eyebrow {
    margin: 0;
    color: var(--accent, #2d63e2);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .tutorial-article__hero h1,
  .tutorial-faq__intro h2,
  .tutorial-section h2 {
    margin: 0;
    font-family: var(--font-display, inherit);
    letter-spacing: -0.04em;
  }

  .tutorial-article__hero h1 {
    font-size: clamp(2.2rem, 4vw, 3.8rem);
    line-height: 1.04;
  }

  .tutorial-article__lede,
  .tutorial-section__summary,
  .tutorial-faq__intro p {
    margin: 0;
    color: var(--muted, #536273);
    line-height: 1.75;
  }

  .tutorial-article__meta,
  .tutorial-article__toc,
  .tutorial-article__toc div {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .tutorial-article__meta {
    color: var(--muted-soft, #718196);
    font-size: 13px;
    font-weight: 700;
  }

  .tutorial-article__toc {
    align-items: flex-start;
    padding: 22px 24px;
    border: 1px solid var(--line, rgba(16, 25, 42, 0.1));
    border-radius: 24px;
    background: rgba(255, 255, 255, 0.72);
  }

  .tutorial-article__toc > span {
    min-width: 110px;
    color: var(--muted-soft, #718196);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .tutorial-article__toc a {
    color: var(--accent-strong, #1745b5);
    font-weight: 700;
    text-decoration: none;
  }

  .tutorial-article__body,
  .tutorial-faq,
  .tutorial-related {
    display: grid;
    gap: 24px;
  }

  .tutorial-section {
    display: grid;
    gap: 16px;
    padding: 28px;
    border: 1px solid var(--line, rgba(16, 25, 42, 0.1));
    border-radius: 24px;
    background: var(--surface-strong, #ffffff);
    box-shadow: var(--shadow, 0 26px 60px rgba(18, 29, 46, 0.08));
  }

  .tutorial-section p,
  .tutorial-faq__list p,
  .tutorial-example-card p,
  .tutorial-parameter-card p {
    margin: 0;
    color: var(--muted, #536273);
    line-height: 1.75;
  }

  .tutorial-section ul {
    margin: 0;
    padding-left: 20px;
    color: var(--muted, #536273);
    line-height: 1.8;
  }

  .tutorial-parameter-grid,
  .tutorial-example-grid,
  .tutorial-faq__list {
    display: grid;
    gap: 16px;
  }

  .tutorial-parameter-grid,
  .tutorial-example-grid {
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  }

  .tutorial-parameter-card,
  .tutorial-example-card {
    display: grid;
    gap: 12px;
    padding: 20px;
    border-radius: 20px;
    background: var(--surface-muted, #f0f4fa);
    border: 1px solid rgba(45, 99, 226, 0.12);
  }

  .tutorial-parameter-card__name {
    font-size: 1.05rem;
    font-weight: 700;
  }

  .tutorial-parameter-card__note {
    color: var(--muted-soft, #718196);
    font-size: 0.95rem;
  }

  .tutorial-faq__intro {
    display: grid;
    gap: 8px;
  }

  .tutorial-faq__list details {
    padding: 20px 22px;
    border: 1px solid var(--line, rgba(16, 25, 42, 0.1));
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.84);
  }

  .tutorial-faq__list summary {
    cursor: pointer;
    font-weight: 700;
  }

  .tutorial-faq__list p {
    margin-top: 12px;
  }

  @media (max-width: 720px) {
    .tutorial-article__hero,
    .tutorial-section {
      padding: 22px;
    }

    .tutorial-article__toc {
      display: grid;
    }
  }
</style>
