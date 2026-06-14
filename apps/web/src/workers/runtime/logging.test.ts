import { describe, it, expect } from 'vitest';
import { describeError } from './logging';

describe('logging', () => {
  describe('describeError', () => {
    it('extracts name, message, stack from Error', () => {
      const err = new Error('test');
      const result = describeError(err);
      expect(result.name).toBe('Error');
      expect(result.message).toBe('test');
      expect(result.stack).toContain('Error');
    });

    it('wraps non-Error in UnknownError', () => {
      const result = describeError('string error');
      expect(result.name).toBe('UnknownError');
      expect(result.message).toBe('string error');
    });

    it('handles null', () => {
      const result = describeError(null);
      expect(result.name).toBe('UnknownError');
      expect(result.message).toBe('null');
    });
  });
});
