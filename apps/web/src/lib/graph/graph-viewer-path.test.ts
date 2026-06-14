import { describe, it, expect } from 'vitest';
import { PathSegTag } from '@core-wasm/index'
import { buildReadablePath } from '../store/tree-path';
import {
  setValueAtPath,
  getValueAtPath,
  renameKeyAtPath,
  normalizeKeyInput,
  buildPathKey,
  getCellTooltipText,
  buildTooltipPayload,
  buildTooltipContent,
} from './graph-viewer-path';
import type { GraphCell } from './graph-viewer-render';

// Helper to create PathSeg
function keySeg(key: string): any {
  return { tag: PathSegTag.KEY, key, index: 0 };
}
function indexSeg(index: number): any {
  return { tag: PathSegTag.INDEX, key: '', index };
}

describe('graph-viewer-path', () => {
  describe('setValueAtPath', () => {
    it('returns the value itself when path is empty', () => {
      expect(setValueAtPath({ a: 1 }, [], 42)).toBe(42);
    });

    it('sets a nested key value', () => {
      const data = { a: { b: 1 } };
      const result = setValueAtPath(data, [keySeg('a'), keySeg('b')], 99);
      expect((result as any).a.b).toBe(99);
    });

    it('sets an array element by index', () => {
      const data = { items: [10, 20, 30] };
      const result = setValueAtPath(data, [keySeg('items'), indexSeg(1)], 99) as any;
      expect(result.items[1]).toBe(99);
    });

    it('returns data unchanged when intermediate is null', () => {
      const data = { a: null };
      const result = setValueAtPath(data, [keySeg('a'), keySeg('b')], 99);
      expect(result).toEqual({ a: null });
    });

    it('handles single-segment key path', () => {
      const data = { x: 1 };
      setValueAtPath(data, [keySeg('x')], 42);
      expect(data.x).toBe(42);
    });
  });

  describe('getValueAtPath', () => {
    it('returns root for empty path', () => {
      const data = { a: 1 };
      expect(getValueAtPath(data, [])).toBe(data);
    });

    it('returns undefined for null path', () => {
      expect(getValueAtPath({ a: 1 }, null as any)).toEqual({ a: 1 });
    });

    it('traverses nested object keys', () => {
      expect(getValueAtPath({ a: { b: { c: 3 } } }, [keySeg('a'), keySeg('b'), keySeg('c')])).toBe(3);
    });

    it('traverses array indices', () => {
      expect(getValueAtPath([10, [20, 30]], [indexSeg(1), indexSeg(0)])).toBe(20);
    });

    it('returns undefined for missing key', () => {
      expect(getValueAtPath({ a: 1 }, [keySeg('b')])).toBeUndefined();
    });

    it('returns undefined when traversing through null', () => {
      expect(getValueAtPath({ a: null }, [keySeg('a'), keySeg('b')])).toBeUndefined();
    });
  });

  describe('renameKeyAtPath', () => {
    it('renames a top-level key', () => {
      const data = { old: 1 };
      const result = renameKeyAtPath(data, [keySeg('old')], 'new') as any;
      expect(result.new).toBe(1);
      expect(result.old).toBeUndefined();
    });

    it('renames a nested key', () => {
      const data = { a: { old: 2 } };
      const result = renameKeyAtPath(data, [keySeg('a'), keySeg('old')], 'new') as any;
      expect(result.a.new).toBe(2);
      expect(result.a.old).toBeUndefined();
    });

    it('returns data unchanged for empty path', () => {
      const data = { a: 1 };
      expect(renameKeyAtPath(data, [], 'x')).toBe(data);
    });

    it('returns data unchanged when lastSeg is index', () => {
      const data = [1, 2, 3];
      expect(renameKeyAtPath(data, [indexSeg(0)], 'x')).toBe(data);
    });

    it('returns data unchanged when key equals nextKey (no-op)', () => {
      const data = { a: 1 };
      const result = renameKeyAtPath(data, [keySeg('a')], 'a') as any;
      expect(result.a).toBe(1);
    });

    it('returns data when parent is array (not renameable)', () => {
      const data = [{ a: 1 }];
      expect(renameKeyAtPath(data, [indexSeg(0), keySeg('a')], 'b')).toBe(data);
    });
  });

  describe('normalizeKeyInput', () => {
    it('strips JSON string quotes when language is json', () => {
      expect(normalizeKeyInput('"hello"', 'json')).toBe('hello');
    });

    it('returns raw when language is not json', () => {
      expect(normalizeKeyInput('"hello"', 'yaml')).toBe('"hello"');
    });

    it('returns raw for invalid JSON', () => {
      expect(normalizeKeyInput('not json', 'json')).toBe('not json');
    });

    it('returns raw when JSON parses to non-string', () => {
      expect(normalizeKeyInput('42', 'json')).toBe('42');
    });
  });

  describe('buildPathKey', () => {
    it('returns empty string for empty path', () => {
      expect(buildPathKey([])).toBe('');
    });

    it('returns empty string for null/undefined path', () => {
      expect(buildPathKey(null as any)).toBe('');
    });

    it('builds key-based path', () => {
      expect(buildPathKey([keySeg('a'), keySeg('b')])).toBe('k:a|k:b');
    });

    it('builds mixed key/index path', () => {
      expect(buildPathKey([keySeg('items'), indexSeg(2)])).toBe('k:items|i:2');
    });
  });

  describe('buildReadablePath', () => {
    it('formats root path', () => {
      expect(buildReadablePath([])).toBe('$');
    });

    it('formats nested path with escaped keys', () => {
      expect(buildReadablePath([keySeg('meta'), keySeg('b-c'), indexSeg(2)])).toBe('$.meta["b-c"][2]');
    });
  });

  describe('getCellTooltipText', () => {
    it('shows raw key text for key cell', () => {
      const cell: Partial<GraphCell> = {
        value: 'flags',
        valueType: 'array',
        text: 'flags',
        path: [keySeg('meta'), keySeg('flags')],
      };
      const result = getCellTooltipText({}, cell as GraphCell, 'json', 'key');
      expect(result).toBe('flags');
    });

    it('formats scalar value cell as literal', () => {
      const cell: Partial<GraphCell> = {
        value: '42',
        valueType: 'number',
        text: '42',
        path: [],
      };
      const result = getCellTooltipText({}, cell as GraphCell, 'json', 'value');
      expect(result).toBe('42');
    });

    it('formats meta cell as readable path', () => {
      const data = { nested: { a: 1 } };
      const cell: Partial<GraphCell> = {
        value: '',
        valueType: 'object',
        path: [keySeg('nested')],
      };
      const result = getCellTooltipText(data, cell as GraphCell, 'json', 'meta');
      expect(result).toBe('$.nested');
    });

    it('formats array value cell with structured preview', () => {
      const data = { items: [1, 2, 3] };
      const cell: Partial<GraphCell> = {
        value: '[3]',
        valueType: 'array',
        path: [keySeg('items')],
      };
      const result = getCellTooltipText(data, cell as GraphCell, 'json', 'value');
      expect(result).toContain('1');
    });

    it('does not stringify empty string fallback for object value cells', () => {
      const cell: Partial<GraphCell> = {
        value: '',
        valueType: 'object',
        text: '{1}',
        path: [keySeg('missing')],
      };
      const result = getCellTooltipText({}, cell as GraphCell, 'json', 'value');
      expect(result).toBe('{1}');
    });

    it('falls back to text when scalar value is null', () => {
      const cell: Partial<GraphCell> = {
        value: null as any,
        valueType: 'string',
        text: 'fallback',
        path: [],
      };
      const result = getCellTooltipText({}, cell as GraphCell, 'json', 'value');
      expect(result).toBe('fallback');
    });
  });

  describe('buildTooltipPayload', () => {
    it('returns plain payload for meta path', () => {
      const target = {
        __graphCell: {
          value: '',
          valueType: 'object',
          path: [keySeg('meta'), keySeg('nested')],
        },
        __graphCellKind: 'meta',
      };
      const result = buildTooltipPayload({}, target, 'json');
      expect(result).toEqual({
        text: '$.meta.nested',
        kind: 'meta',
        languageId: 'json',
        valueType: 'object',
        useSyntaxHighlight: false,
      });
    });

    it('falls back to json language for unsupported language id', () => {
      const target = {
        __graphCell: {
          value: 'hello',
          valueType: 'string',
          text: 'hello',
          path: [],
        },
        __graphCellKind: 'value',
      };
      const result = buildTooltipPayload({}, target, 'unsupported');
      expect(result?.languageId).toBe('json');
      expect(result?.text).toBe('hello');
    });

    it('does not use syntax highlight for boolean and null values', () => {
      const booleanTarget = {
        __graphCell: {
          value: 'true',
          valueType: 'boolean',
          text: 'true',
          path: [keySeg('flag')],
        },
        __graphCellKind: 'value',
      };
      const nullTarget = {
        __graphCell: {
          value: 'null',
          valueType: 'null',
          text: 'null',
          path: [keySeg('missing')],
        },
        __graphCellKind: 'value',
      };

      expect(buildTooltipPayload({ flag: true, missing: null }, booleanTarget, 'json')?.useSyntaxHighlight).toBe(false);
      expect(buildTooltipPayload({ flag: true, missing: null }, nullTarget, 'json')?.useSyntaxHighlight).toBe(false);
    });

    it('only marks non-empty object and array values as structured previews', () => {
      const emptyObjectTarget = {
        __graphCell: {
          valueType: 'object',
          text: '{}',
          path: [keySeg('emptyObject')],
        },
        __graphCellKind: 'value',
      };
      const emptyArrayTarget = {
        __graphCell: {
          valueType: 'array',
          text: '[]',
          path: [keySeg('emptyArray')],
        },
        __graphCellKind: 'value',
      };
      const objectTarget = {
        __graphCell: {
          valueType: 'object',
          text: '{...}',
          path: [keySeg('objectValue')],
        },
        __graphCellKind: 'value',
      };
      const arrayTarget = {
        __graphCell: {
          valueType: 'array',
          text: '[...]',
          path: [keySeg('arrayValue')],
        },
        __graphCellKind: 'value',
      };
      const currentData = {
        emptyObject: {},
        emptyArray: [],
        objectValue: { nested: 1 },
        arrayValue: [1],
      };

      expect(buildTooltipPayload(currentData, emptyObjectTarget, 'json')?.useSyntaxHighlight).toBe(false);
      expect(buildTooltipPayload(currentData, emptyArrayTarget, 'json')?.useSyntaxHighlight).toBe(false);
      expect(buildTooltipPayload(currentData, objectTarget, 'json')?.useSyntaxHighlight).toBe(true);
      expect(buildTooltipPayload(currentData, arrayTarget, 'json')?.useSyntaxHighlight).toBe(true);
    });

    it('treats compact non-empty structured summaries as structured previews', () => {
      const compactObjectTarget = {
        __graphCell: {
          valueType: 'object',
          text: '{1}',
          path: [keySeg('missingObject')],
        },
        __graphCellKind: 'value',
      };
      const compactArrayTarget = {
        __graphCell: {
          valueType: 'array',
          text: '[3]',
          path: [keySeg('missingArray')],
        },
        __graphCellKind: 'value',
      };

      expect(buildTooltipPayload({}, compactObjectTarget, 'json')?.useSyntaxHighlight).toBe(true);
      expect(buildTooltipPayload({}, compactArrayTarget, 'json')?.useSyntaxHighlight).toBe(true);
    });
  });

  describe('buildTooltipContent', () => {
    it('returns empty string when no __graphCell', () => {
      expect(buildTooltipContent({}, {})).toBe('');
    });

    it('wraps tooltip in <pre> tag', () => {
      const target = {
        __graphCell: {
          value: 'hello',
          valueType: 'string',
          text: 'hello',
          path: [],
        },
        __graphCellKind: 'value',
      };
      const result = buildTooltipContent({}, target);
      expect(result).toMatch(/^<div class="graph-tooltip-pre-shell"><pre(?:\s+class="[^"]+")?>.*<\/pre><\/div>$/);
      expect(result).toContain('hello');
    });

    it('escapes html in tooltip content', () => {
      const target = {
        __graphCell: {
          value: '<b>x</b>',
          valueType: 'string',
          text: '<b>x</b>',
          path: [],
        },
        __graphCellKind: 'value',
      };
      const result = buildTooltipContent({}, target);
      expect(result).toContain('&lt;b&gt;x&lt;/b&gt;');
      expect(result).not.toContain('<b>x</b>');
    });

    it('adds muted class for meta path tooltip', () => {
      const target = {
        __graphCell: {
          value: '',
          valueType: 'object',
          path: [keySeg('meta'), keySeg('nested')],
        },
        __graphCellKind: 'meta',
      };
      const result = buildTooltipContent({}, target);
      expect(result).toContain('class="graph-tooltip-meta-path"');
      expect(result).toContain('$.meta.nested');
    });
  });

  describe('buildTooltipPayload', () => {
    it('keeps boolean values on pre preview', () => {
      const payload = buildTooltipPayload(
        { enabled: true },
        {
          __graphCell: {
            value: 'true',
            valueType: 'boolean',
            text: 'true',
            path: [keySeg('enabled')],
          } satisfies Partial<GraphCell>,
          __graphCellKind: 'value',
        },
        'json',
      );
      expect(payload?.text).toBe('true');
      expect(payload?.useSyntaxHighlight).toBe(false);
    });

    it('keeps null values on pre preview', () => {
      const payload = buildTooltipPayload(
        { value: null },
        {
          __graphCell: {
            value: 'null',
            valueType: 'null',
            text: 'null',
            path: [keySeg('value')],
          } satisfies Partial<GraphCell>,
          __graphCellKind: 'value',
        },
        'json',
      );
      expect(payload?.text).toBe('null');
      expect(payload?.useSyntaxHighlight).toBe(false);
    });
  });
});
