<script lang="ts">
  import { ArrowLeft, Rss } from 'lucide-svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';
  import type { PageData } from './$types';

  export let data: PageData;
</script>

<svelte:head>
  <title>{data.entry.title} · Treease Changelog</title>
  <meta name="description" content={data.entry.summary} />
  <link rel="canonical" href={`https://treease.com/changelog/${data.entry.slug}`} />
</svelte:head>

<div class="entry-shell">
  <div class="entry-inner">
    <SiteHeader navItems={[...homeHeaderNavItems, { href: '/changelog', label: 'Changelog' }]} />

    <main class="entry-main">
      <div class="entry-actions">
        <a href="/changelog"><ArrowLeft size={16} strokeWidth={1.8} /> Back to changelog</a>
        <a href="/changelog.xml"><Rss size={15} strokeWidth={1.8} /> RSS</a>
      </div>
      <article class="entry-card">
        <div class="entry-rail"><span class="rail-dot"></span><span>Update</span></div>
        <div class="entry-body">
          <div class="meta"><time datetime={data.entry.isoDate}>{data.entry.date}</time><span>·</span><span>Product update</span></div>
          <h1>{data.entry.title}</h1>
          <p class="summary">{data.entry.summary}</p>
          <div class="tag-row">{#each data.entry.tags as tag}<span class="tag">{tag}</span>{/each}</div>
          {#each data.entry.sections ?? [] as section}
            <section class="entry-section">
              <h2>{section.heading}</h2>
              {#each section.paragraphs as paragraph}<p>{paragraph}</p>{/each}
            </section>
          {/each}
          {#if data.entry.tutorialLinks?.length}
            <nav class="tutorial-links" aria-label="Related tutorials">
              <span>Related tutorials</span>
              {#each data.entry.tutorialLinks as tutorial}<a href={tutorial.href}>{tutorial.label}</a>{/each}
            </nav>
          {/if}
          <p class="byline">Authored by <strong>{data.entry.author}</strong></p>
        </div>
      </article>
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .entry-shell { --ink: #10192a; --muted: #607086; --line: rgba(16,25,42,.11); --accent: #2d63e2; --accent-strong: #1745b5; min-height: 100svh; color: var(--ink); background: radial-gradient(circle at 86% 8%, rgba(80,145,255,.15), transparent 27rem), linear-gradient(180deg, #fbfdff 0%, #f2f6fb 100%); }
  .entry-inner { width: min(1220px, 100%); margin: auto; padding: 28px 24px 100px; }
  .entry-main { width: min(970px, 100%); margin: 58px auto 0; }
  .entry-actions { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; }
  .entry-actions a { display: inline-flex; align-items: center; gap: 7px; color: var(--muted); font-size: 13px; font-weight: 700; text-decoration: none; transition: color .18s ease; }
  .entry-actions a:hover { color: var(--accent); }
  .entry-card { display: grid; grid-template-columns: 112px minmax(0, 1fr); overflow: hidden; border: 1px solid rgba(45,99,226,.18); border-radius: 24px; background: linear-gradient(135deg, rgba(236,244,255,.88), rgba(255,255,255,.93) 62%); box-shadow: 0 28px 70px rgba(34,65,118,.1); }
  .entry-rail { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 34px 16px; border-right: 1px solid rgba(45,99,226,.14); color: var(--accent); font-size: 11px; font-weight: 800; letter-spacing: .15em; text-transform: uppercase; writing-mode: vertical-rl; }
  .rail-dot { width: 9px; height: 9px; border-radius: 50%; background: #65a6ff; box-shadow: 0 0 0 6px rgba(101,166,255,.14); }
  .entry-body { max-width: 760px; padding: 54px clamp(26px, 6vw, 82px) 64px; }
  .meta { display: flex; gap: 9px; color: #8090a4; font-size: 12px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; }
  h1, h2 { margin: 0; font-family: "Avenir Next", "SF Pro Display", "PingFang SC", sans-serif; letter-spacing: -.045em; }
  h1 { margin-top: 20px; font-size: clamp(2.8rem, 6vw, 5.2rem); line-height: .98; }
  .summary { margin: 22px 0 0; color: #4f6077; font-size: 19px; line-height: 1.65; }
  .tag-row { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 24px; }
  .tag { padding: 5px 9px; border-radius: 5px; color: #3868c2; background: rgba(113,161,239,.15); font-size: 11px; font-weight: 800; letter-spacing: .06em; text-transform: uppercase; }
  .entry-section { margin-top: 50px; }
  .entry-section h2 { font-size: 24px; letter-spacing: -.03em; }
  .entry-section p { margin: 14px 0 0; color: #4f6077; font-size: 16px; line-height: 1.75; }
  .tutorial-links { display: flex; flex-wrap: wrap; align-items: center; gap: 9px; margin-top: 46px; }
  .tutorial-links span { width: 100%; color: #78879b; font-size: 12px; font-weight: 800; letter-spacing: .1em; text-transform: uppercase; }
  .tutorial-links a { padding: 8px 11px; border: 1px solid rgba(45,99,226,.18); border-radius: 999px; color: var(--accent); background: rgba(45,99,226,.07); font-size: 13px; font-weight: 700; text-decoration: none; }
  .tutorial-links a:hover { border-color: rgba(45,99,226,.35); background: rgba(45,99,226,.12); }
  .byline { margin: 44px 0 0; color: #78879b; font-size: 13px; }
  .byline strong { color: var(--ink); }
  @media (max-width: 720px) { .entry-inner { padding: 20px 16px 70px; } .entry-main { margin-top: 42px; } .entry-card { display: block; } .entry-rail { flex-direction: row; writing-mode: horizontal-tb; border-right: 0; border-bottom: 1px solid rgba(45,99,226,.14); padding: 16px 22px; } .entry-body { padding: 30px 22px 38px; } .entry-actions { align-items: flex-start; flex-direction: column; gap: 12px; } }
</style>
