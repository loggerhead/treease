import type { SettingsDocument } from './ui-settings';

export const DEFAULT_EDITOR_SPLIT_RATIO = 0.28;
export const DEFAULT_COLUMN_NAVIGATOR_HEIGHT_PX = 220;
export const DEFAULT_SIDEBAR_EXPANDED = true;

const EDITOR_LAYOUT_STATE_KEY = '__treeaseEditorLayout';
const MIN_SPLIT_RATIO = 0.2;
const MAX_SPLIT_RATIO = 0.8;

type EditorLayoutState = {
  splitRatio?: unknown;
  columnNavigatorHeightPx?: unknown;
  sidebarExpanded?: unknown;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function normalizeEditorSplitRatio(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return Math.min(Math.max(value, MIN_SPLIT_RATIO), MAX_SPLIT_RATIO);
}

export function getEditorSplitRatio(document: SettingsDocument): number | null {
  if (!isRecord(document)) return null;
  const state = document[EDITOR_LAYOUT_STATE_KEY];
  if (!isRecord(state)) return null;
  return normalizeEditorSplitRatio((state as EditorLayoutState).splitRatio);
}

export function withEditorSplitRatio(document: SettingsDocument, splitRatio: number): SettingsDocument {
  const normalized = normalizeEditorSplitRatio(splitRatio);
  if (normalized === null) return document;
  const currentState = isRecord(document[EDITOR_LAYOUT_STATE_KEY]) ? document[EDITOR_LAYOUT_STATE_KEY] : {};
  return {
    ...document,
    [EDITOR_LAYOUT_STATE_KEY]: {
      ...currentState,
      splitRatio: normalized,
    },
  };
}

export function normalizeColumnNavigatorHeight(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return Math.max(100, value);
}

export function getColumnNavigatorHeight(document: SettingsDocument): number | null {
  if (!isRecord(document)) return null;
  const state = document[EDITOR_LAYOUT_STATE_KEY];
  if (!isRecord(state)) return null;
  return normalizeColumnNavigatorHeight((state as EditorLayoutState).columnNavigatorHeightPx);
}

export function withColumnNavigatorHeight(document: SettingsDocument, heightPx: number): SettingsDocument {
  const normalized = normalizeColumnNavigatorHeight(heightPx);
  if (normalized === null) return document;
  const currentState = isRecord(document[EDITOR_LAYOUT_STATE_KEY]) ? document[EDITOR_LAYOUT_STATE_KEY] : {};
  return {
    ...document,
    [EDITOR_LAYOUT_STATE_KEY]: {
      ...currentState,
      columnNavigatorHeightPx: normalized,
    },
  };
}

export function normalizeSidebarExpanded(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null;
}

export function getSidebarExpanded(document: SettingsDocument): boolean | null {
  if (!isRecord(document)) return null;
  const state = document[EDITOR_LAYOUT_STATE_KEY];
  if (!isRecord(state)) return null;
  return normalizeSidebarExpanded((state as EditorLayoutState).sidebarExpanded);
}

export function withSidebarExpanded(document: SettingsDocument, expanded: boolean): SettingsDocument {
  const normalized = normalizeSidebarExpanded(expanded);
  if (normalized === null) return document;
  const currentState = isRecord(document[EDITOR_LAYOUT_STATE_KEY]) ? document[EDITOR_LAYOUT_STATE_KEY] : {};
  return {
    ...document,
    [EDITOR_LAYOUT_STATE_KEY]: {
      ...currentState,
      sidebarExpanded: normalized,
    },
  };
}

/** Keeps editor-only state out of the user-editable Settings JSON document. */
export function omitEditorLayoutState(document: SettingsDocument): SettingsDocument {
  if (!isRecord(document) || !(EDITOR_LAYOUT_STATE_KEY in document)) return document;
  const { [EDITOR_LAYOUT_STATE_KEY]: _editorLayoutState, ...settings } = document;
  return settings;
}

/** Re-attaches editor-only state after the Settings dialog saves its visible document. */
export function mergeEditorLayoutState(document: SettingsDocument, existing: SettingsDocument): SettingsDocument {
  if (!isRecord(existing) || !(EDITOR_LAYOUT_STATE_KEY in existing)) return document;
  return {
    ...document,
    [EDITOR_LAYOUT_STATE_KEY]: existing[EDITOR_LAYOUT_STATE_KEY],
  };
}
