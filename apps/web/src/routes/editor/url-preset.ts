import { supportedEditorLanguages, type SupportedEditorLanguageId } from '../../lib/monaco/language-support';

export type EditorUrlActionCommandId = 'format' | 'minify' | 'sort' | 'escape' | 'unescape' | 'compare';
export type EditorUrlUiToken = 'editor' | 'viewer' | 'topbar';
export type EditorUrlPresetTelemetry = {
  rawSearch: string;
  recognized: {
    ui: EditorUrlUiToken[];
    lang: SupportedEditorLanguageId | null;
    textPresent: boolean;
    textUrlPresent: boolean;
    textUrlEffective: boolean;
    rightTextPresent: boolean;
    rightTextUrlPresent: boolean;
    rightTextEffective: boolean;
    rightTextUrlEffective: boolean;
    command: EditorUrlActionCommandId | null;
    yqPresent: boolean;
    yqEffective: boolean;
    nest: boolean | null;
    autoFormat: boolean | null;
    shareID: { present: boolean; value: string | null; valid: boolean };
  };
  ignored: string[];
  finalUi: {
    editor: boolean;
    viewer: boolean;
    topbar: boolean;
  };
  finalAction: 'none' | `command:${EditorUrlActionCommandId}` | 'yq';
};

export type ResolvedEditorUrlPreset = {
  shareID: { present: boolean; value: string | null; valid: boolean };
  ui: {
    editor: boolean;
    viewer: boolean;
    topbar: boolean;
  };
  initialViewerMode: 'graph' | 'text';
  language: SupportedEditorLanguageId | null;
  text: { present: boolean; value: string };
  textUrl: { present: boolean; value: string; effective: boolean };
  rightText: { present: boolean; value: string; effective: boolean };
  rightTextUrl: { present: boolean; value: string; effective: boolean };
  yq: { present: boolean; value: string; effective: boolean };
  command: EditorUrlActionCommandId | null;
  nest: boolean | null;
  autoFormat: boolean | null;
  notes: string[];
  telemetry: EditorUrlPresetTelemetry;
};

type RawQueryValue = {
  present: boolean;
  value: string | null;
};

const allUiTokens: EditorUrlUiToken[] = ['editor', 'viewer', 'topbar'];
const uiTokenSet = new Set<EditorUrlUiToken>(allUiTokens);
const commandSet = new Set<EditorUrlActionCommandId>(['format', 'minify', 'sort', 'escape', 'unescape', 'compare']);
const languageIdMap = new Map(supportedEditorLanguages.map((option) => [option.id.toLowerCase(), option.id as SupportedEditorLanguageId]));
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function readLastQueryValue(searchParams: URLSearchParams, key: string): RawQueryValue {
  const values = searchParams.getAll(key);
  if (values.length === 0) return { present: false, value: null };
  return { present: true, value: values[values.length - 1] ?? '' };
}

export function isEditorResetRequested(search: string): boolean {
  const searchParams = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  return searchParams.get('reset') === '1';
}

function normalizeBooleanValue(raw: RawQueryValue, key: string, ignored: string[]): boolean | null {
  if (!raw.present || raw.value == null || raw.value === '') return null;
  const normalized = raw.value.trim().toLowerCase();
  if (normalized === 'true') return true;
  if (normalized === 'false') return false;
  ignored.push(`${key}=${raw.value}`);
  return null;
}

function normalizeLanguageValue(raw: RawQueryValue, ignored: string[]): SupportedEditorLanguageId | null {
  if (!raw.present || raw.value == null || raw.value === '') return null;
  const normalized = raw.value.trim().toLowerCase();
  const language = languageIdMap.get(normalized) ?? null;
  if (language) return language;
  ignored.push(`lang=${raw.value}`);
  return null;
}

function normalizeCommandValue(raw: RawQueryValue, ignored: string[]): EditorUrlActionCommandId | null {
  if (!raw.present || raw.value == null || raw.value === '') return null;
  const normalized = raw.value.trim().toLowerCase() as EditorUrlActionCommandId;
  if (commandSet.has(normalized)) return normalized;
  ignored.push(`command=${raw.value}`);
  return null;
}

function normalizeUiTokens(raw: RawQueryValue, ignored: string[]): EditorUrlUiToken[] {
  if (!raw.present || raw.value == null || raw.value === '') return [...allUiTokens];
  const resolved = new Set<EditorUrlUiToken>();
  for (const token of raw.value.split(',')) {
    const normalized = token.trim().toLowerCase();
    if (!normalized) continue;
    if (uiTokenSet.has(normalized as EditorUrlUiToken)) {
      resolved.add(normalized as EditorUrlUiToken);
      continue;
    }
    ignored.push(`ui=${token.trim()}`);
  }
  if (!resolved.has('editor') && !resolved.has('viewer')) {
    resolved.add('editor');
    resolved.add('viewer');
  }
  return [...allUiTokens].filter((token) => resolved.has(token));
}

function summarizeAction(command: EditorUrlActionCommandId | null, yqEffective: boolean): EditorUrlPresetTelemetry['finalAction'] {
  if (command) return `command:${command}`;
  return yqEffective ? 'yq' : 'none';
}

export function resolveEditorUrlPreset(search: string): ResolvedEditorUrlPreset {
  const ignored: string[] = [];
  const notes: string[] = [];
  const searchParams = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);

  const rawUi = readLastQueryValue(searchParams, 'ui');
  const rawLang = readLastQueryValue(searchParams, 'lang');
  const rawText = readLastQueryValue(searchParams, 'text');
  const rawTextUrl = readLastQueryValue(searchParams, 'textUrl');
  const rawRightText = readLastQueryValue(searchParams, 'rightText');
  const rawRightTextUrl = readLastQueryValue(searchParams, 'rightTextUrl');
  const rawCommand = readLastQueryValue(searchParams, 'command');
  const rawNest = readLastQueryValue(searchParams, 'nest');
  const rawAutoFormat = readLastQueryValue(searchParams, 'autoFormat');
  const rawYq = readLastQueryValue(searchParams, 'yq');
  const rawShareID = readLastQueryValue(searchParams, 'shareID');
  const shareID = { present: rawShareID.present, value: rawShareID.value, valid: rawShareID.value !== null && uuidPattern.test(rawShareID.value) };

  const uiTokens = normalizeUiTokens(rawUi, ignored);
  const language = normalizeLanguageValue(rawLang, ignored);
  const command = normalizeCommandValue(rawCommand, ignored);
  const nest = normalizeBooleanValue(rawNest, 'nest', ignored);
  const autoFormat = normalizeBooleanValue(rawAutoFormat, 'autoFormat', ignored);

  const textPresent = rawText.present;
  const textValue = rawText.value ?? '';
  const textUrlPresent = rawTextUrl.present && rawTextUrl.value != null && rawTextUrl.value !== '';
  const textUrlValue = rawTextUrl.value ?? '';
  const rightTextPresent = rawRightText.present;
  const rightTextValue = rawRightText.value ?? '';
  const rightTextUrlPresent = rawRightTextUrl.present && rawRightTextUrl.value != null && rawRightTextUrl.value !== '';
  const rightTextUrlValue = rawRightTextUrl.value ?? '';
  const yqPresent = rawYq.present && rawYq.value != null && rawYq.value !== '';
  const yqValue = rawYq.value ?? '';

  let textUrlEffective = textUrlPresent;
  let yqEffective = yqPresent;
  let rightTextEffective = rightTextPresent;
  let rightTextUrlEffective = rightTextUrlPresent;

  if (textPresent && textUrlPresent) {
    textUrlEffective = false;
    notes.push('Ignored textUrl because text takes precedence.');
  }

  if (rightTextPresent && rightTextUrlPresent) {
    rightTextUrlEffective = false;
    notes.push('Ignored rightTextUrl because rightText takes precedence.');
  }

  if (command && yqPresent) {
    yqEffective = false;
    notes.push(`Ignored yq because command=${command} takes precedence.`);
  }

  if (yqEffective && rightTextPresent) {
    rightTextEffective = false;
    notes.push('Ignored rightText because yq takes precedence.');
  }

  if (yqEffective && rightTextUrlEffective) {
    rightTextUrlEffective = false;
    notes.push('Ignored rightTextUrl because yq takes precedence.');
  }

  const baseUi = {
    editor: uiTokens.includes('editor'),
    viewer: uiTokens.includes('viewer'),
    topbar: uiTokens.includes('topbar'),
  };

  const shouldForceViewer = rightTextEffective || rightTextUrlEffective || command === 'compare' || yqEffective;
  const finalUi = {
    editor: baseUi.editor,
    viewer: baseUi.viewer || shouldForceViewer,
    topbar: baseUi.topbar,
  };

  const initialViewerMode: 'graph' | 'text' = shouldForceViewer ? 'text' : 'graph';

  return {
    shareID,
    ui: finalUi,
    initialViewerMode,
    language,
    text: { present: textPresent, value: textValue },
    textUrl: { present: textUrlPresent, value: textUrlValue, effective: textUrlEffective },
    rightText: { present: rightTextPresent, value: rightTextValue, effective: rightTextEffective },
    rightTextUrl: { present: rightTextUrlPresent, value: rightTextUrlValue, effective: rightTextUrlEffective },
    yq: { present: yqPresent, value: yqValue, effective: yqEffective },
    command,
    nest,
    autoFormat,
    notes,
    telemetry: {
      rawSearch: search,
      recognized: {
        ui: uiTokens,
        lang: language,
        textPresent,
        textUrlPresent,
        textUrlEffective,
        rightTextPresent,
        rightTextUrlPresent,
        rightTextEffective,
        rightTextUrlEffective,
        command,
        yqPresent,
        yqEffective,
        nest,
        autoFormat,
        shareID,
      },
      ignored,
      finalUi,
      finalAction: summarizeAction(command, yqEffective),
    },
  };
}

export function canExecuteUrlCommandForLanguage(
  command: EditorUrlActionCommandId,
  language: SupportedEditorLanguageId,
): boolean {
  if (command === 'escape' || command === 'unescape') return language === 'json';
  return true;
}

export function summarizeEditorUrlPresetWarnings(preset: ResolvedEditorUrlPreset): string | null {
  const parts = [...preset.telemetry.ignored, ...preset.notes];
  if (parts.length === 0) return null;
  return `Editor URL preset warnings: ${parts.join(' ')}`;
}
