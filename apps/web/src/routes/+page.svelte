<script lang="ts">
  import HomeDemo from '$lib/components/HomeDemo.svelte';

  const navItems = [
    { href: '#workflow', label: 'Workflow' },
    { href: '#sync', label: 'Sync' },
    { href: '#ship', label: 'Compare + CLI' }
  ];

  const formats = ['JSON', 'YAML', 'TOML', 'CSV', 'JavaScript', 'Python', 'Web UI', 'CLI'];

  const syncPoints = [
    'Click a graph node and jump back to the exact source range.',
    'Move the cursor onto a JSON block inside logs and inspect only that container.',
    'Edit values in graph view or text view and keep one document in play.',
    'Preview URL, color, date, Base64, JWT, and image values without leaving the editor.'
  ];
</script>

<div class="landing">
  <div class="landing-shell">
    <header class="site-header">
      <a class="brand" href="#top" aria-label="Treease home">
        <span class="brand-mark">Treease</span>
        <span class="brand-note">Structured text + graph workspace</span>
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
          <p class="hero-kicker">Structured text workspace</p>
          <h1 id="hero-title">Read, change, and compare structured text with graph context.</h1>
          <p class="hero-lede">
            Import JSON, YAML, TOML, CSV, JavaScript, or Python data, then move
            between text, graph, compare, and export in one flow.
          </p>
          <div class="hero-actions">
            <a class="primary-cta" href="/editor">Open editor</a>
            <a class="secondary-cta" href="#workflow">See workflow</a>
          </div>
        </div>

        <div class="hero-demo">
          <HomeDemo />
        </div>
      </section>

      <section class="format-rail" aria-label="Supported formats and surfaces">
        {#each formats as format}
          <span>{format}</span>
        {/each}
      </section>

      <section class="workflow-section" id="workflow" aria-labelledby="workflow-title">
        <div class="section-copy">
          <h2 id="workflow-title">Built around the way structured files are actually handled.</h2>
          <p>
            Open a file, inspect the active JSON block, reveal fields, preview
            values, and keep edits tied to the same source.
          </p>
        </div>

        <div class="workflow-grid" aria-label="Treease workflow highlights">
          <article class="story-card story-card--wide story-card--accent">
            <div class="story-copy">
              <h3>Import local files and start in the source editor.</h3>
              <p>
                Choose or drop JSON, YAML, TOML, CSV, JavaScript object, or
                Python dict input and let Treease pick the right parser.
              </p>
            </div>
            <div class="file-rail" aria-hidden="true">
              <span>config.production.yaml</span>
              <span>audit-report.csv</span>
              <span>payload.jsonl</span>
              <span>session.js</span>
            </div>
          </article>

          <article class="story-card story-card--dark">
            <div class="story-copy">
              <h3>Lift the active JSON container out of mixed logs.</h3>
              <p>
                Move the cursor onto a JSONL row or embedded payload and let
                graph view focus on that block.
              </p>
            </div>
            <pre class="mini-code" aria-hidden="true">{`2026-06-08 10:14:22 INFO ingest ok
{"event":"sync","meta":{"branch":"main"}}
2026-06-08 10:14:23 INFO next step`}</pre>
          </article>

          <article class="story-card story-card--surface">
            <div class="story-copy">
              <h3>Reveal fields through graph search and tree path.</h3>
              <p>
                Click a node, copy the current path, or jump to a parent path
                without manual scanning.
              </p>
            </div>
            <div class="path-preview" aria-hidden="true">
              <span>root</span>
              <span>services</span>
              <span>api</span>
              <span>retries</span>
            </div>
          </article>

          <article class="story-card story-card--surface story-card--soft">
            <div class="story-copy">
              <h3>Preview values in place.</h3>
              <p>
                Spot colors, dates, URLs, JWTs, Base64, and images without
                leaving the document.
              </p>
            </div>
            <div class="value-preview" aria-hidden="true">
              <span class="value-chip value-chip--color">#2563eb</span>
              <span class="value-chip">2026-06-08</span>
              <span class="value-chip">docs.example.dev</span>
              <span class="value-chip">jwt.header.payload</span>
            </div>
          </article>

          <article class="story-card story-card--wide story-card--ink">
            <div class="story-copy">
              <h3>Edit once and keep text and graph in sync.</h3>
              <p>
                Graph value changes write back to source text. Text edits refresh
                graph structure and keep failure feedback explicit.
              </p>
            </div>
            <div class="sync-preview" aria-hidden="true">
              <div>
                <span class="preview-label">Source</span>
                <pre>{`"retries": 2
"theme": "blue"`}</pre>
              </div>
              <div>
                <span class="preview-label">After graph edit</span>
                <pre>{`"retries": 3
"theme": "blue"`}</pre>
              </div>
            </div>
          </article>
        </div>
      </section>

      <section class="sync-section" id="sync" aria-labelledby="sync-title">
        <div class="section-copy section-copy--narrow">
          <h2 id="sync-title">Text, graph, and tree path stay on the same document.</h2>
          <p>
            The page is not selling a detached diagram. It is selling one editing
            loop that stays anchored while you inspect structure.
          </p>
          <ul class="sync-list">
            {#each syncPoints as point}
              <li>{point}</li>
            {/each}
          </ul>
        </div>

        <div class="sync-board" aria-label="Linked editor and graph behaviors">
          <article>
            <span class="board-title">Selection reveal</span>
            <p>
              Graph clicks return a precise source selection instead of forcing a
              second search.
            </p>
          </article>
          <article>
            <span class="board-title">Value feedback</span>
            <p>
              Supported value edits commit back to text. Invalid edits surface a
              clear reason.
            </p>
          </article>
          <article>
            <span class="board-title">Hover context</span>
            <p>
              Local previews stay contextual so users can inspect values without
              losing their current place.
            </p>
          </article>
        </div>
      </section>

      <section class="ship-section" id="ship" aria-labelledby="ship-title">
        <div class="section-copy">
          <h2 id="ship-title">From quick edits to review and automation.</h2>
          <p>
            Compare structure before download, watch large imports stream into
            graph view, then move the same flow into the terminal.
          </p>
        </div>

        <div class="ship-grid" aria-label="Compare export and CLI workflows">
          <article class="ship-card ship-card--compare">
            <div class="story-copy">
              <h3>Compare structure before you trust the diff.</h3>
              <p>
                Treease prefers structural comparison, then falls back to text
                diff when parsing is not possible.
              </p>
            </div>
            <pre class="diff-snippet" aria-hidden="true">- retries: 2
+ retries: 3
  mode: graph
+ export: yaml</pre>
          </article>

          <article class="ship-card ship-card--export">
            <div class="story-copy">
              <h3>Preview converted output before download.</h3>
              <p>
                Confirm the target format in the same workspace before the file
                leaves the browser.
              </p>
            </div>
            <pre class="mini-code" aria-hidden="true">project: treease
formats:
  - json
  - yaml
graph:
  sync: true</pre>
          </article>

          <article class="ship-card ship-card--progress">
            <div class="story-copy">
              <h3>Keep large imports transparent.</h3>
              <p>
                Stream graph progress in batches so users know whether the file is
                still building or already ready.
              </p>
            </div>
            <ol class="progress-list" aria-hidden="true">
              <li><span>Parse source text</span><strong>ready</strong></li>
              <li><span>Stream graph nodes</span><strong>running</strong></li>
              <li><span>Reveal final graph</span><strong>next</strong></li>
            </ol>
          </article>

          <article class="ship-card ship-card--cli">
            <div class="story-copy">
              <h3>Use the same model in scripts.</h3>
              <p>
                Keep the browser for exploration, then move the repeatable step
                into <code>treease</code> when the job belongs in a terminal.
              </p>
            </div>
            <pre class="terminal-snippet" aria-hidden="true">{`treease '.a.b' file.yaml`}</pre>
          </article>
        </div>
      </section>

      <section class="cta-section" aria-labelledby="cta-title">
        <h2 id="cta-title">Open the editor and work on a real file.</h2>
        <p>
          Start with source text, keep graph context close, and export only after
          the structure looks right.
        </p>
        <div class="hero-actions hero-actions--centered">
          <a class="primary-cta" href="/editor">Open editor</a>
          <a class="secondary-cta" href="#workflow">See workflow</a>
        </div>
      </section>
    </main>
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

    height: 100%;
    overflow-y: auto;
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

  .header-cta:hover,
  .primary-cta:hover {
    background: var(--accent-strong);
  }

  .header-cta:active,
  .primary-cta:active,
  .secondary-cta:active {
    transform: translateY(1px) scale(0.99);
  }

  .secondary-cta {
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.62);
    color: var(--ink);
  }

  .secondary-cta:hover {
    border-color: var(--line-strong);
    background: rgba(255, 255, 255, 0.88);
  }

  .landing-main {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .hero {
    display: grid;
    align-items: center;
    gap: 28px;
    grid-template-columns: minmax(0, 1fr) minmax(420px, 560px);
    min-height: min(760px, calc(100dvh - 152px));
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 20px;
    padding: 8px 0;
  }

  .hero-kicker {
    margin: 0;
    color: var(--accent-strong);
    font-size: 13px;
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
    max-width: 12ch;
    font-size: clamp(3rem, 6vw, 5.75rem);
    line-height: 0.94;
  }

  .hero-lede,
  .section-copy p,
  .story-copy p,
  .sync-list li,
  .sync-board p,
  .cta-section p {
    color: var(--muted);
    font-size: 16px;
    line-height: 1.75;
  }

  .hero-lede {
    max-width: 56ch;
    margin: 0;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .hero-demo {
    min-width: 0;
  }

  .format-rail {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 8px 0 2px;
  }

  .format-rail span {
    padding: 10px 14px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.6);
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
  }

  .workflow-section,
  .sync-section,
  .ship-section,
  .cta-section {
    padding-top: 12px;
  }

  .section-copy {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 68ch;
    margin-bottom: 24px;
  }

  .section-copy--narrow {
    margin-bottom: 0;
  }

  h2 {
    font-size: clamp(2rem, 3.6vw, 3rem);
    line-height: 1;
  }

  h3 {
    font-size: clamp(1.25rem, 2vw, 1.6rem);
    line-height: 1.08;
  }

  .workflow-grid {
    display: grid;
    gap: 18px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .story-card,
  .ship-card,
  .sync-board article,
  .cta-section {
    border: 1px solid var(--line);
    border-radius: 28px;
    background: var(--surface);
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px);
  }

  .story-card,
  .ship-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
    padding: 24px;
  }

  .story-card--wide {
    grid-column: span 2;
  }

  .story-card--accent {
    background:
      linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(255, 255, 255, 0.86)),
      var(--surface);
  }

  .story-card--dark {
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(30, 41, 59, 0.96));
    color: #f8fafc;
  }

  .story-card--dark .story-copy p,
  .story-card--dark .mini-code {
    color: rgba(226, 232, 240, 0.84);
  }

  .story-card--ink {
    background:
      radial-gradient(circle at top right, rgba(96, 165, 250, 0.14), transparent 26%),
      var(--surface);
  }

  .story-card--soft {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(237, 243, 255, 0.92));
  }

  .file-rail,
  .path-preview,
  .value-preview {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .file-rail span,
  .value-chip {
    padding: 10px 14px;
    border-radius: 18px;
    font-size: 13px;
    font-weight: 600;
  }

  .file-rail span {
    border: 1px solid rgba(37, 99, 235, 0.14);
    background: rgba(255, 255, 255, 0.84);
    color: var(--ink);
  }

  .mini-code,
  .sync-preview pre,
  .diff-snippet,
  .terminal-snippet {
    margin: 0;
    padding: 18px 20px;
    border-radius: 20px;
    background: rgba(15, 23, 42, 0.94);
    color: #dbeafe;
    font-size: 13px;
    line-height: 1.7;
    overflow: auto;
  }

  .path-preview span {
    position: relative;
    padding: 10px 16px;
    border-radius: 18px;
    background: rgba(37, 99, 235, 0.08);
    color: var(--accent-strong);
    font-size: 13px;
    font-weight: 700;
  }

  .value-chip {
    border: 1px solid rgba(37, 99, 235, 0.12);
    background: rgba(255, 255, 255, 0.82);
    color: var(--ink);
  }

  .value-chip--color {
    color: #2563eb;
  }

  .sync-preview {
    display: grid;
    gap: 14px;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .preview-label,
  .board-title {
    display: inline-block;
    margin-bottom: 10px;
    color: var(--muted-soft);
    font-size: 13px;
    font-weight: 700;
  }

  .sync-section {
    display: grid;
    gap: 18px;
    grid-template-columns: minmax(0, 1.1fr) minmax(320px, 0.9fr);
    align-items: start;
  }

  .sync-list {
    display: grid;
    gap: 10px;
    margin: 0;
    padding-left: 18px;
  }

  .sync-board {
    display: grid;
    gap: 18px;
  }

  .sync-board article {
    padding: 24px;
  }

  .sync-board p {
    margin: 0;
  }

  .ship-grid {
    display: grid;
    gap: 18px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .ship-card--compare {
    grid-column: span 2;
    background:
      linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(255, 255, 255, 0.88)),
      var(--surface);
  }

  .ship-card--export {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(236, 245, 255, 0.92));
  }

  .ship-card--progress {
    background: rgba(255, 255, 255, 0.84);
  }

  .ship-card--cli {
    grid-column: 1 / -1;
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.98), rgba(17, 24, 39, 0.98));
    color: #f8fafc;
  }

  .ship-card--cli .story-copy p,
  .ship-card--cli code {
    color: rgba(226, 232, 240, 0.84);
  }

  .diff-snippet {
    background: rgba(248, 250, 252, 0.92);
    color: #0f172a;
    border: 1px solid rgba(37, 99, 235, 0.12);
  }

  .progress-list {
    display: grid;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .progress-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 14px 16px;
    border-radius: 18px;
    background: rgba(37, 99, 235, 0.08);
    color: var(--ink);
    font-size: 14px;
    font-weight: 600;
  }

  .progress-list strong {
    color: var(--accent-strong);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .cta-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 36px 28px;
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

  .story-copy,
  .ship-card .story-copy {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .story-copy p,
  .ship-card .story-copy p {
    margin: 0;
  }

  .ship-card code {
    font-family: 'SF Mono', 'Fira Code', 'Fira Mono', Menlo, Consolas, monospace;
    font-size: 0.95em;
  }

  @media (max-width: 1080px) {
    .hero {
      grid-template-columns: 1fr;
      min-height: auto;
    }

    .hero-copy {
      order: 1;
    }

    .hero-demo {
      order: 2;
    }

    .sync-section,
    .ship-grid,
    .workflow-grid {
      grid-template-columns: 1fr;
    }

    .story-card--wide,
    .ship-card--compare,
    .ship-card--cli {
      grid-column: auto;
    }

    .sync-preview {
      grid-template-columns: 1fr;
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
      max-width: 14ch;
      font-size: clamp(2.6rem, 10vw, 4.2rem);
      line-height: 0.97;
    }

    h2 {
      font-size: clamp(1.8rem, 8vw, 2.5rem);
    }

    .mini-code,
    .sync-preview pre,
    .diff-snippet,
    .terminal-snippet {
      padding: 16px;
    }
  }

  @media (max-width: 640px) {
    .site-header {
      position: static;
      min-height: 64px;
      border-radius: 24px;
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

    .story-card,
    .ship-card,
    .sync-board article,
    .cta-section {
      padding: 20px;
      border-radius: 24px;
    }

    .file-rail,
    .path-preview,
    .value-preview,
    .format-rail {
      gap: 8px;
    }

    .progress-list li {
      align-items: flex-start;
      flex-direction: column;
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
    .secondary-cta,
    .format-rail span,
    .file-rail span,
    .value-chip,
    .diff-snippet {
      background: rgba(11, 20, 36, 0.72);
    }

    .site-header {
      box-shadow: 0 16px 36px rgba(2, 6, 23, 0.34);
    }

    .story-card--accent,
    .ship-card--compare,
    .cta-section {
      background:
        linear-gradient(135deg, rgba(37, 99, 235, 0.2), rgba(11, 20, 36, 0.84)),
        var(--surface);
    }

    .story-card--soft,
    .ship-card--export,
    .ship-card--progress {
      background: rgba(11, 20, 36, 0.84);
    }

    .diff-snippet {
      color: #dbeafe;
      border-color: rgba(96, 165, 250, 0.2);
    }

    .path-preview span,
    .progress-list li {
      background: rgba(37, 99, 235, 0.14);
    }

    .file-rail span {
      color: var(--ink);
      border-color: rgba(96, 165, 250, 0.16);
    }
  }
</style>
