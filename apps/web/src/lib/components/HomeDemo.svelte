<script lang="ts">
  const sourceText = `{
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

  const sourceLines = sourceText.split('\n');
  const graphLines = [
    { level: 0, type: 'object', title: 'root', note: 'object', selected: true },
    { level: 1, type: 'string', title: 'service', note: '"treease"', selected: false },
    { level: 1, type: 'string', title: 'focus', note: '"graph sync"', selected: false },
    { level: 1, type: 'array', title: 'formats', note: '[3]', selected: false },
    { level: 1, type: 'object', title: 'selection', note: 'object', selected: false },
    { level: 2, type: 'string', title: 'path', note: '"services.api.retries"', selected: true },
    { level: 2, type: 'number', title: 'value', note: '3', selected: true },
    { level: 1, type: 'object', title: 'preview', note: 'object', selected: false },
    { level: 2, type: 'string', title: 'color', note: '"#2563eb"', selected: false },
  ];

  const metrics = [
    { label: 'Source bytes', value: '182' },
    { label: 'Nodes', value: '13' },
    { label: 'Status', value: 'Ready (read-only)' },
  ];
</script>

<section class="demo-card" aria-labelledby="landing-demo-title" data-testid="landing-demo">
  <div class="demo-header">
    <div class="demo-intro">
      <span class="demo-chip">Read-only demo</span>
      <h2 id="landing-demo-title">Structured text and graph panel side by side</h2>
      <p>
        This static preview shows the same two-pane layout as the editor, with
        source text on the left and parsed structure on the right.
      </p>
    </div>

    <dl class="demo-metrics">
      {#each metrics as metric}
        <div>
          <dt>{metric.label}</dt>
          <dd>{metric.value}</dd>
        </div>
      {/each}
    </dl>
  </div>

  <div class="demo-workbench" role="region" aria-label="Read-only split demo">
    <section class="panel panel--input">
      <div class="panel-head">
        <span class="panel-title">Source</span>
        <span class="panel-note">sample.tree.json</span>
      </div>
      <ol class="demo-editor" aria-label="Read-only source text">
        {#each sourceLines as line}
          <li>{line}</li>
        {/each}
      </ol>
    </section>

    <div class="readonly-divider" aria-hidden="true">
      <span class="readonly-divider__bar"></span>
      <span class="readonly-divider__icon">⟷</span>
    </div>

    <section class="panel panel--graph">
      <div class="panel-head">
        <span class="panel-title">Parsed graph</span>
        <span class="panel-note">preview</span>
      </div>
      <div class="graph-root" aria-label="Parsed graph preview">
        {#each graphLines as node}
          <div class="graph-row {node.selected ? 'graph-row--selected' : ''}">
            <span class={`graph-indent-${node.level}`}></span>
            <span class="graph-type">{node.type}</span>
            <span class="graph-title">{node.title}</span>
            <span class="graph-note">{node.note}</span>
          </div>
        {/each}
      </div>
    </section>
  </div>
</section>

<style>
  .demo-card {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    border-radius: 26px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(237, 243, 255, 0.86));
    box-shadow: var(--shadow, 0 30px 80px rgba(15, 23, 42, 0.12));
    backdrop-filter: blur(16px);
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
    font-size: clamp(1.35rem, 2.4vw, 1.9rem);
    line-height: 1.15;
    letter-spacing: -0.04em;
  }

  .demo-intro p,
  .panel-note,
  .demo-metrics dt {
    color: var(--muted, #4b5563);
  }

  .demo-intro p {
    margin: 0;
    font-size: 14px;
    line-height: 1.7;
  }

  .demo-metrics {
    display: grid;
    gap: 10px;
    margin: 0;
  }

  .demo-metrics div {
    padding: 12px 14px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.74);
  }

  .demo-metrics dt,
  .demo-metrics dd {
    margin: 0;
  }

  .demo-metrics dt {
    font-size: 11px;
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
    gap: 0;
    min-height: 330px;
    grid-template-columns: minmax(0, 1fr) 18px minmax(0, 0.95fr);
    border-radius: 20px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.12));
    overflow: hidden;
    background: var(--panel-bg, #ffffff);
  }

  .panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .panel--input,
  .panel--graph {
    background: rgba(255, 255, 255, 0.94);
  }

  .panel--graph {
    border-left: 1px solid var(--line, rgba(15, 23, 42, 0.1));
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-height: 44px;
    padding: 0 14px;
    border-bottom: 1px solid var(--line, rgba(15, 23, 42, 0.1));
  }

  .panel-title {
    color: var(--ink, #0f172a);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .panel-note {
    font-size: 12px;
    font-weight: 600;
  }

  .demo-editor,
  .graph-root {
    flex: 1;
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
    overflow: auto;
    font-family: 'SF Mono', 'Fira Code', 'Fira Mono', Menlo, Consolas, monospace;
    font-size: 13px;
  }

  .demo-editor {
    padding: 14px 12px 14px 0;
    color: var(--ink, #0f172a);
    counter-reset: list-item;
  }

  .demo-editor li {
    display: grid;
    grid-template-columns: 42px 1fr;
    align-items: center;
    min-width: 0;
    padding: 0 12px 0 10px;
    line-height: 1.6;
  }

  .demo-editor li::before {
    content: counter(list-item) ' ';
    counter-increment: list-item;
    color: var(--muted-soft, #6b7280);
    text-align: right;
    font-size: 12px;
    user-select: none;
  }

  .readonly-divider {
    position: relative;
    display: flex;
    align-items: stretch;
    justify-content: center;
    background: rgba(148, 163, 184, 0.08);
  }

  .readonly-divider__bar {
    width: 1px;
    background: var(--line, rgba(15, 23, 42, 0.2));
  }

  .readonly-divider__icon {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 20px;
    height: 20px;
    margin-left: -10px;
    margin-top: -10px;
    border-radius: 999px;
    border: 1px solid var(--line, rgba(15, 23, 42, 0.2));
    background: rgba(255, 255, 255, 0.95);
    color: var(--muted, #4b5563);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
  }

  .graph-root {
    padding: 10px;
    color: var(--ink, #0f172a);
    background: rgba(255, 255, 255, 0.9);
  }

  .graph-row {
    display: grid;
    grid-template-columns: auto auto 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 8px 6px;
    border-radius: 10px;
    line-height: 1.3;
  }

  .graph-row + .graph-row {
    margin-top: 4px;
  }

  .graph-row--selected {
    background: rgba(37, 99, 235, 0.1);
    color: #1d4ed8;
  }

  .graph-type {
    color: #0f172a;
    opacity: 0.8;
    font-size: 11px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .graph-title {
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .graph-note {
    color: var(--muted, #4b5563);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .graph-indent-0,
  .graph-indent-1,
  .graph-indent-2 {
    width: 12px;
  }

  .graph-indent-1 {
    margin-left: 10px;
  }

  .graph-indent-2 {
    margin-left: 22px;
  }

  @media (max-width: 1080px) {
    .demo-workbench {
      grid-template-columns: 1fr;
      min-height: 0;
    }

    .panel--graph,
    .panel--input {
      min-height: 300px;
    }

    .readonly-divider {
      display: none;
    }

    .panel--graph {
      border-left: none;
      border-top: 1px solid var(--line, rgba(15, 23, 42, 0.12));
    }

    .demo-metrics {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 640px) {
    .demo-header,
    .demo-workbench {
      gap: 12px;
    }

    .demo-card {
      padding: 14px;
      border-radius: 22px;
    }

    .demo-header {
      grid-template-columns: 1fr;
    }

    .demo-metrics {
      grid-template-columns: 1fr;
      gap: 8px;
    }

    .demo-intro p {
      max-width: 100%;
    }

    .demo-editor,
    .graph-root {
      font-size: 12px;
    }

    .graph-row {
      gap: 6px;
      padding: 7px 4px;
    }

    .graph-note {
      font-size: 11px;
    }
  }

  @media (prefers-color-scheme: dark) {
    .demo-card,
    .demo-metrics div,
    .panel--input,
    .panel--graph,
    .readonly-divider__icon,
    .graph-root {
      background: rgba(11, 20, 36, 0.88);
    }

    .demo-intro p,
    .panel-note,
    .demo-metrics dt,
    .demo-metrics dt,
    .panel-note,
    .graph-note,
    .graph-type,
    .demo-editor {
      color: #d3deef;
    }

    .panel-title,
    .demo-metrics dd,
    .graph-title,
    .demo-editor li::before,
    .demo-editor li,
    .graph-title {
      color: #e5eefc;
    }

    .demo-chip {
      background: rgba(96, 165, 250, 0.16);
      color: #93c5fd;
    }

    .graph-row--selected {
      background: rgba(96, 165, 250, 0.18);
      color: #bfdbfe;
    }

    .demo-workbench,
    .panel--graph,
    .panel--input,
    .readonly-divider,
    .readonly-divider__bar,
    .demo-metrics div {
      border-color: rgba(148, 163, 184, 0.2);
    }
  }
</style>
