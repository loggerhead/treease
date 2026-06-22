<script lang="ts">
  import { assetUrl } from '$lib/assets';
  import HomeHeroDemoDeck from '$lib/components/HomeHeroDemoDeck.svelte';

  let cliInstallExpanded = false;

  const navItems = [
    { href: '#features', label: 'Features' },
    { href: '#faq', label: 'FAQ' }
  ];

  const valuePoints = [
    'Open a local file and see its structure without leaving the source.',
    'Trace fields across graph, tree path, and source text without losing place.',
    'Review, convert, and export only after the structure looks right.'
  ];

  const faqItems = [
    {
      question: 'Why visualize structured text in Treease?',
      answer:
        'Because raw JSON, YAML, TOML, CSV, or embedded payloads get hard to follow long before they become hard to edit. Treease turns source text into a graph so users can see the structure first, then trace, edit, compare, and export with context.'
    },
    {
      question: 'What kinds of files and workflows does Treease cover?',
      answer:
        'Treease supports local import, format and minify commands, key sorting, active-container isolation inside mixed logs, graph search with tree path reveal, local value preview, synchronized text and graph edits, structural compare with text fallback, export preview, and visible progress for large JSON imports.'
    },
    {
      question: 'What makes Treease different from a plain editor or JSON viewer?',
      answer:
        'Treease keeps source text and graph context attached to the same document state. Instead of showing a disconnected preview, it lets users move from text to graph to tree path to output while staying anchored to the same file.'
    },
    {
      question: 'When should I use Treease instead of a terminal tool or general editor?',
      answer:
        'Use Treease when seeing the structure matters: inspecting nested data, tracing fields, checking changes, or previewing converted output. General-purpose editing, custom CLI filters, and batch automation still belong in an editor or terminal when the job stops being document-centric.'
    }
  ];

  const footerLinks = [
    { href: '/terms', label: 'Terms' },
    { href: '/privacy', label: 'Privacy' }
  ];
</script>

<div class="landing">
  <div class="landing-shell">
    <header class="site-header">
      <a class="brand" href="#top" aria-label="Treease home">
        <img class="brand-logo" src={assetUrl('/treease-logo.png')} alt="Treease logo" />
        <span class="brand-copy">
          <span class="brand-mark">Treease</span>
          <span class="brand-note">Structured text workspace</span>
        </span>
      </a>

      <nav class="site-nav" aria-label="Primary">
        {#each navItems as item}
          <a href={item.href}>{item.label}</a>
        {/each}
      </nav>

      <a class="header-cta" href="/editor">Open editor</a>
    </header>

    <main class="landing-main" id="top" aria-labelledby="hero-title">
      <section class="hero">
        <div class="hero-copy">
          <p class="hero-kicker">Visualize structured text</p>
          <h1 id="hero-title">See the structure inside the source.</h1>
          <p class="hero-lede">
            Treease turns JSON, YAML, TOML, CSV, and embedded payloads into a
            graph you can inspect, trace, edit, compare, and export with
            confidence.
          </p>
          <div class="hero-cta-stack">
            <div class="hero-actions">
              <a class="primary-cta" href="/editor">Open editor</a>
              <button
                type="button"
                class="secondary-cta"
                aria-expanded={cliInstallExpanded}
                aria-controls="hero-cli-install"
                on:click={() => {
                  cliInstallExpanded = !cliInstallExpanded;
                }}
              >
                Install the CLI
              </button>
            </div>
            {#if cliInstallExpanded}
              <div class="cli-quickstart" id="hero-cli-install" aria-label="Treease CLI quick start">
                <div class="cli-quickstart__label">
                  <strong>Install the CLI, then open a readonly local graph view.</strong>
                </div>
                <div class="cli-quickstart__commands">
                  <code>cargo install treease-cli</code>
                  <code>treease web '.services.api' config.yaml</code>
                </div>
              </div>
            {/if}
          </div>
        </div>

        <div class="hero-demo">
          <HomeHeroDemoDeck />
        </div>
      </section>

      <section class="value-strip" aria-labelledby="value-title">
        <div class="section-copy section-copy--compact">
          <h2 id="value-title">Visual structure stays attached to the file.</h2>
          <p>
            Treease keeps source text, graph context, compare, and export
            preview attached to the same working document, so users can
            understand the structure before they act on it.
          </p>
        </div>

        <div class="value-points">
          {#each valuePoints as point}
            <article class="value-point">
              <p>{point}</p>
            </article>
          {/each}
        </div>
      </section>

      <section class="capabilities-section" id="features" aria-labelledby="features-title">
        <div class="section-copy">
          <h2 id="features-title">See, trace, and edit with context.</h2>
          <p>
            Treease is built for the repetitive work around structured files:
            opening them, seeing their shape, tracing exact fields, and making
            changes without losing the source.
          </p>
        </div>

        <div class="capability-grid" aria-label="Treease feature highlights">
          <article class="capability-card capability-card--hero">
            <div class="story-copy">
              <h3>Open a local file and see the real structure.</h3>
              <p>
                Load supported structured files directly into the editor and
                visualize their shape from the first step.
              </p>
            </div>
            <div class="story-media story-media--contain">
              <img
                src={assetUrl('/landing/feature-import.png')}
                alt="Importing a YAML file into the Treease editor."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--soft">
            <div class="story-copy">
              <h3>Clean up the current document in place.</h3>
              <p>
                Format, minify, or sort keys without leaving the editor or
                breaking visual context.
              </p>
            </div>
            <div class="story-media">
              <img
                src={assetUrl('/landing/feature-format.png')}
                alt="Sorting JSON keys from the command search inside Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--dark">
            <div class="story-copy">
              <h3>Lift the active JSON container out of mixed logs.</h3>
              <p>
                Put the cursor inside a JSONL row or embedded payload and let
                the graph isolate the active container from surrounding log
                noise.
              </p>
            </div>
            <div class="story-media story-media--dark story-media--contain story-media--wide-short">
              <img
                src={assetUrl('/landing/feature-container.png')}
                alt="Isolating an embedded JSON payload from mixed log text in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--surface">
            <div class="story-copy">
              <h3>Trace a field from graph to source.</h3>
              <p>
                Search the graph, land on the exact node, and keep tree path,
                highlight, and source reveal aligned.
              </p>
            </div>
            <div class="story-media story-media--contain story-media--wide-short">
              <img
                src={assetUrl('/landing/feature-reveal.png')}
                alt="Tracing a selected field through tree path breadcrumbs in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--copy">
            <div class="story-copy">
              <h3>Edit either side and keep both in sync.</h3>
              <p>
                Update a value in graph or source text and keep both views
                anchored to the same document state. Hover previews and local
                graph context stay nearby while you work.
              </p>
            </div>
          </article>

          <article class="capability-card capability-card--cli">
            <div class="story-copy">
              <h3>Query, convert, and visualize from the CLI.</h3>
              <p>
                Use jq-style expressions for structured data work, then open a
                readonly local graph view when the result needs visual
                structure.
              </p>
            </div>
            <div class="cli-card">
              <div class="cli-card__row">
                <span class="cli-card__hint">Query</span>
                <code>treease '.services.api.url' example.json</code>
              </div>
              <div class="cli-card__row">
                <span class="cli-card__hint">Convert</span>
                <code>treease -o yaml '.' example.json</code>
              </div>
              <div class="cli-card__row">
                <span class="cli-card__hint">Visualize</span>
                <code>treease web '.services.api' example.json</code>
              </div>
            </div>
          </article>
        </div>
      </section>

      <section class="workflow-story" id="workflow" aria-labelledby="workflow-title">
        <div class="section-copy">
          <h2 id="workflow-title">Act on structure once it is clear.</h2>
          <p>
            Treease keeps import progress, conversion preview, and compare
            decisions inside the same visual workflow, so users can verify
            structure before exporting or trusting a diff.
          </p>
        </div>

        <div class="workflow-story__grid" aria-label="Import progress, preview export, and compare structure">
          <article class="workflow-story__card workflow-story__card--progress">
            <div class="story-copy">
              <h3>Keep large JSON imports transparent.</h3>
              <p>
                See visible progress while large JSON is parsed and rendered,
                instead of waiting on a blank screen.
              </p>
            </div>
            <div class="ship-media ship-media--dark">
              <video
                src={assetUrl('/landing/workflow-progress.mp4')}
                poster={assetUrl('/landing/workflow-progress.png')}
                autoplay
                muted
                loop
                playsinline
                preload="auto"
                aria-label="Streaming graph progress while importing a 2MB JSON file in Treease."
              ></video>
            </div>
          </article>

          <article class="workflow-story__card workflow-story__card--export">
            <div class="story-copy">
              <h3>Preview converted output before download.</h3>
              <p>
                Confirm the target format in the same workspace before the file
                leaves the browser.
              </p>
            </div>
            <div class="ship-media ship-media--contain">
              <img
                src={assetUrl('/landing/workflow-export.png')}
                alt="Previewing converted YAML output before export in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="workflow-story__card workflow-story__card--compare">
            <div class="story-copy">
              <h3>Compare structure before you trust the diff.</h3>
              <p>
                Prefer structural comparison first. Fall back to text diff only
                when structure cannot be compared safely.
              </p>
            </div>
          </article>
        </div>
      </section>

      <section class="faq-section" id="faq" aria-labelledby="faq-title">
        <div class="section-copy section-copy--compact">
          <h2 id="faq-title">FAQ</h2>
          <p>
            Treease works best when the page is explicit about what it does
            well, and what still belongs outside the graph view.
          </p>
        </div>

        <div class="faq-list">
          {#each faqItems as item}
            <details class="faq-item">
              <summary>{item.question}</summary>
              <p>{item.answer}</p>
            </details>
          {/each}
        </div>
      </section>

      <section class="cta-section" aria-labelledby="cta-title">
        <h2 id="cta-title">Try it with a real file.</h2>
        <p>
          Open a real document, see the structure, and keep every next step
          anchored to what you already understand.
        </p>
        <div class="hero-actions hero-actions--centered">
          <a class="primary-cta" href="/editor">Open editor</a>
        </div>
      </section>
    </main>

    <footer class="site-footer" aria-label="Site footer">
      <span class="site-footer__brand">© 2026 Treease</span>
      <nav class="site-footer__links" aria-label="Legal">
        {#each footerLinks as item}
          <a href={item.href}>{item.label}</a>
        {/each}
      </nav>
    </footer>
  </div>
</div>

<style>
  .landing {
    --accent: #2563eb;
    --accent-strong: #1d4ed8;
    --accent-soft: rgba(37, 99, 235, 0.12);
    --accent-tint: rgba(96, 165, 250, 0.16);
    --bg: #eff4fb;
    --bg-strong: #f7faff;
    --surface: rgba(255, 255, 255, 0.88);
    --surface-strong: #ffffff;
    --surface-muted: #edf3ff;
    --surface-dark: #0f172a;
    --ink: #0f172a;
    --muted: #4b5563;
    --muted-soft: #6b7280;
    --line: rgba(15, 23, 42, 0.1);
    --line-strong: rgba(37, 99, 235, 0.18);
    --shadow: 0 30px 80px rgba(15, 23, 42, 0.12);

    min-height: 100svh;
    background:
      radial-gradient(circle at 12% 10%, rgba(125, 211, 252, 0.2), transparent 30%),
      radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.16), transparent 34%),
      linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    color: var(--ink);
  }

  .landing-shell {
    box-sizing: border-box;
    width: min(1240px, 100%);
    margin: 0 auto;
    padding: 24px 24px 56px;
  }

  .site-header {
    display: grid;
    align-items: center;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 24px;
    min-height: 72px;
    margin-bottom: 20px;
    padding: 12px 18px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.7);
    backdrop-filter: blur(18px);
    box-shadow: 0 12px 40px rgba(15, 23, 42, 0.08);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }

  .brand-logo {
    width: 44px;
    height: 44px;
    flex: 0 0 auto;
    object-fit: contain;
  }

  .brand-copy {
    display: inline-flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand-mark {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.04em;
  }

  .brand-note {
    color: var(--muted-soft);
    font-size: 14px;
    white-space: nowrap;
  }

  .site-nav {
    display: inline-flex;
    align-items: center;
    gap: 18px;
  }

  .site-nav a {
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    transition: color 160ms ease;
  }

  .site-nav a:hover {
    color: var(--ink);
  }

  .site-footer,
  .site-footer__links {
    display: inline-flex;
    align-items: center;
    gap: 18px;
  }

  .site-footer {
    justify-content: space-between;
    margin-top: 20px;
    padding: 6px 4px 0;
    color: var(--muted-soft);
    font-size: 13px;
  }

  .site-footer__links a {
    color: inherit;
    font-weight: 600;
    text-decoration: none;
    transition: color 160ms ease;
  }

  .site-footer__links a:hover {
    color: var(--ink);
  }

  .header-cta,
  .primary-cta,
  .secondary-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 48px;
    padding: 0 22px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 700;
    text-decoration: none;
    transition:
      transform 140ms ease,
      border-color 160ms ease,
      background-color 160ms ease,
      color 160ms ease,
      box-shadow 160ms ease;
  }

  .header-cta,
  .primary-cta {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 16px 30px rgba(37, 99, 235, 0.24);
  }

  .secondary-cta {
    border: 1px solid var(--line-strong);
    background: rgba(255, 255, 255, 0.76);
    color: var(--ink);
    box-shadow: 0 14px 28px rgba(15, 23, 42, 0.08);
  }

  .header-cta:hover,
  .primary-cta:hover {
    background: var(--accent-strong);
  }

  .secondary-cta:hover {
    border-color: rgba(37, 99, 235, 0.34);
    background: rgba(255, 255, 255, 0.92);
  }

  .header-cta:active,
  .primary-cta:active,
  .secondary-cta:active {
    transform: translateY(1px) scale(0.99);
  }

  .landing-main {
    display: flex;
    flex-direction: column;
    gap: 36px;
  }

  .hero {
    display: grid;
    align-items: start;
    gap: 24px;
    grid-template-columns: minmax(0, 1fr) minmax(420px, 560px);
    min-height: min(660px, calc(100dvh - 132px));
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
    max-width: 580px;
    padding-top: 8px;
  }

  .hero-kicker {
    margin: 0;
    color: var(--accent-strong);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3 {
    margin: 0;
    font-weight: 700;
    letter-spacing: -0.05em;
  }

  h1 {
    max-width: 11ch;
    font-size: clamp(3rem, 6vw, 5.75rem);
    line-height: 0.94;
  }

  h2 {
    font-size: clamp(2rem, 3.6vw, 3rem);
    line-height: 1;
  }

  h3 {
    font-size: clamp(1.25rem, 2vw, 1.6rem);
    line-height: 1.08;
  }

  .hero-lede,
  .section-copy p,
  .story-copy p,
  .value-point p,
  .cta-section p,
  .faq-item p {
    color: var(--muted);
    font-size: 16px;
    line-height: 1.75;
  }

  .hero-lede {
    max-width: 48ch;
    margin: 0;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .hero-cta-stack {
    display: grid;
    gap: 14px;
    width: min(100%, 720px);
  }

  .cli-quickstart {
    display: grid;
    gap: 14px;
    padding: 16px 18px;
    border: 1px solid var(--line);
    border-radius: 24px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(237, 243, 255, 0.88));
    box-shadow: 0 18px 40px rgba(15, 23, 42, 0.08);
  }

  .cli-quickstart__label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cli-quickstart__label strong {
    font-size: 15px;
    line-height: 1.45;
  }

  .cli-card__hint {
    color: var(--muted-soft);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .cli-quickstart__commands,
  .cli-card {
    display: grid;
    gap: 10px;
  }

  .cli-quickstart code,
  .cli-card code {
    display: block;
    overflow-x: auto;
    padding: 12px 14px;
    border: 1px solid rgba(37, 99, 235, 0.14);
    border-radius: 16px;
    background: rgba(15, 23, 42, 0.94);
    color: #dbeafe;
    font-size: 13px;
    line-height: 1.45;
    white-space: nowrap;
  }

  .hero-demo {
    min-width: 0;
    padding-top: 6px;
  }

  .section-copy {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 64ch;
  }

  .section-copy--compact {
    max-width: 58ch;
  }

  .value-strip,
  .capabilities-section,
  .workflow-story,
  .faq-section,
  .cta-section {
    padding-top: 8px;
  }

  .value-strip {
    display: grid;
    gap: 18px;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1.4fr);
    align-items: start;
  }

  .value-points {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }

  .value-point,
  .capability-card,
  .workflow-story__card,
  .faq-item,
  .cta-section {
    border: 1px solid var(--line);
    border-radius: 28px;
    background: var(--surface);
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px);
  }

  .value-point {
    padding: 20px 22px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(237, 243, 255, 0.94));
  }

  .value-point p,
  .story-copy p,
  .faq-item p {
    margin: 0;
  }

  .capability-grid {
    display: grid;
    gap: 18px;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.8fr);
    margin-top: 28px;
  }

  .capability-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
    padding: 24px;
  }

  .capability-card--hero {
    background:
      linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(255, 255, 255, 0.86)),
      var(--surface);
  }

  .capability-card--soft {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(237, 243, 255, 0.92));
  }

  .capability-card--surface {
    background: rgba(255, 255, 255, 0.9);
  }

  .capability-card--dark {
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(30, 41, 59, 0.96));
    color: #f8fafc;
  }

  .capability-card--dark .story-copy p {
    color: rgba(226, 232, 240, 0.84);
  }

  .capability-card--copy {
    align-self: start;
    justify-content: flex-start;
    min-height: 0;
    background:
      radial-gradient(circle at top right, rgba(96, 165, 250, 0.16), transparent 26%),
      var(--surface);
  }

  .capability-card--cli {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(236, 245, 255, 0.92));
  }

  .story-copy,
  .workflow-story__card .story-copy {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .cli-card {
    margin-top: auto;
    padding: 18px;
    border: 1px solid rgba(37, 99, 235, 0.14);
    border-radius: 22px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(226, 232, 240, 0.88));
  }

  .cli-card__row {
    display: grid;
    gap: 8px;
  }

  .story-media {
    position: relative;
    overflow: hidden;
    min-height: 228px;
    border-radius: 22px;
    border: 1px solid rgba(37, 99, 235, 0.14);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(226, 232, 240, 0.9));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.45);
  }

  .story-media--dark {
    border-color: rgba(148, 163, 184, 0.18);
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.92), rgba(30, 41, 59, 0.88));
  }

  .story-media img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: top left;
  }

  .story-media--contain img {
    object-fit: contain;
    background: transparent;
  }

  .story-media--wide-short {
    min-height: 0;
    aspect-ratio: 2.15 / 1;
  }

  .workflow-story__grid {
    display: grid;
    gap: 18px;
    grid-template-columns: minmax(0, 1.45fr) minmax(320px, 0.95fr);
    margin-top: 28px;
  }

  .workflow-story__card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 24px;
  }

  .workflow-story__card--progress {
    grid-row: span 2;
    background: rgba(255, 255, 255, 0.84);
  }

  .workflow-story__card--export {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(236, 245, 255, 0.92));
  }

  .workflow-story__card--compare {
    justify-content: center;
    min-height: 0;
    background:
      linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(255, 255, 255, 0.88)),
      var(--surface);
  }

  .ship-media {
    position: relative;
    overflow: hidden;
    min-height: 208px;
    border-radius: 22px;
    border: 1px solid rgba(37, 99, 235, 0.14);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(226, 232, 240, 0.9));
  }

  .ship-media--dark {
    border-color: rgba(148, 163, 184, 0.18);
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.92), rgba(30, 41, 59, 0.88));
  }

  .ship-media img,
  .ship-media video {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: top left;
  }

  .ship-media--contain {
    min-height: 152px;
  }

  .ship-media--contain img {
    object-fit: contain;
  }

  .faq-list {
    display: grid;
    gap: 14px;
    margin-top: 28px;
  }

  .faq-item {
    padding: 20px 24px;
  }

  .faq-item summary {
    cursor: pointer;
    list-style: none;
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    line-height: 1.4;
  }

  .faq-item summary::-webkit-details-marker {
    display: none;
  }

  .faq-item p {
    margin-top: 12px;
  }

  .cta-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 42px 28px;
    text-align: center;
    background:
      linear-gradient(135deg, rgba(37, 99, 235, 0.12), rgba(255, 255, 255, 0.84)),
      var(--surface);
  }

  .cta-section p {
    max-width: 44ch;
    margin: 0;
  }

  .hero-actions--centered {
    justify-content: center;
  }

  @media (max-width: 1120px) {
    .hero,
    .value-strip,
    .capability-grid,
    .workflow-story__grid {
      grid-template-columns: 1fr;
    }

    .hero {
      min-height: auto;
    }

    .workflow-story__card--progress {
      grid-row: auto;
    }
  }

  @media (max-width: 860px) {
    .landing-shell {
      padding: 18px 18px 42px;
    }

    .site-header {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      padding: 12px 16px;
    }

    .site-nav,
    .brand-note {
      display: none;
    }

    h1 {
      max-width: 12ch;
      font-size: clamp(2.6rem, 10vw, 4.2rem);
      line-height: 0.97;
    }

    h2 {
      font-size: clamp(1.8rem, 8vw, 2.5rem);
    }

    .value-points {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 640px) {
    .site-header {
      min-height: 64px;
      border-radius: 24px;
    }

    .site-footer {
      flex-direction: column;
      align-items: flex-start;
      gap: 10px;
      padding-left: 0;
      padding-right: 0;
    }

    .header-cta,
    .primary-cta,
    .secondary-cta {
      width: 100%;
    }

    .hero-actions {
      width: 100%;
      flex-direction: column;
    }

    .value-point,
    .capability-card,
    .workflow-story__card,
    .cta-section,
    .faq-item {
      padding: 20px;
      border-radius: 24px;
    }

    .story-media {
      min-height: 180px;
      border-radius: 18px;
    }

    .story-media--wide-short {
      min-height: 0;
      aspect-ratio: 2 / 1;
    }

    .ship-media {
      min-height: 180px;
      border-radius: 18px;
    }

    .ship-media--contain {
      min-height: 146px;
    }
  }

  @media (prefers-color-scheme: dark) {
    .landing {
      --accent: #60a5fa;
      --accent-strong: #93c5fd;
      --accent-soft: rgba(96, 165, 250, 0.16);
      --accent-tint: rgba(59, 130, 246, 0.2);
      --bg: #07111f;
      --bg-strong: #0b1526;
      --surface: rgba(13, 23, 38, 0.78);
      --surface-strong: rgba(15, 23, 42, 0.96);
      --surface-muted: rgba(18, 29, 47, 0.94);
      --surface-dark: #020617;
      --ink: #e5eefc;
      --muted: #b7c5dd;
      --muted-soft: #8ea4c4;
      --line: rgba(148, 163, 184, 0.18);
      --line-strong: rgba(96, 165, 250, 0.28);
      --shadow: 0 28px 70px rgba(2, 6, 23, 0.42);

      background:
        radial-gradient(circle at 10% 8%, rgba(14, 165, 233, 0.16), transparent 28%),
        radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.22), transparent 34%),
        linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    }

    .site-header,
    .faq-item {
      background: rgba(11, 20, 36, 0.72);
    }

    .site-header {
      box-shadow: 0 16px 36px rgba(2, 6, 23, 0.34);
    }

    .value-point,
    .capability-card--hero,
    .capability-card--cli,
    .workflow-story__card--compare,
    .cta-section {
      background:
        linear-gradient(135deg, rgba(37, 99, 235, 0.2), rgba(11, 20, 36, 0.84)),
        var(--surface);
    }

    .capability-card--soft,
    .capability-card--surface,
    .workflow-story__card--export,
    .workflow-story__card--progress {
      background: rgba(11, 20, 36, 0.84);
    }

    .cli-quickstart,
    .cli-card {
      background: rgba(11, 20, 36, 0.84);
      box-shadow: 0 18px 44px rgba(2, 6, 23, 0.24);
    }

    .cli-quickstart code,
    .cli-card code {
      border-color: rgba(148, 163, 184, 0.18);
      background: rgba(2, 6, 23, 0.92);
      color: #dbeafe;
    }

    .faq-item summary {
      color: var(--ink);
    }
  }
</style>
