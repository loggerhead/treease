import { describe, expect, it, vi } from 'vitest';
import { fetchCliGraphResult, normalizeCliGraphResult, readCliGraphTokenFromSearch } from './result-client';

describe('cli graph result client', () => {
  it('reads token from search params', () => {
    expect(readCliGraphTokenFromSearch('?token=abc123')).toBe('abc123');
    expect(readCliGraphTokenFromSearch('?other=value')).toBe('');
    expect(readCliGraphTokenFromSearch('')).toBe('');
  });

  it('fetches metadata and source text for a CLI graph result', async () => {
    const fetcher = vi.fn(async (input: string) => {
      if (input === '/cli/meta?token=secret') {
        return new Response(
          JSON.stringify({
            source_label: 'input.json',
            expression: '.items',
            language: 'json',
            source_url: '/cli/source?token=secret',
          }),
          { status: 200 },
        );
      }
      if (input === '/cli/source?token=secret') {
        return new Response('[1,2]', { status: 200 });
      }
      return new Response('not found', { status: 404 });
    });

    await expect(fetchCliGraphResult('secret', fetcher)).resolves.toEqual({
      sourceLabel: 'input.json',
      expression: '.items',
      language: 'json',
      text: '[1,2]',
    });
    expect(fetcher).toHaveBeenNthCalledWith(1, '/cli/meta?token=secret');
    expect(fetcher).toHaveBeenNthCalledWith(2, '/cli/source?token=secret');
  });

  it('URL-encodes the token when fetching', async () => {
    const fetcher = vi.fn(async (input: string) => {
      if (input === '/cli/meta?token=a%20b') {
        return new Response(
          JSON.stringify({
            source_label: '-',
            expression: '.',
            language: 'yaml',
            source_url: '/cli/source?token=a%20b',
          }),
          { status: 200 },
        );
      }
      return new Response('a: 1\n', { status: 200 });
    });

    await fetchCliGraphResult('a b', fetcher);

    expect(fetcher).toHaveBeenNthCalledWith(1, '/cli/meta?token=a%20b');
    expect(fetcher).toHaveBeenNthCalledWith(2, '/cli/source?token=a%20b');
  });

  it('rejects missing tokens before fetching', async () => {
    const fetcher = vi.fn();

    await expect(fetchCliGraphResult('', fetcher)).rejects.toThrow('Missing CLI graph token');
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('rejects non-ok responses with status', async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response('forbidden', { status: 403 }));

    await expect(fetchCliGraphResult('secret', fetcher)).rejects.toThrow('Failed to load CLI graph metadata: HTTP 403');
  });

  it('rejects invalid payload shapes', async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify({ source_label: 'input.json' }), { status: 200 }));

    await expect(fetchCliGraphResult('secret', fetcher)).rejects.toThrow('Invalid CLI graph metadata payload');
    expect(() => normalizeCliGraphResult(null)).toThrow('Invalid CLI graph result payload');
  });
});
