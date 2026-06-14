import { describe, it, expect, vi } from 'vitest';
import { handleError } from './error-handler';

describe('error-handler', () => {
  it('logs structured error to console.error', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    handleError(new Error('test error'), { component: 'Test', operation: 'run' });
    expect(spy).toHaveBeenCalledOnce();
    expect(spy.mock.calls[0][0]).toContain('[Test]');
    expect(spy.mock.calls[0][0]).toContain('run failed');
    spy.mockRestore();
  });
});
