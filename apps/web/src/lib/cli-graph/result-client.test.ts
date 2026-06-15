import { describe, expect, it, vi } from 'vitest';
import { fetchCliGraphResult, normalizeCliGraphResult, readCliGraphTokenFromSearch } from './result-client';

describe('cli graph result client', () => {
  it('reads token from search params', () => {
    expect(readCliGraphTokenFromSearch('?token=abc123')).toBe('abc123');
    expect(readCliGraphTokenFromSearch('?other=value')).toBe('');
    expect(readCliGraphTokenFromSearch('')).toBe('');
  });

  it('fetches and normalizes the CLI graph result payload', async () => {
    const fetcher = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          source_label: 'input.json',
          expression: '.items',
          language: 'json',
          text: '[1,2]',
        }),
        { status: 200 },
      ),
    );

    await expect(fetchCliGraphResult('secret', fetcher)).resolves.toEqual({
      sourceLabel: 'input.json',
      expression: '.items',
      language: 'json',
      text: '[1,2]',
    });
    expect(fetcher).toHaveBeenCalledWith('/cli/result?token=secret');
  });

  it('URL-encodes the token when fetching', async () => {
    const fetcher = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          source_label: '-',
          expression: '.',
          language: 'yaml',
          text: 'a: 1\n',
        }),
        { status: 200 },
      ),
    );

    await fetchCliGraphResult('a b', fetcher);

    expect(fetcher).toHaveBeenCalledWith('/cli/result?token=a%20b');
  });

  it('rejects missing tokens before fetching', async () => {
    const fetcher = vi.fn();

    await expect(fetchCliGraphResult('', fetcher)).rejects.toThrow('Missing CLI graph token');
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('rejects non-ok responses with status', async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response('forbidden', { status: 403 }));

    await expect(fetchCliGraphResult('secret', fetcher)).rejects.toThrow('Failed to load CLI graph result: HTTP 403');
  });

  it('rejects invalid payload shapes', async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify({ source_label: 'input.json' }), { status: 200 }));

    await expect(fetchCliGraphResult('secret', fetcher)).rejects.toThrow('Invalid CLI graph result payload');
    expect(() => normalizeCliGraphResult(null)).toThrow('Invalid CLI graph result payload');
  });
});
