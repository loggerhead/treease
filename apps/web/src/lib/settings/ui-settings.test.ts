import { describe, it, expect } from 'vitest';
import { TOKEN_TYPES, TOKEN_TYPE_THEME_KEY, TREE_NODE_TOKEN_TYPES } from '@core-wasm/index';
import {
  mergeSettings,
  buildEditorTheme,
  defaultSettings,
  sanitizeSettingsDocument,
  settingsJsonSchema,
} from './ui-settings';
import type { Settings } from './ui-settings';

describe('ui-settings', () => {
  describe('mergeSettings', () => {
    it('returns a deep clone of base when custom is empty', () => {
      const result = mergeSettings(defaultSettings, {});
      expect(result).toEqual(defaultSettings);
      expect(result).not.toBe(defaultSettings);
    });

    it('returns a deep clone when custom is null', () => {
      const result = mergeSettings(defaultSettings, null as any);
      expect(result).toEqual(defaultSettings);
    });

    it('overrides scalar values', () => {
      const result = mergeSettings(defaultSettings, {
        formatting: { indent: 4 },
      } as Partial<Settings>);
      expect(result.formatting.indent).toBe(4);
      expect(result.formatting.smart).toBe(defaultSettings.formatting.smart);
    });

    it('deep merges nested objects', () => {
      const result = mergeSettings(defaultSettings, {
        editor: { semanticTypeColors: { str: '#ff0000' } },
      } as any);
      expect(result.editor.semanticTypeColors.str).toBe('#ff0000');
      expect(result.editor.semanticTypeColors.int).toBe(defaultSettings.editor.semanticTypeColors.int);
    });

    it('does not mutate base or custom', () => {
      const baseCopy = JSON.parse(JSON.stringify(defaultSettings));
      const custom = { formatting: { indent: 8 } } as Partial<Settings>;
      mergeSettings(defaultSettings, custom);
      expect(defaultSettings).toEqual(baseCopy);
    });
  });

  describe('sanitizeSettingsDocument', () => {
    it('falls back to defaults for invalid scalar values', () => {
      const result = sanitizeSettingsDocument({
        formatting: {
          indent: 'bad',
          smart: 'true'
        }
      });
      expect(result.formatting.indent).toBe(defaultSettings.formatting.indent);
      expect(result.formatting.smart).toBe(defaultSettings.formatting.smart);
    });

    it('keeps valid nested values while dropping unknown keys from effective settings', () => {
      const result = sanitizeSettingsDocument({
        parser: {
          enableNest: false
        },
        unknownKey: true
      });
      expect(result.parser.enableNest).toBe(false);
      expect(result).not.toHaveProperty('unknownKey');
    });
  });

  describe('settingsJsonSchema', () => {
    it('derives a closed object schema from default settings', () => {
      expect(settingsJsonSchema).toEqual(expect.objectContaining({
        type: 'object',
        additionalProperties: false
      }));
      expect(settingsJsonSchema).toEqual(expect.objectContaining({
        properties: expect.objectContaining({
          formatting: expect.objectContaining({
            type: 'object',
            additionalProperties: false
          })
        })
      }));
    });
  });

  describe('buildEditorTheme', () => {
    it('returns theme with base, rules, and colors', () => {
      const theme = buildEditorTheme(defaultSettings);
      expect(theme.base).toBe('vs');
      expect(theme.inherit).toBe(true);
      expect(theme.rules).toBeInstanceOf(Array);
      expect(theme.rules.length).toBeGreaterThan(0);
      theme.rules.forEach((r: any) => {
        expect(typeof r.token).toBe('string');
        expect(r.foreground).toMatch(/^[0-9a-fA-F]{6}$/);
      });
      expect(theme.colors).toEqual(defaultSettings.editor.uiColors);
    });

    it('maps semantic type colors to semantic tokens', () => {
      const theme = buildEditorTheme(defaultSettings);
      expect(theme.semanticTokenColors).toEqual(expect.objectContaining({
        map: defaultSettings.editor.semanticTypeColors.map,
        key: defaultSettings.editor.semanticTypeColors.key,
        seq: defaultSettings.editor.semanticTypeColors.seq,
        str: defaultSettings.editor.semanticTypeColors.str,
        int: defaultSettings.editor.semanticTypeColors.int,
        float: defaultSettings.editor.semanticTypeColors.float,
        boolean: defaultSettings.editor.semanticTypeColors.boolean,
        nil: defaultSettings.editor.semanticTypeColors.nil,
      }));
      expect(Object.keys(theme.semanticTokenColors)).toEqual(TOKEN_TYPES);
      expect(TREE_NODE_TOKEN_TYPES.every((token) => theme.semanticTokenColors[token] === defaultSettings.editor.semanticTypeColors[token])).toBe(true);
      expect(theme.semanticTokenColors.variable).toBe(defaultSettings.editor.semanticTypeColors[TOKEN_TYPE_THEME_KEY.variable]);
      expect(theme.semanticTokenColors.tag).toBe(defaultSettings.editor.semanticTypeColors[TOKEN_TYPE_THEME_KEY.tag]);
      expect(theme.semanticTokenColors.attribute).toBe(defaultSettings.editor.semanticTypeColors[TOKEN_TYPE_THEME_KEY.attribute]);
      expect(theme.rules.map((r: any) => r.token)).toEqual(TOKEN_TYPES);
      const punctuationRule = theme.rules.find((r: any) => r.token === 'punctuation');
      const variableRule = theme.rules.find((r: any) => r.token === 'variable');
      const tagRule = theme.rules.find((r: any) => r.token === 'tag');
      const attributeRule = theme.rules.find((r: any) => r.token === 'attribute');
      expect(punctuationRule?.foreground).toBe(theme.semanticTokenColors.punctuation.slice(1));
      expect(variableRule?.foreground).toBe(theme.semanticTokenColors.variable.slice(1));
      expect(tagRule?.foreground).toBe(theme.semanticTokenColors.tag.slice(1));
      expect(attributeRule?.foreground).toBe(theme.semanticTokenColors.attribute.slice(1));
    });
  });

  });
