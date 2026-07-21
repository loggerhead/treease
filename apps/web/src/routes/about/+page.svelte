<script lang="ts">
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';
</script>

<svelte:head>
  <title>About Treease | Structured Text Workspace</title>
  <meta
    name="description"
    content="Learn what Treease is, how its browser workspace connects structured text with graph views, and where to find the open-source CLI and core packages."
  />
  <link rel="canonical" href="https://treease.com/about" />
</svelte:head>

<div class="about-shell">
  <div class="about-shell__inner">
    <SiteHeader navItems={[...homeHeaderNavItems, { href: '/changelog', label: 'Changelog' }]} />

    <main class="prose-page">
      <p class="eyebrow">About Treease</p>
      <h1>Understand structured text before you change it.</h1>
      <p class="lede">
        Treease is a browser workspace for inspecting JSON, YAML, TOML, CSV, and embedded payloads.
        It keeps source text, tree paths, graph context, comparison, and export preview connected.
      </p>

      <section>
        <h2>What Treease is for</h2>
        <p>
          Use Treease when a document is difficult to understand before it is difficult to edit:
          nested API responses, configuration files, mixed logs, migration payloads, and structured
          data that needs a careful review.
        </p>
      </section>

      <section>
        <h2>Open-source foundation</h2>
        <p>
          The CLI and parsing/runtime packages are published as open-source Rust crates. Read the
          source on <a href="https://github.com/loggerhead/treease">GitHub</a>, install the
          <a href="https://crates.io/crates/treease-cli">Treease CLI</a>, or browse its
          <a href="https://docs.rs/treease-cli">API documentation</a>.
        </p>
      </section>

      <section>
        <h2>Privacy by design</h2>
        <p>
          The local editor workflow is designed to process documents in the browser. Read the
          <a href="/privacy">privacy policy</a> for the product's data and account boundaries.
        </p>
      </section>
    </main>

    <SiteFooter />
  </div>
</div>

<style>
  .about-shell { --accent: #2d63e2; --accent-strong: #1745b5; --line: rgba(16, 25, 42, 0.1); --muted: #536273; --muted-soft: #718196; --font-display: "Avenir Next", "SF Pro Display", sans-serif; min-height: 100svh; color: #10192a; background: linear-gradient(180deg, #fbfdff 0%, #f4f7fb 100%); }
  .about-shell__inner { box-sizing: border-box; width: min(1220px, 100%); margin: 0 auto; padding: 28px 24px 72px; }
  .prose-page { max-width: 760px; margin: 0 auto; padding: 14px 0 80px; color: #10192a; }
  .prose-page a { color: #1745b5; }
  .eyebrow { margin: 56px 0 12px; color: #2d63e2; font-weight: 700; letter-spacing: .12em; text-transform: uppercase; }
  h1 { max-width: 680px; margin: 0; font-size: clamp(2.4rem, 6vw, 4.5rem); line-height: 1.02; letter-spacing: -.05em; }
  h2 { margin: 42px 0 10px; font-size: 1.5rem; }
  p { line-height: 1.75; }
  .lede { max-width: 650px; margin-top: 22px; color: #536273; font-size: 1.2rem; }
</style>
