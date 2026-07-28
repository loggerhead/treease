import { MAX_ANCESTOR_LEVELS, MAX_CANDIDATE_BYTES } from './constants';

export type CandidateExtraction =
  | { status: 'candidate'; text: string; sourceTag: string; sourceLength: number }
  | { status: 'too_large'; sourceTag: string; sourceLength: number }
  | { status: 'none' };

const sensitiveSelectors = [
  'input[type="password"]',
  'input[autocomplete="current-password"]',
  'input[autocomplete="new-password"]',
  '[contenteditable="true"]',
  '[data-treease-ignore]',
];

function isSensitive(element: Element): boolean {
  // Feishu renders read-only code content inside a Slate contenteditable zone.
  // Treat only that explicit code-body container as a code source; ordinary
  // rich-text editors remain excluded.
  if (element.matches('.editor-kit-code-block .code-block-content .code-block-zone-container')
    || element.closest('.editor-kit-code-block .code-block-content .code-block-zone-container')) return false;
  return sensitiveSelectors.some((selector) => element.matches(selector));
}

function isSafeTextContainer(element: Element): boolean {
  return !['BODY', 'HTML'].includes(element.tagName);
}

function normalizeText(text: string): string {
  const withoutBom = text.replace(/^\uFEFF/, '').trim();
  const fenced = withoutBom.match(/^```(?:json|yaml|yml|toml)?\s*\r?\n([\s\S]*?)\r?\n?```$/i);
  return (fenced?.[1] ?? withoutBom).trim();
}

/**
 * This is only a cheap routing check, never an acceptance parser. JSON.parse
 * and Treease Core remain the authorities. Its purpose is to avoid treating a
 * syntax-highlighted token (for example one JSON string value in a GitHub
 * table cell) as the final candidate before its enclosing code container is
 * considered.
 */
export function looksLikeStructuredText(text: string): boolean {
  const first = text[0];
  return first === '{' || first === '[' || first === '"' || first === '-' || first === 't' || first === 'f' || first === 'n'
    || (first != null && first >= '0' && first <= '9') || /^[\w.-]+\s*[:=]/.test(text)
    || /[\[{]/.test(text);
}

function toExtraction(text: string, source: Element): CandidateExtraction {
  const normalized = normalizeText(text);
  const sourceLength = new TextEncoder().encode(normalized).byteLength;
  if (!normalized) return { status: 'none' };
  if (sourceLength > MAX_CANDIDATE_BYTES) {
    return { status: 'too_large', sourceTag: source.tagName.toLowerCase(), sourceLength };
  }
  return { status: 'candidate', text: normalized, sourceTag: source.tagName.toLowerCase(), sourceLength };
}

function readElementText(element: Element): string | null {
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) return element.value;
  if (!isSafeTextContainer(element)) return null;
  return element.textContent;
}

function candidateSources(target: Element): Element[] {
  const feishuCodeBody = target.closest('.editor-kit-code-block')?.querySelector<HTMLElement>(
    '.code-block-content .code-block-zone-container',
  );
  // A syntax-highlighted token can itself be a valid JSON scalar (for example
  // "ok"). Prefer the complete Feishu code body so the graph represents the
  // code block the user clicked, rather than one token.
  const sources: Element[] = feishuCodeBody ? [feishuCodeBody] : [target];
  const nearestContainer = target.closest('pre, code, textarea, input');
  if (nearestContainer && nearestContainer !== target) sources.push(nearestContainer);
  let current: Element | null = target.parentElement;
  for (let level = 0; current && level < MAX_ANCESTOR_LEVELS; level += 1, current = current.parentElement) {
    if (isSafeTextContainer(current) && !sources.includes(current)) sources.push(current);
  }
  return sources;
}

export function extractCandidate(target: EventTarget | null): CandidateExtraction {
  if (!(target instanceof Element) || isSensitive(target)) return { status: 'none' };
  for (const source of candidateSources(target)) {
    if (isSensitive(source)) continue;
    const text = readElementText(source);
    if (text == null) continue;
    const result = toExtraction(text, source);
    if (result.status === 'too_large') return result;
    // A non-empty token is not automatically a JSON candidate. Continue to a
    // nearby pre/code/ancestor container; this is what lets a click inside a
    // GitHub syntax-highlighting span resolve to the complete code-cell text.
    if (result.status === 'candidate' && looksLikeStructuredText(result.text)) return result;
  }
  return { status: 'none' };
}
