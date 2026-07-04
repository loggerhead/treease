import { describe, it, expect } from 'vitest';
import { formatScalarLiteral, formatStructuredPreview, resolveGraphCellDisplayText } from './literal-display';

describe('literal-display', () => {
  describe('formatScalarLiteral', () => {
    it('returns text unchanged for non-python', () => {
      expect(formatScalarLiteral('true', 'boolean', 'json')).toBe('true');
    });

    it('converts true → True for python', () => {
      expect(formatScalarLiteral('true', 'boolean', 'python')).toBe('True');
    });

    it('converts false → False for python', () => {
      expect(formatScalarLiteral('false', 'boolean', 'python')).toBe('False');
    });

    it('converts null → None for python', () => {
      expect(formatScalarLiteral('null', 'null', 'python')).toBe('None');
    });

    it('leaves non-boolean/null text unchanged for python', () => {
      expect(formatScalarLiteral('hello', 'string', 'python')).toBe('hello');
    });

    it('leaves non-matching boolean text unchanged for python', () => {
      expect(formatScalarLiteral('yes', 'boolean', 'python')).toBe('yes');
    });

    it('leaves non-matching null text unchanged for python', () => {
      expect(formatScalarLiteral('None', 'null', 'python')).toBe('None');
    });

    it('handles undefined language', () => {
      expect(formatScalarLiteral('true', 'boolean')).toBe('true');
    });

    it('shows empty strings as double quotes', () => {
      expect(formatScalarLiteral('', 'string', 'json')).toBe('""');
    });

    it('shows empty null placeholders as null', () => {
      expect(formatScalarLiteral('', 'null', 'json')).toBe('null');
    });
  });

  describe('resolveGraphCellDisplayText', () => {
    it('falls back to the scalar value when text is empty', () => {
      expect(resolveGraphCellDisplayText('', '', 'null', 'json')).toBe('null');
      expect(resolveGraphCellDisplayText('', '', 'string', 'json')).toBe('""');
    });

    it('prefers explicit text when present', () => {
      expect(resolveGraphCellDisplayText('Alice', 'ignored', 'string', 'json')).toBe('Alice');
    });
  });

  describe('formatStructuredPreview', () => {
    it('returns JSON.stringify for non-python', () => {
      const result = formatStructuredPreview({ a: 1 });
      expect(result).toBe(JSON.stringify({ a: 1 }, null, 2));
    });

    it('formats python dict', () => {
      const result = formatStructuredPreview({ a: 1 }, 'python');
      expect(result).toContain('"a"');
      expect(result).toContain('1');
    });

    it('formats python list', () => {
      const result = formatStructuredPreview([1, 2, 3], 'python');
      expect(result).toContain('1');
      expect(result).toContain('2');
    });

    it('formats python booleans as True/False', () => {
      const result = formatStructuredPreview({ flag: true }, 'python');
      expect(result).toContain('True');
    });

    it('formats python null as None', () => {
      const result = formatStructuredPreview({ val: null }, 'python');
      expect(result).toContain('None');
    });

    it('formats empty array as []', () => {
      expect(formatStructuredPreview([], 'python')).toBe('[]');
    });

    it('formats empty object as {}', () => {
      expect(formatStructuredPreview({}, 'python')).toBe('{}');
    });

    it('formats nested structures', () => {
      const result = formatStructuredPreview({ a: { b: [1, null, true] } }, 'python');
      expect(result).toContain('None');
      expect(result).toContain('True');
    });
  });
});
