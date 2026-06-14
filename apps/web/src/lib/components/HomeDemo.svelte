<script lang="ts">
  const sampleInput = `{
  "service": "treease",
  "focus": "graph sync",
  "formats": ["json", "yaml", "toml"],
  "selection": {
    "path": "services.api.retries",
    "value": 3
  },
  "preview": {
    "color": "#2563eb"
  }
}`;

  const formatted = `{
  "service": "treease",
  "focus": "graph sync",
  "formats": [
    "json",
    "yaml",
    "toml"
  ],
  "selection": {
    "path": "services.api.retries",
    "value": 3
  },
  "preview": {
    "color": "#2563eb"
  }
}`;

  const treeLines = [
    'root (object)',
    '  service: treease',
    '  focus: graph sync',
    '  formats (array)',
    '    [0]: json',
    '    [1]: yaml',
    '    [2]: toml',
    '  selection (object)',
    '    path: services.api.retries',
    '    value: 3',
    '  preview (object)',
    '    color: #2563eb',
  ];

  const input = sampleInput;
  const sourceBytes = 162;
  const nodeCount = 13;
  const rootLabel = 'object';
  const statusLabel = 'Ready';
</script>

<section class="demo-card" aria-labelledby="landing-demo-title" data-testid="landing-demo">
  <div class="demo-header">
    <div class="demo-intro">
      <span class="demo-chip">Format &amp; Parse</span>
      <h2 id="landing-demo-title">Format and parse without a server hop.</h2>
      <p>
        JSON, YAML, TOML — parse, format, and inspect the tree structure entirely
        in the browser.
      </p>
    </div>

    <dl class="demo-metrics">
      <div>
        <dt>Source</dt>
        <dd>{sourceBytes} bytes</dd>
      </div>
      <div>
        <dt>Tree</dt>
        <dd>{nodeCount} nodes {rootLabel}</dd>
      </div>
      <div>
        <dt>Status</dt>
        <dd>{statusLabel}</dd>
      </div>
    </dl>
  </div>

  <div class="demo-workbench">
    <div class="panel panel--input">
      <div class="panel-head">
        <span class="panel-title">Input</span>
      </div>
      <pre class="demo-input" aria-label="Landing page JSON demo input">
{input}</pre>
    </div>

    <section class="panel">
      <div class="panel-head">
        <span class="panel-title">Formatted output</span>
      </div>
      <pre class="demo-output" data-testid="landing-demo-formatted">{formatted}</pre>
    </section>

    <section class="panel panel--tree">
      <div class="panel-head">
        <span class="panel-title">Parsed tree</span>
        <span class="panel-note">{rootLabel}</span>
      </div>
      <ul class="tree-output" data-testid="landing-demo-tree">
        {#each treeLines as line}
          <li>{line}</li>
        {/each}
      </ul>
    </section>
  </div>
</section>

<style>
  .demo-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 24px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    border-radius: 28px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(239, 244, 251, 0.82));
    box-shadow: var(--shadow, 0 30px 80px rgba(15, 23, 42, 0.12));
    backdrop-filter: blur(18px);
  }

  .demo-header {
    display: grid;
    gap: 16px;
    grid-template-columns: minmax(0, 1.2fr) minmax(220px, 0.8fr);
  }

  .demo-intro {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .demo-chip {
    align-self: flex-start;
    padding: 8px 12px;
    border-radius: 999px;
    background: rgba(37, 99, 235, 0.1);
    color: var(--accent-strong, #1d4ed8);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .demo-intro h2 {
    margin: 0;
    font-size: clamp(1.55rem, 2.8vw, 2rem);
    line-height: 1;
  }

  .demo-intro p,
  .panel-note,
  .demo-metrics dt {
    color: var(--muted, #4b5563);
  }

  .demo-intro p {
    margin: 0;
    font-size: 15px;
    line-height: 1.7;
  }

  .demo-metrics {
    display: grid;
    gap: 12px;
    margin: 0;
  }

  .demo-metrics div {
    padding: 14px 16px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.74);
  }

  .demo-metrics dt,
  .demo-metrics dd {
    margin: 0;
  }

  .demo-metrics dt {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .demo-metrics dd {
    margin-top: 6px;
    color: var(--ink, #0f172a);
    font-size: 16px;
    font-weight: 700;
  }

  .demo-workbench {
    display: grid;
    gap: 14px;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
  }

  .panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 200px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    border-radius: 22px;
    background: rgba(255, 255, 255, 0.78);
    overflow: hidden;
  }

  .panel--input {
    grid-row: 1 / span 2;
  }

  .panel--tree {
    min-height: 180px;
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-height: 52px;
    padding: 0 16px;
    border-bottom: 1px solid var(--line, rgba(15, 23, 42, 0.1));
  }

  .panel-title {
    color: var(--ink, #0f172a);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .panel-note {
    font-size: 12px;
    font-weight: 600;
  }

  .demo-input,
  .demo-output,
  .tree-output {
    flex: 1;
    margin: 0;
    padding: 16px 18px;
    color: var(--ink, #0f172a);
    font-family: 'SF Mono', 'Fira Code', 'Fira Mono', Menlo, Consolas, monospace;
    font-size: 13px;
    line-height: 1.7;
  }

  .demo-input {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .demo-output {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .tree-output {
    list-style: none;
    overflow: auto;
  }

  .tree-output li + li {
    margin-top: 2px;
  }

  @media (max-width: 1080px) {
    .demo-header,
    .demo-workbench {
      grid-template-columns: 1fr;
    }

    .panel--input {
      grid-row: auto;
    }
  }

  @media (max-width: 640px) {
    .demo-card {
      padding: 20px;
      border-radius: 24px;
    }

    .demo-metrics div,
    .panel {
      border-radius: 18px;
    }

    .panel-head {
      min-height: 56px;
      padding: 0 14px;
    }
  }

  @media (prefers-color-scheme: dark) {
    .demo-card {
      background:
        linear-gradient(180deg, rgba(12, 21, 36, 0.9), rgba(8, 15, 28, 0.86));
    }

    .demo-chip,
    .demo-metrics div,
    .panel {
      background: rgba(11, 20, 36, 0.78);
    }

    .demo-input,
    .demo-output,
    .tree-output,
    .panel-title,
    .demo-metrics dd {
      color: var(--ink, #e5eefc);
    }
  }
</style>
