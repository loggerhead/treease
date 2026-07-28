import { CLICK_DEDUP_WINDOW_MS } from './constants';
import { extractCandidate } from './extract-candidate';
import { extractPureJsonPage } from './pure-json-page';

let lastSignature = '';
let lastHandledAt = 0;
let enabled = false;
let privacyAcknowledged = false;
let disabledOrigins: string[] = [];
let allowlist: string[] = [];
let blocklist: string[] = [];

function refreshListeningSettings(values: Record<string, unknown>): void {
  enabled = values.enabled === true;
  privacyAcknowledged = values.privacyAcknowledged === true;
  disabledOrigins = Array.isArray(values.disabledOrigins)
    ? values.disabledOrigins.filter((value): value is string => typeof value === 'string')
    : [];
  allowlist = Array.isArray(values.allowlist) ? values.allowlist.filter((value): value is string => typeof value === 'string') : [];
  blocklist = Array.isArray(values.blocklist) ? values.blocklist.filter((value): value is string => typeof value === 'string') : ['*://treease.com/*', '*://*.treease.com/*'];
}

void chrome.storage.local.get(['enabled', 'privacyAcknowledged', 'disabledOrigins', 'allowlist', 'blocklist']).then((values) => {
  refreshListeningSettings(values);
  capturePureJsonPage();
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (area !== 'local') return;
  refreshListeningSettings({
    enabled: changes.enabled?.newValue ?? enabled,
    privacyAcknowledged: changes.privacyAcknowledged?.newValue ?? privacyAcknowledged,
    disabledOrigins: changes.disabledOrigins?.newValue ?? disabledOrigins,
    allowlist: changes.allowlist?.newValue ?? allowlist,
    blocklist: changes.blocklist?.newValue ?? blocklist,
  });
});

function pageMetadata() {
  return { pageTitle: document.title, pageOrigin: location.origin };
}

function describeDomPath(target: Element): string {
  const parts: string[] = [];
  let current: Element | null = target;
  // This is UI metadata only: it contains tags, ids and classes, never text.
  for (let depth = 0; current && depth < 6; depth += 1, current = current.parentElement) {
    const id = current.id ? `#${CSS.escape(current.id)}` : '';
    const classes = Array.from(current.classList).slice(0, 2).map((name) => `.${CSS.escape(name)}`).join('');
    parts.unshift(`${current.tagName.toLowerCase()}${id}${classes}`);
    if (current.id) break;
  }
  return parts.join(' > ');
}

function send(message: unknown): void {
  void chrome.runtime.sendMessage(message).catch(() => {
    // A navigation can tear down the extension context between click and delivery.
  });
}

function matchesAny(patterns: string[]): boolean {
  return patterns.some((pattern) => {
    try { return new RegExp('^' + pattern.replace(/[.+^${}()|[\\]\\]/g, '\\$&').replace(/\\\*/g, '.*') + '$', 'i').test(location.href); } catch { return false; }
  });
}

function canListen(): boolean {
  return enabled && privacyAcknowledged && !disabledOrigins.includes(location.origin) && !matchesAny(blocklist) && (allowlist.length === 0 || matchesAny(allowlist));
}

function sendExtraction(extracted: Exclude<ReturnType<typeof extractCandidate>, { status: 'none' }>, target: Element, openMode: 'user-gesture' | 'auto'): void {
  if (extracted.status === 'too_large') {
    send({ type: 'candidate-too-large', payload: { ...extracted, domPath: describeDomPath(target), ...pageMetadata() }, openMode });
    return;
  }
  send({ type: 'candidate', payload: { ...extracted, domPath: describeDomPath(target), ...pageMetadata() }, openMode });
}

function capturePureJsonPage(): void {
  if (!canListen()) return;
  const extracted = extractPureJsonPage();
  if (extracted.status === 'none') return;
  const pre = document.body?.firstElementChild;
  if (!(pre instanceof Element)) return;
  sendExtraction(extracted, pre, 'auto');
}

function showOpenFallbackHint(): void {
  if (document.querySelector('[data-treease-open-hint]')) return;
  const hint = document.createElement('button');
  hint.type = 'button';
  hint.dataset.treeaseOpenHint = 'true';
  hint.textContent = 'Treease captured JSON — click the extension icon to view the graph';
  hint.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:2147483647;padding:10px 12px;border:1px solid #d9ff62;background:#10211c;color:#effff5;font:12px ui-monospace,monospace;box-shadow:4px 4px 0 #000;cursor:pointer';
  hint.addEventListener('click', () => hint.remove());
  document.documentElement.append(hint);
  window.setTimeout(() => hint.remove(), 8_000);
}

chrome.runtime.onMessage.addListener((message: unknown) => {
  if (typeof message === 'object' && message !== null && (message as { type?: string }).type === 'treease-panel-open-fallback') {
    showOpenFallbackHint();
  }
});

document.addEventListener('click', (event) => {
  if (event.button !== 0 || (!event.metaKey && !event.ctrlKey) || event.altKey || event.shiftKey) return;
  if (!canListen()) return;
  const pathTarget = event.composedPath().find((entry): entry is Element => entry instanceof Element) ?? event.target;
  if (!(pathTarget instanceof Element)) return;
  const extracted = extractCandidate(pathTarget);
  if (extracted.status === 'none') return;

  const signature = extracted.status === 'candidate'
    ? `${extracted.sourceTag}:${extracted.sourceLength}:${extracted.text}`
    : `${extracted.sourceTag}:${extracted.sourceLength}:too-large`;
  const now = Date.now();
  if (signature === lastSignature && now - lastHandledAt < CLICK_DEDUP_WINDOW_MS) return;
  lastSignature = signature;
  lastHandledAt = now;

  sendExtraction(extracted, pathTarget, 'user-gesture');
}, { capture: true, passive: true });
