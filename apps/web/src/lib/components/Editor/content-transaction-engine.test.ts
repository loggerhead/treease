import { describe, expect, it, vi } from 'vitest';
import { createContentTransactionEngine } from './content-transaction-engine';

describe('content transaction engine', () => {
  it('runs pure formatting before tokenization without document-runtime state', async () => {
    const call = vi.fn(async (method: string, input: any) => {
      if (method === 'format') return '{\n  "value": 1\n}\n';
      if (method === 'semanticTokens') return { semanticTokens: [0, 0, 7, 2, 0] };
      throw new Error(`unexpected ${method}`);
    });
    const engine = createContentTransactionEngine(call);

    await expect(engine.process({
      channel: 'sidecar-input',
      language: 'json',
      text: '{"value":1}',
      format: {
        indent: 2,
        smart: true,
        maxLineLength: 100,
        maxInlineComplexity: 1,
        maxArrayInlineItems: 6,
        alignObjectArrays: true,
        nest: false,
      },
    })).resolves.toEqual({
      language: 'json',
      text: '{\n  "value": 1\n}\n',
      semanticTokens: [0, 0, 7, 2, 0],
      formatting: 'applied',
    });
    expect(call).toHaveBeenNthCalledWith(1, 'format', expect.objectContaining({ language: 'json' }));
    expect(call).toHaveBeenNthCalledWith(2, 'semanticTokens', {
      language: 'json',
      text: '{\n  "value": 1\n}\n',
    });
  });

  it('commits through the supplied channel sink before projecting tokens', async () => {
    const events: string[] = [];
    const engine = createContentTransactionEngine(async (method) => {
      events.push(method);
      return method === 'semanticTokens' ? { semanticTokens: [1] } : '';
    });
    let revision = 1;
    const status = await engine.run(
      { channel: 'sidecar-input', language: 'json', text: '{"a":1}', format: null },
      { revision },
      {
        isDocumentCurrent: (target) => target.revision === revision,
        commit: (target, value) => {
          events.push(`commit:${value.text}`);
          revision += 1;
          return { revision: target.revision + 1 };
        },
        isVisibleCurrent: () => true,
        project: (_target, result) => events.push(`project:${result.semanticTokens.join(',')}`),
      },
    );
    expect(status).toBe('committed');
    expect(events).toEqual(['commit:{"a":1}', 'semanticTokens', 'project:1']);
  });

  it('does not commit or project an obsolete formatting operation', async () => {
    let current = true;
    const engine = createContentTransactionEngine(async () => {
      current = false;
      return '{}';
    });
    const commit = vi.fn();
    await expect(engine.run(
      { channel: 'sidecar-input', language: 'json', text: '{}', format: { indent: 2, smart: true, maxLineLength: 100, maxInlineComplexity: 1, maxArrayInlineItems: 6, alignObjectArrays: true, nest: false } },
      'target',
      { isDocumentCurrent: () => current, commit, isVisibleCurrent: () => true, project: vi.fn() },
    )).resolves.toBe('stale');
    expect(commit).not.toHaveBeenCalled();
  });

  it('accepts invalid source as an explicit unformatted content outcome', async () => {
    const engine = createContentTransactionEngine(async (method) => {
      if (method === 'format') throw new Error('invalid JSON');
      return { semanticTokens: [] };
    });
    await expect(engine.process({
      channel: 'sidecar-input', language: 'json', text: '{',
      format: { indent: 2, smart: true, maxLineLength: 100, maxInlineComplexity: 1, maxArrayInlineItems: 6, alignObjectArrays: true, nest: false },
    })).resolves.toMatchObject({ text: '{', formatting: 'invalid-source' });
  });
});
