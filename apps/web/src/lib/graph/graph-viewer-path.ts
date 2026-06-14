import { buildReadablePath as formatReadablePath, isPathSegIndex, isPathSegKey, pathSegKeyValue, type PathSeg } from '../store/tree-path';
import { supportedEditorLanguageSet, type SupportedEditorLanguageId } from '../monaco/language-support';
import type { GraphCell, GraphCellKind } from './graph-viewer-render';
import { formatScalarLiteral, formatStructuredPreview } from './literal-display';
import { escapeHtml } from '../preview/utils';

export type GraphTooltipPayload = {
  text: string;
  kind: GraphCellKind | 'meta';
  languageId: SupportedEditorLanguageId;
  valueType?: string;
  useSyntaxHighlight: boolean;
};

export function setValueAtPath(data: any, path: PathSeg[], value: unknown): unknown {
  if (path.length == 0) return value;
  let target = data;
  for (let i = 0; i < path.length - 1; i += 1) {
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    if (target == null) return data;
    target = target[key];
  }
  const lastSeg = path[path.length - 1];
  const lastKey = isPathSegIndex(lastSeg) ? lastSeg.index : pathSegKeyValue(lastSeg);
  if (target != null) target[lastKey] = value;
  return data;
}

export function normalizeKeyInput(raw: string, languageId?: string): string {
  if (languageId !== 'json') return raw;
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === 'string') return parsed;
  } catch {
    return raw;
  }
  return raw;
}

export function renameKeyAtPath(data: any, path: PathSeg[], nextKey: string): unknown {
  if (path.length == 0) return data;
  const lastSeg = path[path.length - 1];
  if (!isPathSegKey(lastSeg)) return data;
  const lastKey = pathSegKeyValue(lastSeg);
  let target = data;
  for (let i = 0; i < path.length - 1; i += 1) {
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    if (target == null) return data;
    target = target[key];
  }
  if (target == null || typeof target !== 'object' || Array.isArray(target)) return data;
  if (lastKey === nextKey) return data;
  const value = target[lastKey];
  delete target[lastKey];
  target[nextKey] = value;
  return data;
}

export function getValueAtPath(data: any, path: PathSeg[]): unknown {
  if (!path || path.length == 0) return data;
  let target = data;
  for (let i = 0; i < path.length; i += 1) {
    if (target == null) return undefined;
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    target = target[key];
  }
  return target;
}

function resolveTooltipLanguage(language?: string): SupportedEditorLanguageId {
  if (language && supportedEditorLanguageSet.has(language as SupportedEditorLanguageId)) {
    return language as SupportedEditorLanguageId;
  }
  return 'json';
}

function formatTooltipValue(raw: unknown, cell: GraphCell, language?: string): string {
  try {
    return typeof raw === 'object' && raw !== null
      ? formatStructuredPreview(raw, language)
      : formatScalarLiteral(String(raw), cell.valueType, language);
  } catch (error) {
    console.error('[graph-viewer-path] formatTooltipValue failed', { valueType: cell.valueType }, error);
    return String(raw);
  }
}

function isStructuredValue(value: unknown): value is Record<string, unknown> | unknown[] {
  return Array.isArray(value) || (!!value && typeof value === 'object');
}

function isNonEmptyStructuredSummary(valueType: string | undefined, text: string | undefined): boolean {
  const normalized = text?.trim() ?? '';
  if (!normalized) return false;
  if (valueType === 'object') {
    if (normalized === '{}' || normalized === '{0}') return false;
    if (/^\{[1-9]\d*\}$/.test(normalized)) return true;
    return normalized === '{...}';
  }
  if (valueType === 'array') {
    if (normalized === '[]' || normalized === '[0]') return false;
    if (/^\[[1-9]\d*\]$/.test(normalized)) return true;
    return normalized === '[...]';
  }
  return false;
}

function shouldUseSyntaxHighlight(cell: GraphCell, raw: unknown): boolean {
  if (cell.valueType !== 'object' && cell.valueType !== 'array') return false;
  if (Array.isArray(raw)) return raw.length > 0;
  if (raw && typeof raw === 'object') return Object.keys(raw as Record<string, unknown>).length > 0;
  return isNonEmptyStructuredSummary(cell.valueType, cell.text);
}

export function buildTooltipPayload(currentData: unknown, target: any, language?: string): GraphTooltipPayload | null {
  const cell = target?.__graphCell as GraphCell | undefined;
  if (!cell) return null;
  const kind = ((target?.__graphCellKind as GraphCellKind | undefined) ?? null) as GraphCellKind | null;
  const languageId = resolveTooltipLanguage(language);

  if (kind === 'key') {
    const text = cell.text ?? '';
    if (!text) return null;
    return {
      text,
      kind,
      languageId,
      valueType: cell.valueType,
      useSyntaxHighlight: false,
    };
  }

  if (kind === 'meta') {
    const text = formatReadablePath(cell.path);
    if (!text) return null;
    return {
      text,
      kind: 'meta',
      languageId,
      valueType: cell.valueType,
      useSyntaxHighlight: false,
    };
  }

  const hasPath = Array.isArray(cell.path) && cell.path.length > 0;
  const raw = hasPath ? getValueAtPath(currentData, cell.path) : undefined;
  let text = '';
  if (raw !== undefined) {
    text = formatTooltipValue(raw, cell, language);
  } else if (cell.valueType === 'object' || cell.valueType === 'array') {
    if (isStructuredValue(cell.value)) {
      text = formatStructuredPreview(cell.value, language);
    } else if (cell.text) {
      text = cell.text;
    }
  } else if (cell.value != null) {
    text = formatScalarLiteral(`${cell.value}`, cell.valueType, language);
  } else {
    text = formatScalarLiteral(cell.text ?? '', cell.valueType, language);
  }

  if (!text) return null;
  return {
    text,
    kind: kind ?? 'value',
    languageId,
    valueType: cell.valueType,
    useSyntaxHighlight: shouldUseSyntaxHighlight(cell, raw),
  };
}

export function getCellTooltipText(currentData: unknown, cell: GraphCell, language?: string, kind?: GraphCellKind | null): string {
  const payload = buildTooltipPayload(currentData, { __graphCell: cell, __graphCellKind: kind ?? null }, language);
  return payload?.text ?? '';
}

function buildTooltipClassName(payload: GraphTooltipPayload): string {
  if (payload.kind === 'meta') return 'graph-tooltip-meta-path';
  if (payload.kind === 'key') return 'graph-tooltip-key';
  if (payload.kind === 'value') {
    if (payload.valueType === 'string') return 'graph-tooltip-value-string';
    if (payload.valueType === 'boolean') return 'graph-tooltip-value-boolean';
    if (payload.valueType === 'null') return 'graph-tooltip-value-null';
    if (payload.valueType === 'number') return 'graph-tooltip-value-number';
  }
  return '';
}

export function buildTooltipContent(currentData: unknown, target: any, language?: string): string {
  const payload = buildTooltipPayload(currentData, target, language);
  if (!payload?.text) return '';
  const className = buildTooltipClassName(payload);
  if (className) {
    return `<div class="graph-tooltip-pre-shell"><pre class="${className}">${escapeHtml(payload.text)}</pre></div>`;
  }
  return `<div class="graph-tooltip-pre-shell"><pre>${escapeHtml(payload.text)}</pre></div>`;
}

// NOTE: A namesake `buildPathKey` also exists in
// `apps/web/src/workers/runtime/tree-path.ts` (identical logic).
// Keep in sync if changing the format.
export function buildPathKey(path: PathSeg[]): string {
  if (!path || path.length == 0) return '';
  return path.map((seg) => (isPathSegKey(seg) ? `k:${pathSegKeyValue(seg)}` : `i:${seg.index}`)).join('|');
}
