<script lang="ts">
  import { onMount } from 'svelte';

  type InitStatus = 'loading' | 'ready' | 'error';

  interface TreeNode {
    kind: number;
    tag: string;
    value: string;
    children: TreeNode[];
  }

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

  const encoder = new TextEncoder();

  let initStatus: InitStatus = 'loading';
  let errorMsg = '';
  let runError = '';
  let input = sampleInput;
  let formatted = '';
  let treeLines: string[] = [];
  let nodeCount = 0;
  let rootLabel = 'object';
  let sourceBytes = encoder.encode(sampleInput).byteLength;
  let wasmModule: any = null;

  $: sourceBytes = encoder.encode(input).byteLength;

  $: statusLabel =
    initStatus === 'loading'
      ? 'Loading core-lite.wasm'
      : initStatus === 'error'
        ? 'Unavailable'
        : runError
          ? 'Run failed'
          : 'Ready';

  onMount(async () => {
    try {
      const mod = await import('@core-wasm/lite-pkg/core-lite');
      if (typeof mod.default === 'function') {
        await mod.default();
      }
      if (typeof mod.init_wasm === 'function') {
        mod.init_wasm();
      }
      wasmModule = mod;
      initStatus = 'ready';
      runDemo();
    } catch (error) {
      initStatus = 'error';
      errorMsg = error instanceof Error ? error.message : String(error);
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault();
      runDemo();
    }
  }

  function resetDemo() {
    input = sampleInput;
    if (initStatus === 'ready') {
      runDemo();
    }
  }

  function runDemo() {
    if (!wasmModule) return;

    try {
      const nextFormatted = wasmModule.format_text({
        language: 'json',
        text: input,
        indent: 2,
        sortKeys: false
      });
      const parsed = wasmModule.parse_value_to_tree({
        language: 'json',
        text: input,
        nest: false
      });
      const nextTree = parsed?.tree ?? null;

      formatted = typeof nextFormatted === 'string' ? nextFormatted : String(nextFormatted);
      treeLines = buildTreeLines(nextTree);
      nodeCount = countNodes(nextTree);
      rootLabel = describeRoot(nextTree);
      runError = '';
    } catch (error) {
      formatted = '';
      treeLines = [];
      nodeCount = 0;
      rootLabel = 'unavailable';
      runError = error instanceof Error ? error.message : String(error);
    }
  }

  function describeRoot(node: TreeNode | null): string {
    if (!node) return 'unavailable';
    if (node.kind === 1) return 'object';
    if (node.kind === 0) return 'array';
    return 'value';
  }

  function countNodes(node: TreeNode | null): number {
    if (!node) return 0;
    return 1 + node.children.reduce((sum, child) => sum + countNodes(child), 0);
  }

  function buildTreeLines(node: TreeNode | null, depth = 0, label = 'root'): string[] {
    if (!node) return [];

    const indent = '  '.repeat(depth);

    if (node.kind === 1) {
      const lines = [`${indent}${label} (object)`];
      for (let index = 0; index < node.children.length; index += 2) {
        const keyNode = node.children[index];
        const valueNode = node.children[index + 1];
        const childLabel = keyNode?.value || `field-${index / 2}`;

        if (!valueNode) {
          lines.push(`${'  '.repeat(depth + 1)}${childLabel}: null`);
          continue;
        }

        if (valueNode.children.length > 0) {
          lines.push(...buildTreeLines(valueNode, depth + 1, childLabel));
        } else {
          lines.push(`${'  '.repeat(depth + 1)}${childLabel}: ${valueNode.value || 'null'}`);
        }
      }
      return lines;
    }

    if (node.kind === 0) {
      const lines = [`${indent}${label} (array)`];
      node.children.forEach((child, index) => {
        const childLabel = `[${index}]`;
        if (child.children.length > 0) {
          lines.push(...buildTreeLines(child, depth + 1, childLabel));
        } else {
          lines.push(`${'  '.repeat(depth + 1)}${childLabel}: ${child.value || 'null'}`);
        }
      });
      return lines;
    }

    return [`${indent}${label}: ${node.value || 'null'}`];
  }
</script>

<section class="demo-card" aria-labelledby="landing-demo-title" data-testid="landing-demo">
  <div class="demo-header">
    <div class="demo-intro">
      <span class="demo-chip">core-lite.wasm</span>
      <h2 id="landing-demo-title">Format and parse in the browser.</h2>
      <p>
        The landing page keeps the demo small on purpose. It loads the lite
        binary, formats JSON, and renders the parsed tree without a server hop.
      </p>
    </div>

    <dl class="demo-metrics">
      <div>
        <dt>Status</dt>
        <dd aria-live="polite" data-testid="landing-demo-status">{statusLabel}</dd>
      </div>
      <div>
        <dt>Source</dt>
        <dd>{sourceBytes} bytes</dd>
      </div>
      <div>
        <dt>Tree</dt>
        <dd>{nodeCount} nodes</dd>
      </div>
    </dl>
  </div>

  <div class="demo-workbench">
    <label class="panel panel--input">
      <span class="panel-head">
        <span class="panel-title">Input</span>
        <span class="panel-note">Ctrl + Enter</span>
      </span>
      <textarea
        aria-label="Landing page JSON demo input"
        class="demo-input"
        data-testid="landing-demo-input"
        bind:value={input}
        disabled={initStatus !== 'ready'}
        spellcheck="false"
        onkeydown={handleKeydown}
      ></textarea>
    </label>

    <section class="panel">
      <div class="panel-head panel-head--split">
        <span class="panel-title">Formatted output</span>
        <div class="panel-actions">
          <button class="ghost-btn" type="button" onclick={resetDemo}>Reset</button>
          <button
            class="solid-btn"
            data-testid="landing-demo-run"
            disabled={initStatus !== 'ready'}
            type="button"
            onclick={runDemo}
          >
            Run demo
          </button>
        </div>
      </div>

      {#if initStatus === 'loading'}
        <div class="panel-state panel-state--loading" aria-label="Loading demo output">
          <span class="skeleton skeleton--wide"></span>
          <span class="skeleton"></span>
          <span class="skeleton skeleton--short"></span>
        </div>
      {:else if initStatus === 'error'}
        <div class="panel-state panel-state--error">WASM init failed: {errorMsg}</div>
      {:else if runError}
        <div class="panel-state panel-state--error">Parse failed: {runError}</div>
      {:else}
        <pre class="demo-output" data-testid="landing-demo-formatted">{formatted}</pre>
      {/if}
    </section>

    <section class="panel panel--tree">
      <div class="panel-head">
        <span class="panel-title">Parsed tree</span>
        <span class="panel-note">{rootLabel}</span>
      </div>

      {#if initStatus === 'loading'}
        <div class="panel-state panel-state--loading" aria-label="Loading tree output">
          <span class="skeleton skeleton--wide"></span>
          <span class="skeleton"></span>
          <span class="skeleton skeleton--short"></span>
        </div>
      {:else if initStatus === 'error'}
        <div class="panel-state panel-state--error">WASM init failed: {errorMsg}</div>
      {:else if runError}
        <div class="panel-state panel-state--error">Tree unavailable while JSON is invalid.</div>
      {:else}
        <ul class="tree-output" data-testid="landing-demo-tree">
          {#each treeLines as line}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
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
  .panel-state,
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

  .panel-head--split {
    align-items: center;
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

  .panel-actions {
    display: inline-flex;
    gap: 8px;
  }

  .ghost-btn,
  .solid-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 36px;
    padding: 0 14px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 700;
    transition:
      transform 140ms ease,
      border-color 160ms ease,
      background-color 160ms ease,
      color 160ms ease;
  }

  .ghost-btn {
    border: 1px solid var(--line, rgba(15, 23, 42, 0.1));
    background: transparent;
    color: var(--ink, #0f172a);
  }

  .solid-btn {
    border: none;
    background: var(--accent, #2563eb);
    color: #fff;
  }

  .ghost-btn:hover {
    border-color: var(--line-strong, rgba(37, 99, 235, 0.18));
    background: rgba(255, 255, 255, 0.7);
  }

  .solid-btn:hover {
    background: var(--accent-strong, #1d4ed8);
  }

  .ghost-btn:active,
  .solid-btn:active {
    transform: translateY(1px) scale(0.99);
  }

  .ghost-btn:disabled,
  .solid-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
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
    border: none;
    background: transparent;
    resize: none;
    outline: none;
  }

  .demo-input:disabled {
    opacity: 0.6;
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

  .panel-state {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 10px;
    justify-content: center;
    min-height: 160px;
    padding: 18px;
    font-size: 14px;
    line-height: 1.6;
  }

  .panel-state--error {
    color: #b91c1c;
  }

  .skeleton {
    display: block;
    height: 12px;
    border-radius: 999px;
    background: linear-gradient(90deg, rgba(37, 99, 235, 0.08), rgba(37, 99, 235, 0.18), rgba(37, 99, 235, 0.08));
  }

  .skeleton--wide {
    width: 88%;
  }

  .skeleton--short {
    width: 56%;
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

    .panel-actions {
      flex-wrap: wrap;
      justify-content: flex-end;
    }

    .ghost-btn,
    .solid-btn {
      min-height: 34px;
      padding: 0 12px;
    }
  }

  @media (prefers-color-scheme: dark) {
    .demo-card {
      background:
        linear-gradient(180deg, rgba(12, 21, 36, 0.9), rgba(8, 15, 28, 0.86));
    }

    .demo-chip,
    .demo-metrics div,
    .panel,
    .ghost-btn:hover {
      background: rgba(11, 20, 36, 0.78);
    }

    .demo-input,
    .demo-output,
    .tree-output,
    .ghost-btn,
    .panel-title,
    .demo-metrics dd {
      color: var(--ink, #e5eefc);
    }

    .panel-state--error {
      color: #fecaca;
    }
  }
</style>
