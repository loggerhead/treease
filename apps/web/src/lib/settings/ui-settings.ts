import { TOKEN_TYPES, TOKEN_TYPE_LAYER, TOKEN_TYPE_THEME_KEY, type TokenType } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { defaultSettings, editorUiColors, graphViewerConfig, neutralSyntaxColors, semanticTypeColors } from './ui-settings-data';

type JsonSchema =
  | {
      type: 'string' | 'number' | 'boolean' | 'null';
    }
  | {
      type: 'array';
      items: JsonSchema;
    }
  | {
      type: 'object';
      properties: Record<string, JsonSchema>;
      additionalProperties: boolean;
    };

function resolveTokenColor(tokenType: TokenType, colors: SemanticTypeColors) {
  if (TOKEN_TYPE_LAYER[tokenType] === 'tree-node') {
    return colors[tokenType];
  }

  const themeKey = TOKEN_TYPE_THEME_KEY[tokenType];
  if (themeKey in neutralSyntaxColors) {
    return neutralSyntaxColors[themeKey as keyof typeof neutralSyntaxColors];
  }
  return colors[themeKey as keyof SemanticTypeColors];
}

function createSemanticTokenColors(colors: SemanticTypeColors) {
  return Object.fromEntries(TOKEN_TYPES.map((tokenType) => [tokenType, resolveTokenColor(tokenType, colors)])) as Record<TokenType, string>;
}

function createLexicalTokenRules(_colors: SemanticTypeColors) {
  const pendingSemanticColor = neutralSyntaxColors.operator.slice(1);
  return [
    // Lexical tokenizers cannot distinguish null from boolean or int from float in every
    // supported language. They stay neutral until Core semantic tokens are available.
    { token: 'string', foreground: pendingSemanticColor },
    { token: 'string.value', foreground: pendingSemanticColor },
    { token: 'string.value.json', foreground: pendingSemanticColor },
    { token: 'string.key', foreground: pendingSemanticColor },
    { token: 'string.key.json', foreground: pendingSemanticColor },
    { token: 'number', foreground: pendingSemanticColor },
    { token: 'number.float', foreground: pendingSemanticColor },
    { token: 'keyword', foreground: pendingSemanticColor },
    { token: 'keyword.json', foreground: pendingSemanticColor },
    { token: 'delimiter', foreground: neutralSyntaxColors.punctuation.slice(1) },
    { token: 'delimiter.bracket', foreground: neutralSyntaxColors.punctuation.slice(1) },
    { token: 'delimiter.array', foreground: neutralSyntaxColors.punctuation.slice(1) },
    { token: 'delimiter.comma', foreground: neutralSyntaxColors.punctuation.slice(1) },
    { token: 'delimiter.colon', foreground: neutralSyntaxColors.punctuation.slice(1) },
  ];
}

export type GraphViewerConfig = typeof graphViewerConfig;
export type SemanticTypeColors = typeof semanticTypeColors;
export type SettingsDocument = Record<string, unknown>;
export type { AutoSaveMode } from './ui-settings-data';

export type Settings = {
  editor: {
    semanticTypeColors: SemanticTypeColors;
    uiColors: typeof editorUiColors;
  };
  formatting: {
    indent: number;
    smart: boolean;
    maxLineLength: number;
    maxInlineComplexity: number;
    maxArrayInlineItems: number;
    alignObjectArrays: boolean;
  };
  viewer: {
    graphViewer: GraphViewerConfig;
  };
  interaction: {
    enableSyncScroll: boolean;
    autoSave: import('./ui-settings-data').AutoSaveMode;
  };
  parser: {
    enableNest: boolean;
  };
};

export { defaultSettings, graphViewerConfig };

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function cloneJsonValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

function createJsonSchema(value: unknown): JsonSchema {
  if (Array.isArray(value)) {
    return {
      type: 'array',
      items: value.length > 0 ? createJsonSchema(value[0]) : { type: 'null' }
    };
  }
  if (isPlainObject(value)) {
    const properties = Object.fromEntries(Object.entries(value).map(([key, nestedValue]) => [key, createJsonSchema(nestedValue)]));
    return {
      type: 'object',
      properties,
      additionalProperties: false
    };
  }
  if (typeof value === 'string') {
    return { type: 'string' };
  }
  if (typeof value === 'number') {
    return { type: 'number' };
  }
  if (typeof value === 'boolean') {
    return { type: 'boolean' };
  }
  return { type: 'null' };
}

function sanitizeValue(defaultValue: unknown, candidate: unknown): unknown {
  if (Array.isArray(defaultValue)) {
    if (!Array.isArray(candidate)) return cloneJsonValue(defaultValue);
    const itemDefault = defaultValue[0];
    if (itemDefault === undefined) return cloneJsonValue(candidate);
    return candidate.map((item) => sanitizeValue(itemDefault, item));
  }
  if (isPlainObject(defaultValue)) {
    if (!isPlainObject(candidate)) return cloneJsonValue(defaultValue);
    return Object.fromEntries(
      Object.entries(defaultValue).map(([key, nestedDefault]) => [key, sanitizeValue(nestedDefault, candidate[key])])
    );
  }
  if (typeof defaultValue === 'string') {
    return typeof candidate === 'string' ? candidate : defaultValue;
  }
  if (typeof defaultValue === 'number') {
    return typeof candidate === 'number' && Number.isFinite(candidate) ? candidate : defaultValue;
  }
  if (typeof defaultValue === 'boolean') {
    return typeof candidate === 'boolean' ? candidate : defaultValue;
  }
  return candidate ?? defaultValue;
}

export const settingsJsonSchema = createJsonSchema(defaultSettings);

export function buildEditorTheme(settings: Settings) {
  const semanticTokenColors = createSemanticTokenColors(settings.editor.semanticTypeColors);
  const tokenRules = [
    ...TOKEN_TYPES.map((tokenType) => ({
      token: tokenType,
      foreground: semanticTokenColors[tokenType].slice(1),
    })),
    ...createLexicalTokenRules(settings.editor.semanticTypeColors),
  ];
  return {
    base: 'vs',
    inherit: true,
    semanticHighlighting: true,
    rules: tokenRules,
    semanticTokenColors,
    colors: settings.editor.uiColors,
  };
}

/** Identifies the settings that can change Monaco's rendered theme. */
export function buildEditorThemeSignature(settings: Settings): string {
  return JSON.stringify({
    semanticTypeColors: settings.editor.semanticTypeColors,
    uiColors: settings.editor.uiColors,
  });
}

type AppliedEditorTheme = {
  themeName: string;
  signature: string;
};

// Monaco themes are global to a loaded Monaco runtime. Keep this state at the
// runtime boundary so mounting another editor surface does not reset the DOM
// decorations of editors that are already visible.
const appliedEditorThemes = new WeakMap<object, AppliedEditorTheme>();

/** Apply the one shared semantic palette to every Monaco surface. */
export function applyEditorTheme(
  monaco: typeof import('monaco-editor'),
  themeName: string,
  settings: Settings,
): void {
  const signature = buildEditorThemeSignature(settings);
  const applied = appliedEditorThemes.get(monaco);
  if (applied?.themeName === themeName && applied.signature === signature) return;

  monaco.editor.defineTheme(themeName, buildEditorTheme(settings) as Monaco.editor.IStandaloneThemeData);
  monaco.editor.setTheme(themeName);
  appliedEditorThemes.set(monaco, { themeName, signature });
}

function mergeObject(target: any, source: any) {
  if (!source || typeof source !== 'object' || Array.isArray(source)) return target;
  const result = Array.isArray(target) ? [...target] : { ...target };
  Object.keys(source).forEach((key) => {
    const value = source[key];
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      result[key] = mergeObject(target?.[key] ?? {}, value);
    } else if (value !== undefined) {
      result[key] = value;
    }
  });
  return result;
}

export function mergeSettings(base: Settings, custom: Partial<Settings>): Settings {
  if (!custom) return cloneJsonValue(base);
  return mergeObject(base, custom);
}

export function mergeSettingsDocument(base: SettingsDocument, custom: Partial<Settings>): SettingsDocument {
  if (!custom) return cloneJsonValue(base);
  if (!isPlainObject(base)) {
    return mergeSettings(defaultSettings, custom);
  }
  return mergeObject(base, custom);
}

export function sanitizeSettingsDocument(document: SettingsDocument): Settings {
  return sanitizeValue(defaultSettings, document) as Settings;
}
