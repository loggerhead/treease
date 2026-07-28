import './styles.css';
import { GraphViewerRuntime, defaultGraphViewerRenderConfig } from '@treease/graph-viewer-runtime';
import { resolveWorkspacePath } from './workspace-path';
import type { ExtensionMessage } from '../shared/messages';
import type { ExtensionSettings, GraphData, PanelState } from '../shared/types';

const appHost = document.querySelector<HTMLElement>('#app')!;
if (!appHost) throw new Error('Missing Treease Side Panel host.');

let state: PanelState = { status: 'empty' };
let settings: ExtensionSettings | null = null;
let selectedPath = '$';
let graphRequestId = 0;
let graphData: GraphData | null = null;
let graphViewer: GraphViewerRuntime | null = null;
let subgraphViewer: GraphViewerRuntime | null = null;
let graphDocumentExpiry = 0;
const graphWorker = new Worker(new URL('./graph.worker.ts', import.meta.url), { type: 'module' });

function request<T>(message: ExtensionMessage): Promise<T> {
  return chrome.runtime.sendMessage(message) as Promise<T>;
}

function escapeText(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  })[character] ?? character);
}

function render(): void {
  graphViewer?.destroy();
  graphViewer = null;
  subgraphViewer?.destroy();
  subgraphViewer = null;
  const disabled = settings && !settings.enabled;
  const settingsMarkup = settings ? `
    <section class="settings" aria-label="Listening settings">
      <label><input id="global-toggle" type="checkbox" ${settings.enabled ? 'checked' : ''}> Listen on websites</label>
      ${state.status === 'ready' ? `<button id="site-toggle" class="quiet">${settings.disabledOrigins.includes(state.document.pageOrigin) ? 'Enable this site' : 'Pause this site'}</button>` : ''}
      <details class="site-rules"><summary>Site rules</summary><label>Allowlist<textarea id="allowlist" placeholder="*://api.example.com/*">${escapeText(settings.allowlist.join('\n'))}</textarea></label><label>Blocklist<textarea id="blocklist">${escapeText(settings.blocklist.join('\n'))}</textarea></label><button id="save-site-rules" class="quiet">Save rules</button></details>
    </section>` : '';
  let content = '';
  const header = state.status === 'ready'
    ? `<header class="source-header"><dl class="source-summary"><div><dt>Host</dt><dd>${escapeText(new URL(state.document.pageOrigin).host)}</dd></div><div><dt>DOM path</dt><dd><code>${escapeText(state.document.domPath)}</code></dd></div></dl></header>`
    : '<header><img class="brand-logo" src="treease-logo.svg" alt=""><span>Treease</span><span class="mode">JSON GRAPH</span></header>';
  if (!settings?.privacyAcknowledged) {
    content = `<section class="privacy-card"><p class="eyebrow">LOCAL BY DEFAULT</p><h1>See structure, keep context.</h1><p>Treease reads only the text near an element you click. JSON is parsed locally and is never uploaded. Password fields, cookies, forms, and browsing history are not read.</p><button id="accept-privacy">Enable local webpage listening</button></section>`;
  } else if (disabled) {
    content = `<section class="empty-card"><p class="eyebrow">PAUSED</p><h1>Listening is off</h1><p>Turn it on to visualize JSON you click on supported web pages.</p></section>`;
  } else if (state.status === 'empty') {
    content = `<section class="empty-card"><p class="eyebrow">READY</p><h1>Click JSON on a page.</h1><p>Treease will open here with a local graph. No page is scanned in advance.</p></section>`;
  } else if (state.status === 'loading') {
    content = `<section class="empty-card"><p class="eyebrow">BUILDING</p><h1>Preparing your graph…</h1><p>${escapeText(state.pageOrigin)}</p></section>`;
  } else if (state.status === 'invalid') {
    content = `<section class="empty-card error"><p class="eyebrow">NOT JSON</p><h1>That text is not strict JSON.</h1><p>${escapeText(state.message)}${state.position == null ? '' : ` (position ${state.position})`}</p></section>`;
  } else if (state.status === 'too_large') {
    content = `<section class="empty-card error"><p class="eyebrow">TOO LARGE</p><h1>This candidate exceeds 1 MB.</h1><p>Treease skipped parsing to keep the page responsive.</p></section>`;
  } else if (state.status === 'graph_error') {
    content = `<section class="empty-card error"><p class="eyebrow">GRAPH UNAVAILABLE</p><h1>Treease could not build this graph.</h1><p>${escapeText(state.message)}</p></section>`;
  } else {
    content = '<section id="graph" class="graph-host" aria-label="Treease GraphViewer"></section><section id="subgraph-workspace" class="subgraph-workspace" hidden></section><section class="path-bar"><code id="selected-path">' + escapeText(selectedPath) + '</code></section>';
  }
  appHost.innerHTML = `${header}${content}${settingsMarkup}`;
  bindControls();
  if (state.status === 'ready' && state.document.expiresAt !== graphDocumentExpiry) {
    graphDocumentExpiry = state.document.expiresAt;
    graphData = null;
    buildGraph(state.document.text, state.document.language);
  }
  if (state.status === 'ready' && graphData) renderCurrentGraph();
}

function bindControls(): void {
  document.querySelector<HTMLInputElement>('#global-toggle')?.addEventListener('change', (event) => {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void updateSettings({ enabled });
  });
  document.querySelector<HTMLButtonElement>('#site-toggle')?.addEventListener('click', () => {
    if (state.status !== 'ready' || !settings) return;
    const origin = state.document.pageOrigin;
    const disabledOrigins = settings.disabledOrigins.includes(origin)
      ? settings.disabledOrigins.filter((item) => item !== origin)
      : [...settings.disabledOrigins, origin];
    void updateSettings({ disabledOrigins });
  });
  document.querySelector<HTMLButtonElement>('#accept-privacy')?.addEventListener('click', () => {
    void updateSettings({ enabled: true, privacyAcknowledged: true });
  });
  document.querySelector<HTMLButtonElement>('#save-site-rules')?.addEventListener('click', () => {
    const parse = (id: string) => (document.querySelector<HTMLTextAreaElement>(id)?.value ?? '').split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
    void updateSettings({ allowlist: parse('#allowlist'), blocklist: parse('#blocklist') });
  });
}

function renderCurrentGraph(): void {
  const host = document.querySelector<HTMLElement>('#graph');
  if (!host || !graphData) return;
  graphViewer?.destroy();
  graphViewer = new GraphViewerRuntime({ host, config: defaultGraphViewerRenderConfig, interaction: { onActivate: ({ path, nodeKind }) => {
    selectGraphPath(path);
    const workspacePath = resolveWorkspacePath(graphData, path);
    if ((nodeKind === 'object' || nodeKind === 'table') && workspacePath) showSubgraphWorkspace(path, workspacePath);
  } } });
  void graphViewer.replaceGraph(graphData).catch((error: unknown) => {
    if (state.status !== 'ready') return;
    state = { status: 'graph_error', message: error instanceof Error ? error.message : String(error), document: state.document };
    render();
  });
}

function isSamePathSegment(left: GraphData['nodes'][number]['path'][number], right: GraphData['nodes'][number]['path'][number]): boolean {
  return left.tag === right.tag && (left.tag === 0 ? left.key === right.key : left.index === right.index);
}

function isPathPrefix(prefix: GraphData['nodes'][number]['path'], value: GraphData['nodes'][number]['path']): boolean {
  return prefix.length <= value.length && prefix.every((segment, index) => isSamePathSegment(segment, value[index]!));
}

function buildSubgraph(pathValue: GraphData['nodes'][number]['path']): GraphData | null {
  if (!graphData) return null;
  const nodes = graphData.nodes.filter((node) => isPathPrefix(pathValue, node.path));
  if (nodes.length === 0) return null;
  const handles = new Set(nodes.map((node) => node.renderHandle));
  const left = Math.min(...nodes.map((node) => node.boxArgs.x));
  const top = Math.min(...nodes.map((node) => node.boxArgs.y));
  const translateNode = (node: GraphData['nodes'][number]) => ({
    ...node,
    boxArgs: { ...node.boxArgs, x: node.boxArgs.x - left, y: node.boxArgs.y - top },
    meta: {
      ...node.meta,
      boxArgs: { ...node.meta.boxArgs, x: node.meta.boxArgs.x - left, y: node.meta.boxArgs.y - top },
      textArgs: { ...node.meta.textArgs, x: node.meta.textArgs.x - left, y: node.meta.textArgs.y - top },
    },
  });
  const translateEdge = (edge: GraphData['edges'][number]) => ({
    ...edge,
    bezierArgs: {
      ...edge.bezierArgs,
      fromX: edge.bezierArgs.fromX - left, fromY: edge.bezierArgs.fromY - top,
      c1x: edge.bezierArgs.c1x - left, c1y: edge.bezierArgs.c1y - top,
      c2x: edge.bezierArgs.c2x - left, c2y: edge.bezierArgs.c2y - top,
      toX: edge.bezierArgs.toX - left, toY: edge.bezierArgs.toY - top,
    },
  });
  return {
    nodes: nodes.map(translateNode),
    edges: graphData.edges.filter((edge) => handles.has(edge.fromRenderHandle) && handles.has(edge.toRenderHandle)).map(translateEdge),
    coreGraphAvailable: true,
  };
}

function readPathValue(pathValue: GraphData['nodes'][number]['path']): unknown {
  if (state.status !== 'ready') return undefined;
  try {
    return pathValue.reduce<unknown>((value, segment) => {
      if (value == null) return undefined;
      return segment.tag === 0 ? (value as Record<string, unknown>)[String(segment.key)] : (value as unknown[])[segment.index];
    }, JSON.parse(state.document.text));
  } catch { return undefined; }
}

function formatJsonPath(pathValue: GraphData['nodes'][number]['path']): string {
  return pathValue.reduce((result, segment) => segment.tag === 0 && typeof segment.key === 'string'
    ? `${result}.${segment.key}` : `${result}[${segment.index}]`, '$');
}

function showSubgraphWorkspace(selectedCellPath: GraphData['nodes'][number]['path'], workspacePath: GraphData['nodes'][number]['path']): void {
  const host = document.querySelector<HTMLElement>('#subgraph-workspace');
  if (!host) return;
  subgraphViewer?.destroy();
  host.hidden = false;
  document.querySelector<HTMLElement>('#graph')?.classList.add('subgraph-open');
  const isCellSelection = selectedCellPath.length > workspacePath.length;
  const titlePath = formatJsonPath(selectedCellPath);
  host.innerHTML = `<header class="subgraph-heading"><span>${isCellSelection ? 'Selected cell' : 'Subgraph workspace'}</span><code>${escapeText(titlePath)}</code><button id="close-subgraph" class="quiet" aria-label="Close subgraph workspace">Close</button></header>${isCellSelection ? `<pre class="subgraph-value">${escapeText(JSON.stringify(readPathValue(selectedCellPath), null, 2))}</pre>` : '<div id="subgraph-canvas" class="subgraph-canvas"></div>'}`;
  document.querySelector<HTMLButtonElement>('#close-subgraph')?.addEventListener('click', () => {
    subgraphViewer?.destroy();
    subgraphViewer = null;
    host.hidden = true;
    host.replaceChildren();
    document.querySelector<HTMLElement>('#graph')?.classList.remove('subgraph-open');
  });
  if (isCellSelection) return;
  const graph = buildSubgraph(workspacePath);
  const canvas = document.querySelector<HTMLElement>('#subgraph-canvas');
  if (!graph || !canvas) return;
  subgraphViewer = new GraphViewerRuntime({ host: canvas, config: defaultGraphViewerRenderConfig, interaction: { onActivate: ({ path }) => selectGraphPath(path) } });
  void subgraphViewer.replaceGraph(graph);
}

function selectGraphPath(pathValue: GraphData['nodes'][number]['path']): void {
  selectedPath = pathValue.reduce((result, segment) => {
    if (segment.tag === 0 && typeof segment.key === 'string') return /^[A-Za-z_$][\w$]*$/.test(segment.key) ? `${result}.${segment.key}` : `${result}[${JSON.stringify(segment.key)}]`;
    return `${result}[${segment.index}]`;
  }, '$');
  const path = document.querySelector<HTMLElement>('#selected-path');
  if (path) path.textContent = selectedPath;
}

function buildGraph(text: string, language: 'json' | 'yaml' | 'toml'): void {
  const id = ++graphRequestId;
  graphWorker.postMessage({ id, text, language });
}

async function updateSettings(patch: Partial<ExtensionSettings>): Promise<void> {
  const response = await request<{ type: 'settings'; settings: ExtensionSettings }>({ type: 'update-settings', patch });
  settings = response.settings;
  render();
}

graphWorker.addEventListener('message', (event: MessageEvent<{ id: number; ok: boolean; data?: GraphData; error?: string }>) => {
  if (event.data.id !== graphRequestId || state.status !== 'ready') return;
  if (!event.data.ok || !event.data.data) {
    state = { status: 'graph_error', message: event.data.error ?? 'Unknown graph error', document: state.document };
    render();
    return;
  }
  graphData = event.data.data;
  renderCurrentGraph();
});

chrome.runtime.onMessage.addListener((message: unknown) => {
  if (typeof message !== 'object' || message === null || (message as { type?: string }).type !== 'panel-state') return;
  state = (message as { state: PanelState }).state;
  render();
});

// Render the local privacy disclosure synchronously. Extension service workers can wake slowly,
// but a delayed runtime response must never leave the native panel blank.
render();

void Promise.all([
  request<{ type: 'settings'; settings: ExtensionSettings }>({ type: 'get-settings' }),
  request<{ type: 'panel-state'; state: PanelState }>({ type: 'get-panel-state' }),
]).then(([settingsResponse, stateResponse]) => {
  settings = settingsResponse.settings;
  state = stateResponse.state;
  render();
}).catch(() => {
  // The first render remains actionable; the next extension event will refresh live state.
});
