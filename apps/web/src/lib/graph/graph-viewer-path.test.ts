import { describe, it, expect } from 'vitest';
import { PathSegTag } from '@core-wasm/index'
import { buildReadablePath } from '../store/tree-path';
import {
  setValueAtPath,
  getValueAtPath,
  renameKeyAtPath,
  normalizeKeyInput,
  buildPathKey,
} from './graph-viewer-path';

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
  });

  describe('getValueAtPath', () => {
    it('returns nested key value', () => {
      expect(getValueAtPath({ a: { b: 3 } }, [keySeg('a'), keySeg('b')])).toBe(3);
    });

    it('returns array element', () => {
      expect(getValueAtPath({ items: ['x', 'y'] }, [keySeg('items'), indexSeg(1)])).toBe('y');
    });

    it('returns undefined for missing branch', () => {
      expect(getValueAtPath({ a: 1 }, [keySeg('missing')])).toBeUndefined();
    });
  });

  describe('renameKeyAtPath', () => {
    it('renames an object key in place', () => {
      const data = { a: { oldKey: 1 } };
      const result = renameKeyAtPath(data, [keySeg('a'), keySeg('oldKey')], 'newKey') as any;
      expect(result.a.oldKey).toBeUndefined();
      expect(result.a.newKey).toBe(1);
    });

    it('keeps arrays unchanged', () => {
      const data = { list: ['a'] };
      expect(renameKeyAtPath(data, [keySeg('list'), indexSeg(0)], 'newKey')).toBe(data);
    });
  });

  describe('normalizeKeyInput', () => {
    it('parses quoted json string for json', () => {
      expect(normalizeKeyInput('"hello"', 'json')).toBe('hello');
    });

    it('keeps raw value for invalid json string', () => {
      expect(normalizeKeyInput('"hello', 'json')).toBe('"hello');
    });

    it('keeps raw value for non-json language', () => {
      expect(normalizeKeyInput('"hello"', 'yaml')).toBe('"hello"');
    });
  });

  describe('buildPathKey', () => {
    it('returns empty string for empty path', () => {
      expect(buildPathKey([])).toBe('');
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
});
