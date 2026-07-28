import type { CandidateKind, JsonRootType, StructuredLanguage } from '../shared/types';

export type JsonDetection =
  | { status: 'valid'; rootType: JsonRootType }
  | { status: 'invalid'; message: string; position: number | null };

function rootType(value: unknown): JsonRootType {
  if (Array.isArray(value)) return 'array';
  if (value !== null && typeof value === 'object') return 'object';
  return 'scalar';
}

function errorPosition(message: string): number | null {
  const match = message.match(/position (\d+)/i);
  return match ? Number(match[1]) : null;
}

export function detectStrictJson(text: string): JsonDetection {
  try {
    return { status: 'valid', rootType: rootType(JSON.parse(text)) };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Invalid JSON';
    return { status: 'invalid', message: message.slice(0, 180), position: errorPosition(message) };
  }
}

export type StructuredDetection =
  | { status: 'valid'; language: 'json'; rootType: JsonRootType; text: string; candidateKind: CandidateKind }
  | { status: 'candidate'; language: 'yaml' | 'toml'; text: string; candidateKind: 'whole' }
  | { status: 'invalid'; message: string; position: number | null };

/**
 * Finds a complete JSON object/array without evaluating any non-JSON syntax.
 * The scanner is deliberately limited to balanced JSON delimiters and strings;
 * JSON.parse remains the only authority for accepting the result.
 */
function findEmbeddedJson(text: string): string | null {
  for (let start = 0; start < text.length; start += 1) {
    const opener = text[start];
    if (opener !== '{' && opener !== '[') continue;
    const closer = opener === '{' ? '}' : ']';
    let depth = 0;
    let quoted = false;
    let escaped = false;
    for (let index = start; index < text.length; index += 1) {
      const character = text[index]!;
      if (quoted) {
        if (escaped) escaped = false;
        else if (character === '\\') escaped = true;
        else if (character === '"') quoted = false;
        continue;
      }
      if (character === '"') { quoted = true; continue; }
      if (character === opener) depth += 1;
      else if (character === closer && --depth === 0) {
        const candidate = text.slice(start, index + 1);
        if (detectStrictJson(candidate).status === 'valid') return candidate;
        break;
      }
    }
  }
  return null;
}

function likelyFormat(text: string): StructuredLanguage | null {
  const firstLine = text.split(/\r?\n/, 1)[0]?.trim().toLowerCase() ?? '';
  if (/^```(?:yaml|yml)\b/.test(firstLine) || /^---(?:\s|$)/.test(firstLine) || /^[\w.-]+\s*:\s*\S/m.test(text)) return 'yaml';
  if (/^```toml\b/.test(firstLine) || /^\[[^\]]+\]\s*$/m.test(text) || /^[\w.-]+\s*=\s*\S/m.test(text)) return 'toml';
  return null;
}

export function detectStructuredCandidate(text: string): StructuredDetection {
  const direct = detectStrictJson(text);
  if (direct.status === 'valid') return { ...direct, language: 'json', text, candidateKind: 'whole' };
  const embedded = findEmbeddedJson(text);
  if (embedded) {
    const parsed = detectStrictJson(embedded);
    if (parsed.status === 'valid') return { ...parsed, language: 'json', text: embedded, candidateKind: 'embedded' };
  }
  const language = likelyFormat(text);
  if (language === 'yaml' || language === 'toml') return { status: 'candidate', language, text, candidateKind: 'whole' };
  return direct;
}
