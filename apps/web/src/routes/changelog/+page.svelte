<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import { ArrowUpRight, Rss } from 'lucide-svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import { changelogEntries, type ChangelogEntry } from '$lib/content/changelog';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';

  let activeYear = '2026';

  const years = [...new Set(changelogEntries.map((entry) => entry.isoDate.slice(0, 4)))];
  const entriesForYear = (year: string): readonly ChangelogEntry[] =>
    changelogEntries.filter((entry) => entry.isoDate.startsWith(year));
  const sectionId = (entry: ChangelogEntry, heading: string): string =>
    `${entry.slug}-${heading.toLowerCase().replaceAll(/[^a-z0-9]+/g, '-')}`;

</script>

<SeoHead
  title="Changelog · Treease"
  description="New updates and product improvements in Treease."
  canonical="https://treease.com/changelog"
/>

<div class="changelog-shell">
  <div class="changelog-inner">
    <SiteHeader navItems={[...homeHeaderNavItems, { href: '/changelog', label: 'Changelog' }]} />

    <main id="main-content" tabindex="-1" aria-labelledby="changelog-title">
      <section class="intro">
        <div>
          <p class="eyebrow">Product journal <span>✳</span></p>
          <h1 id="changelog-title">Changelog</h1>
          <p class="lede">Small releases, sharper workflows, and the details behind how Treease keeps structured text understandable.</p>
        </div>
        <div class="intro-actions" role="region" aria-label="Changelog actions">
          <a class="action-link" href="/changelog.xml"><Rss size={15} strokeWidth={1.8} /> RSS</a>
        </div>
      </section>

      {#if changelogEntries[0]}
        <article class="feature-card">
          <div class="feature-rail"><span class="rail-dot"></span><span>Latest</span></div>
          <div class="feature-body">
            <div class="meta"><time datetime={changelogEntries[0].isoDate}>{changelogEntries[0].date}</time><span>·</span><span>Product update</span></div>
            <h2>{changelogEntries[0].title}</h2>
            <p class="feature-summary">{changelogEntries[0].summary}</p>
            <div class="tag-row">
              {#each changelogEntries[0].tags as tag}<span class="tag">{tag}</span>{/each}
            </div>
            {#if changelogEntries[0].sections}
              {#each changelogEntries[0].sections as section}
                <section class="feature-section" id={sectionId(changelogEntries[0], section.heading)}>
                  <h3>{section.heading}<a href={`#${sectionId(changelogEntries[0], section.heading)}`} aria-label={`Link to ${section.heading}`}>#</a></h3>
                {#each section.paragraphs as paragraph}<p>{paragraph}</p>{/each}
              </section>
            {/each}
            {/if}
            {#if changelogEntries[0].tutorialLinks?.length}
              <nav class="tutorial-links" aria-label="Related tutorials">
                <span>Related tutorials</span>
                {#each changelogEntries[0].tutorialLinks as tutorial}<a href={tutorial.href}>{tutorial.label}</a>{/each}
              </nav>
            {/if}
            <p class="byline">Authored by <strong>{changelogEntries[0].author}</strong></p>
          </div>
        </article>
      {/if}

      <section class="archive" aria-labelledby="archive-title">
        <div class="archive-header">
          <div><p class="eyebrow">The archive <span>↘</span></p><h2 id="archive-title">More updates</h2></div>
          <div class="year-tabs" role="tablist" aria-label="Filter by year">
            {#each years as year}
              <button
                id={`changelog-tab-${year}`}
                type="button"
                role="tab"
                aria-controls="changelog-year-panel"
                aria-selected={activeYear === year}
                tabindex={activeYear === year ? 0 : -1}
                class:active={activeYear === year}
                on:click={() => (activeYear = year)}
              >{year}</button>
            {/each}
          </div>
        </div>
        <div id="changelog-year-panel" role="tabpanel" aria-labelledby={`changelog-tab-${activeYear}`} class="timeline" tabindex="0">
          {#each entriesForYear(activeYear).filter((entry) => entry.slug !== changelogEntries[0]?.slug) as entry, index}
            <a class="timeline-item" href={`/changelog/${entry.slug}`} style={`--delay: ${index * 70}ms`}>
              <time datetime={entry.isoDate}>{entry.date.replace(', 2026', '')}</time>
              <div class="timeline-copy"><h3>{entry.title} <ArrowUpRight size={17} strokeWidth={1.7} /></h3><p>{entry.summary}</p><div class="tag-row">{#each entry.tags as tag}<span class="tag">{tag}</span>{/each}</div></div>
            </a>
          {/each}
        </div>
      </section>
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .changelog-shell {
    --ink: #10192a; --muted: #607086; --line: rgba(16, 25, 42, .11); --accent: #2d63e2; --accent-strong: #1745b5;
    min-height: 100svh; color: var(--ink); background: radial-gradient(circle at 86% 8%, rgba(80, 145, 255, .15), transparent 27rem), linear-gradient(180deg, #fbfdff 0%, #f2f6fb 100%);
  }
  .changelog-inner { width: min(1220px, 100%); margin: auto; padding: 28px 24px 100px; }
  .intro { display: flex; justify-content: space-between; align-items: end; gap: 32px; margin: 66px 0 54px; }
  .eyebrow { margin: 0 0 14px; color: var(--accent); font-size: 12px; font-weight: 800; letter-spacing: .16em; text-transform: uppercase; }
  .eyebrow span { display: inline-block; margin-left: 6px; font-size: 16px; transform: rotate(8deg); }
  h1, h2, h3 { margin: 0; font-family: "Avenir Next", "SF Pro Display", "PingFang SC", sans-serif; letter-spacing: -.045em; }
  h1 { font-size: clamp(4rem, 10vw, 8.5rem); line-height: .86; font-weight: 700; }
  .lede { max-width: 590px; margin: 26px 0 0; color: var(--muted); font-size: 18px; line-height: 1.6; }
  .intro-actions { display: flex; gap: 10px; padding-bottom: 4px; }
  .action-link { display: inline-flex; align-items: center; gap: 7px; min-height: 37px; padding: 0 13px; border: 1px solid var(--line); border-radius: 999px; color: var(--muted); background: rgba(255,255,255,.6); font: inherit; font-size: 13px; font-weight: 700; text-decoration: none; cursor: pointer; transition: .18s ease; }
  .action-link:hover { color: var(--accent); border-color: rgba(45,99,226,.35); background: #fff; transform: translateY(-1px); }
  .feature-card { display: grid; grid-template-columns: 128px minmax(0, 1fr); overflow: hidden; border: 1px solid rgba(45,99,226,.18); border-radius: 24px; background: linear-gradient(135deg, rgba(236,244,255,.88), rgba(255,255,255,.93) 62%); box-shadow: 0 28px 70px rgba(34,65,118,.1); }
  .feature-rail { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 34px 16px; border-right: 1px solid rgba(45,99,226,.14); color: var(--accent); font-size: 11px; font-weight: 800; letter-spacing: .15em; text-transform: uppercase; writing-mode: vertical-rl; }
  .rail-dot { width: 9px; height: 9px; border-radius: 50%; background: #65a6ff; box-shadow: 0 0 0 6px rgba(101,166,255,.14); }
  .feature-body { max-width: 770px; padding: 48px clamp(26px, 6vw, 82px) 54px; }
  .meta, .timeline-item time { color: #8090a4; font-size: 12px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; }
  .meta { display: flex; gap: 9px; }
  .feature-body h2 { margin-top: 20px; font-size: clamp(2.5rem, 6vw, 4.7rem); line-height: .98; }
  .feature-summary { max-width: 660px; margin: 20px 0 0; color: #4f6077; font-size: 18px; line-height: 1.65; }
  .tag-row { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 22px; }
  .tag { padding: 5px 9px; border-radius: 5px; color: #3868c2; background: rgba(113,161,239,.15); font-size: 11px; font-weight: 800; letter-spacing: .06em; text-transform: uppercase; }
  .feature-section { margin-top: 48px; }
  .feature-section h3 { display: flex; align-items: center; gap: 8px; font-size: 22px; letter-spacing: -.03em; }
  .feature-section h3 a { color: #8ba3ca; font-size: 16px; text-decoration: none; }
  .feature-section p { margin: 14px 0 0; color: #4f6077; font-size: 16px; line-height: 1.75; }
  .byline { margin: 42px 0 0; color: #78879b; font-size: 13px; }
  .byline strong { color: var(--ink); }
  .tutorial-links { display: flex; flex-wrap: wrap; align-items: center; gap: 9px; margin-top: 42px; }
  .tutorial-links span { width: 100%; color: #78879b; font-size: 11px; font-weight: 800; letter-spacing: .1em; text-transform: uppercase; }
  .tutorial-links a { padding: 8px 11px; border: 1px solid rgba(45,99,226,.18); border-radius: 999px; color: var(--accent); background: rgba(45,99,226,.07); font-size: 12px; font-weight: 700; text-decoration: none; }
  .tutorial-links a:hover { border-color: rgba(45,99,226,.35); background: rgba(45,99,226,.12); }
  .archive { margin-top: 112px; }
  .archive-header { display: flex; justify-content: space-between; align-items: end; gap: 24px; padding-bottom: 22px; border-bottom: 1px solid var(--line); }
  .archive h2 { font-size: clamp(2.4rem, 5vw, 4rem); line-height: .95; }
  .year-tabs { display: flex; gap: 6px; }
  .year-tabs button { border: 0; border-radius: 999px; padding: 8px 12px; color: #8492a5; background: transparent; font: inherit; font-size: 13px; font-weight: 800; cursor: pointer; }
  .year-tabs button:hover, .year-tabs button.active { color: var(--ink); background: #fff; box-shadow: 0 5px 18px rgba(26,47,79,.08); }
  .timeline { margin-left: 78px; }
  .timeline-item { display: grid; grid-template-columns: 128px minmax(0, 1fr); gap: 26px; padding: 31px 0; border-bottom: 1px solid var(--line); color: inherit; text-decoration: none; animation: rise .5s both; animation-delay: var(--delay); }
  .timeline-item time { padding-top: 6px; }
  .timeline-copy h3 { display: flex; align-items: center; gap: 7px; font-size: clamp(1.2rem, 2.3vw, 1.65rem); letter-spacing: -.035em; }
  .timeline-copy h3 :global(svg) { color: #9aabc2; transition: transform .18s ease, color .18s ease; }
  .timeline-item:hover .timeline-copy h3 :global(svg) { color: var(--accent); transform: translate(3px, -3px); }
  .timeline-copy p { max-width: 670px; margin: 9px 0 0; color: var(--muted); font-size: 15px; line-height: 1.55; }
  .timeline-copy .tag-row { margin-top: 15px; }
  @keyframes rise { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
  @media (max-width: 720px) { .changelog-inner { padding: 20px 16px 70px; } .intro { display: block; margin: 50px 0 35px; } h1 { font-size: clamp(4rem, 21vw, 7rem); } .lede { font-size: 16px; } .intro-actions { margin-top: 22px; } .feature-card { display: block; } .feature-rail { flex-direction: row; writing-mode: horizontal-tb; border-right: 0; border-bottom: 1px solid rgba(45,99,226,.14); padding: 16px 22px; } .feature-body { padding: 30px 22px 38px; } .feature-body h2 { font-size: 2.6rem; } .archive { margin-top: 75px; } .archive-header { align-items: start; flex-direction: column; } .timeline { margin-left: 0; } .timeline-item { display: block; padding: 24px 0; } .timeline-item time { display: block; margin-bottom: 10px; } }
</style>
