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
    --accent: #2d63e2;
    --accent-strong: #1745b5;
    --accent-soft: rgba(45, 99, 226, 0.1);
    --accent-tint: rgba(124, 160, 255, 0.18);
    --bg: #f4f7fb;
    --bg-strong: #fbfdff;
    --bg-muted: #e9eef6;
    --surface: rgba(255, 255, 255, 0.92);
    --surface-strong: #ffffff;
    --surface-muted: #f0f4fa;
    --surface-dark: #0f172a;
    --ink: #10192a;
    --muted: #536273;
    --muted-soft: #718196;
    --line: rgba(16, 25, 42, 0.1);
    --line-strong: rgba(45, 99, 226, 0.2);
    --shadow: 0 26px 60px rgba(18, 29, 46, 0.08);
    --font-sans: "SF Pro Text", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Segoe UI", sans-serif;
    --font-display: "Avenir Next", "SF Pro Display", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
    --font-mono: "SF Mono", "SFMono-Regular", Menlo, Consolas, monospace;

    position: relative;
    isolation: isolate;
    min-height: 100svh;
    background:
      radial-gradient(circle at 100% 0%, rgba(45, 99, 226, 0.08), transparent 24%),
      radial-gradient(circle at 0% 18%, rgba(125, 160, 255, 0.1), transparent 24%),
      linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    color: var(--ink);
    font-family: var(--font-sans);
  }

  .landing::before {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(90deg, rgba(16, 25, 42, 0.035) 1px, transparent 1px),
      linear-gradient(rgba(16, 25, 42, 0.035) 1px, transparent 1px);
    background-size: 88px 88px;
    mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent 72%);
  }

  .landing-shell {
    box-sizing: border-box;
    position: relative;
    z-index: 1;
    width: min(1220px, 100%);
    margin: 0 auto;
    padding: 28px 24px 72px;
  }

  .site-header {
    display: grid;
    align-items: center;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 24px;
    min-height: 68px;
    margin-bottom: 42px;
    padding: 0 0 18px;
    border-bottom: 1px solid var(--line);
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
    filter: drop-shadow(0 10px 18px rgba(45, 99, 226, 0.16));
  }

  .brand-copy {
    display: inline-flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand-mark {
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }

  .brand-note {
    color: var(--muted-soft);
    font-size: 13px;
    letter-spacing: 0.02em;
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
    color: var(--accent-strong);
  }

  .site-footer,
  .site-footer__links {
    display: inline-flex;
    align-items: center;
    gap: 18px;
  }

  .site-footer {
    justify-content: space-between;
    margin-top: 34px;
    padding: 12px 0 0;
    color: var(--muted-soft);
    font-size: 13px;
    border-top: 1px solid var(--line);
  }

  .site-footer__links a {
    color: inherit;
    font-weight: 600;
    text-decoration: none;
    transition: color 160ms ease;
  }

  .site-footer__links a:hover {
    color: var(--accent-strong);
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
    background: linear-gradient(135deg, var(--accent) 0%, #4a7dff 100%);
    color: #fff;
    box-shadow: 0 16px 30px rgba(45, 99, 226, 0.22);
  }

  .secondary-cta {
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.88);
    color: var(--ink);
    box-shadow: 0 10px 24px rgba(15, 23, 42, 0.06);
  }

  .header-cta:hover,
  .primary-cta:hover {
    transform: translateY(-1px);
    box-shadow: 0 18px 34px rgba(45, 99, 226, 0.28);
  }

  .secondary-cta:hover {
    border-color: rgba(45, 99, 226, 0.32);
    background: #ffffff;
  }

  .header-cta:active,
  .primary-cta:active,
  .secondary-cta:active {
    transform: translateY(1px) scale(0.99);
  }

  .landing-main {
    display: flex;
    flex-direction: column;
    gap: 72px;
  }

  .hero {
    display: grid;
    align-items: center;
    gap: clamp(32px, 5vw, 72px);
    grid-template-columns: minmax(0, 0.84fr) minmax(420px, 1.16fr);
    min-height: min(620px, calc(100dvh - 168px));
    padding-top: 0;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 18px;
    max-width: 520px;
    padding-top: 0;
  }

  .hero-kicker {
    margin: 0;
    color: var(--accent-strong);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3 {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: -0.04em;
    text-wrap: balance;
  }

  h1 {
    max-width: 8.4ch;
    font-size: clamp(3.45rem, 7vw, 5.9rem);
    line-height: 0.9;
  }

  h2 {
    max-width: 11ch;
    font-size: clamp(2.1rem, 4vw, 3.2rem);
    line-height: 0.98;
  }

  h3 {
    font-size: clamp(1.28rem, 2.2vw, 1.72rem);
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
    text-wrap: pretty;
  }

  .hero-lede {
    max-width: 38ch;
    margin: 0;
    font-size: 18px;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }

  .hero-cta-stack {
    display: grid;
    gap: 14px;
    width: min(100%, 640px);
  }

  .cli-quickstart {
    display: grid;
    gap: 14px;
    padding: 18px 20px;
    border: 1px solid var(--line);
    border-radius: 20px;
    background: #fff;
    box-shadow: 0 18px 40px rgba(15, 23, 42, 0.06);
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
    font-family: var(--font-mono);
    display: block;
    overflow-x: auto;
    padding: 12px 14px;
    border: 1px solid rgba(15, 23, 42, 0.08);
    border-radius: 14px;
    background: rgba(15, 23, 42, 0.94);
    color: #dbeafe;
    font-size: 13px;
    line-height: 1.45;
    white-space: nowrap;
  }

  .hero-demo {
    min-width: 0;
    position: relative;
    padding: 0;
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
    padding-top: 6px;
  }

  .value-strip {
    display: grid;
    gap: 24px;
    grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
    align-items: start;
    padding-top: 24px;
    border-top: 1px solid var(--line);
  }

  .value-points {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }

  .value-point,
  .capability-card,
  .workflow-story__card,
  .faq-item,
  .cta-section {
    border: 1px solid var(--line);
    border-radius: 22px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .value-point {
    padding: 20px 20px 18px;
    background: linear-gradient(180deg, #ffffff 0%, #f7faff 100%);
  }

  .value-point p,
  .story-copy p,
  .faq-item p {
    margin: 0;
  }

  .capability-grid {
    display: grid;
    gap: 18px;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    margin-top: 34px;
  }

  .capability-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
    padding: 24px;
  }

  .capability-card--hero {
    grid-column: span 7;
    grid-row: span 2;
    background:
      linear-gradient(180deg, #ffffff 0%, #f3f7ff 100%);
  }

  .capability-card--soft {
    grid-column: span 5;
    background: #fbfcff;
  }

  .capability-card--surface {
    grid-column: span 5;
    background: linear-gradient(180deg, #f6f9fd 0%, #eef3fa 100%);
  }

  .capability-card--dark {
    grid-column: span 7;
    background:
      linear-gradient(180deg, #16233a 0%, #0f172a 100%);
    color: #f8fafc;
  }

  .capability-card--dark .story-copy p {
    color: rgba(226, 232, 240, 0.84);
  }

  .capability-card--copy {
    grid-column: span 4;
    align-self: start;
    justify-content: center;
    min-height: 100%;
    padding-top: 34px;
    background:
      radial-gradient(circle at top right, rgba(96, 165, 250, 0.14), transparent 26%),
      #fcfdff;
  }

  .capability-card--cli {
    grid-column: span 8;
    background: linear-gradient(180deg, #ffffff 0%, #f2f6fd 100%);
  }

  .capability-card--copy h3,
  .workflow-story__card--compare h3 {
    max-width: 12ch;
  }

  .capability-card--copy .story-copy,
  .workflow-story__card--compare .story-copy {
    gap: 14px;
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
    border: 1px solid rgba(16, 25, 42, 0.08);
    border-radius: 18px;
    background: #eef3fa;
  }

  .cli-card__row {
    display: grid;
    gap: 8px;
  }

  .story-media {
    position: relative;
    overflow: hidden;
    min-height: 228px;
    border-radius: 18px;
    border: 1px solid rgba(16, 25, 42, 0.08);
    background: #eff4fb;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.5);
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
    aspect-ratio: 2.1 / 1;
  }

  .workflow-story__grid {
    display: grid;
    gap: 20px;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    margin-top: 34px;
  }

  .workflow-story__card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .workflow-story__card--progress {
    grid-column: span 7;
    grid-row: span 2;
  }

  .workflow-story__card--export {
    grid-column: span 5;
  }

  .workflow-story__card--compare {
    grid-column: span 5;
    justify-content: center;
    min-height: 240px;
    padding: 30px 0 0 22px;
    border-left: 1px solid rgba(45, 99, 226, 0.2);
  }

  .workflow-story__card--progress .story-copy,
  .workflow-story__card--export .story-copy {
    padding-bottom: 14px;
  }

  .ship-media {
    position: relative;
    overflow: hidden;
    min-height: 208px;
    border-radius: 18px;
    border: 1px solid rgba(16, 25, 42, 0.08);
    background: #eef3fa;
    box-shadow: 0 20px 44px rgba(16, 25, 42, 0.08);
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

  .workflow-story__card--progress .ship-media {
    min-height: 420px;
    background: linear-gradient(180deg, #f7faff 0%, #edf3fb 100%);
  }

  .workflow-story__card--export .ship-media {
    min-height: 244px;
    background: linear-gradient(180deg, #ffffff 0%, #f2f6fd 100%);
  }

  .faq-list {
    display: grid;
    gap: 0;
    margin-top: 28px;
    border-top: 1px solid var(--line);
  }

  .faq-item {
    padding: 22px 0;
    border: 0;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .faq-item summary {
    cursor: pointer;
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    line-height: 1.4;
  }

  .faq-item summary::after {
    content: '+';
    flex: 0 0 auto;
    color: var(--accent-strong);
    font-size: 24px;
    line-height: 1;
  }

  .faq-item[open] summary::after {
    content: '−';
  }

  .faq-item summary::-webkit-details-marker {
    display: none;
  }

  .faq-item p {
    max-width: 74ch;
    margin-top: 14px;
  }

  .cta-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 54px 28px;
    text-align: center;
    background:
      linear-gradient(135deg, rgba(45, 99, 226, 0.14), rgba(255, 255, 255, 0.95));
  }

  .cta-section p {
    max-width: 44ch;
    margin: 0;
  }

  .hero-actions--centered {
    justify-content: center;
  }

  .hero-demo :global(.deck-stage) {
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 18px 34px rgba(16, 25, 42, 0.12));
    transform: translateY(18px);
  }

  .site-nav a:focus-visible,
  .header-cta:focus-visible,
  .primary-cta:focus-visible,
  .secondary-cta:focus-visible,
  .faq-item summary:focus-visible,
  .site-footer__links a:focus-visible {
    outline: 2px solid var(--accent-strong);
    outline-offset: 3px;
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

    .hero-demo {
      padding: 0;
    }

    .hero-demo :global(.deck-stage) {
      transform: none;
    }

    .workflow-story__card--progress {
      grid-row: auto;
    }

    .capability-card--hero {
      grid-row: auto;
    }

    .capability-card--hero,
    .capability-card--soft,
    .capability-card--surface,
    .capability-card--dark,
    .capability-card--copy,
    .capability-card--cli,
    .workflow-story__card--progress,
    .workflow-story__card--export,
    .workflow-story__card--compare {
      grid-column: span 12;
    }

    .workflow-story__card--compare {
      min-height: 0;
      padding: 18px 0 0;
      border-left: 0;
      border-top: 1px solid rgba(45, 99, 226, 0.2);
    }
  }

  @media (max-width: 860px) {
    .landing-shell {
      padding: 20px 18px 46px;
    }

    .site-header {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      margin-bottom: 28px;
      padding: 0 0 14px;
    }

    .site-nav,
    .brand-note {
      display: none;
    }

    h1 {
      max-width: 12ch;
      font-size: clamp(2.8rem, 11vw, 4.3rem);
      line-height: 0.95;
    }

    h2 {
      font-size: clamp(1.9rem, 8vw, 2.6rem);
    }

    .value-points {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 640px) {
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
      border-radius: 20px;
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

    .workflow-story__card--progress .ship-media {
      min-height: 220px;
    }

    .workflow-story__card--export .ship-media {
      min-height: 180px;
    }

    .faq-item {
      padding-left: 0;
      padding-right: 0;
      border-radius: 0;
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
        radial-gradient(circle at 10% 8%, rgba(14, 165, 233, 0.12), transparent 28%),
        radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.16), transparent 34%),
        linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    }

    .landing::before {
      background:
        linear-gradient(90deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px),
        linear-gradient(rgba(255, 255, 255, 0.045) 1px, transparent 1px);
    }

    .value-point,
    .capability-card--hero,
    .capability-card--soft,
    .capability-card--surface,
    .capability-card--copy,
    .capability-card--cli,
    .workflow-story__card--progress,
    .workflow-story__card--export,
    .workflow-story__card--compare,
    .cta-section {
      background: rgba(11, 20, 36, 0.82);
    }

    .story-media,
    .ship-media,
    .cli-card {
      background: rgba(15, 23, 42, 0.92);
    }

    .faq-item {
      background: transparent;
    }

    .cli-quickstart {
      background: rgba(11, 20, 36, 0.84);
    }

    .workflow-story__card--compare {
      border-left-color: rgba(147, 197, 253, 0.28);
      border-top-color: rgba(147, 197, 253, 0.28);
    }

    .workflow-story__card--progress .ship-media,
    .workflow-story__card--export .ship-media {
      background: rgba(11, 20, 36, 0.9);
      box-shadow: 0 24px 50px rgba(2, 6, 23, 0.24);
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

  @media (prefers-reduced-motion: reduce) {
    .header-cta,
    .primary-cta,
    .secondary-cta {
      transition: none;
    }
  }
</style>
