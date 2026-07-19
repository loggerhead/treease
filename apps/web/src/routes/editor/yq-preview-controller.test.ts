// Responsibility: unit tests for yq-preview-controller.
import { describe, expect, it, vi } from 'vitest';
import { resolveYqPreviewLanguage, runYqPreview, toFriendlyYqErrorMessage } from './yq-preview-controller';

const formatting = {
  indent: 2,
  smart: true,
  maxLineLength: 120,
  maxInlineComplexity: 4,
  maxArrayInlineItems: 8,
  alignObjectArrays: true,
};

describe('yq-preview-controller', () => {
  it('resolves preview language from expression codecs', () => {
    expect(resolveYqPreviewLanguage('.items | to_yaml', 'json')).toBe('yaml');
    expect(resolveYqPreviewLanguage('.items | @toml', 'json')).toBe('toml');
    expect(resolveYqPreviewLanguage('.items[]', 'json')).toBe('json');
  });

  it('maps common errors to friendly messages', () => {
    expect(toFriendlyYqErrorMessage(new Error('Expression is required'))).toBe('Enter a yq expression.');
    expect(toFriendlyYqErrorMessage(new Error('parse failed: bad json'))).toContain('cannot be parsed');
    expect(toFriendlyYqErrorMessage(new Error('Parse(ParticipleLexer(UnknownToken { offset: 0, lexeme: "invalid" }))'))).toContain('could not be executed');
  });

  it('returns validation error for empty expression', async () => {
    const callWorker = vi.fn();
    await expect(runYqPreview({ expression: '  ', text: '{}', language: 'json', formatting, enableNest: true, callWorker })).resolves.toEqual({
      ok: false,
      error: 'Enter a yq expression.',
    });
    expect(callWorker).not.toHaveBeenCalled();
  });

  it('calls worker and returns preview payload', async () => {
    const callWorker = vi.fn().mockResolvedValue('a: 1');
    const result = await runYqPreview({ expression: 'to_yaml', text: '{"a":1}', language: 'json', formatting, enableNest: false, callWorker });
    expect(result).toEqual({ ok: true, result: 'a: 1', previewLanguage: 'yaml' });
    expect(callWorker).toHaveBeenCalledWith('runYq', expect.objectContaining({ language: 'json', expression: 'to_yaml' }));
  });

  it('returns display-ready preview payloads without UI normalization', async () => {
    const callWorker = vi
      .fn()
      .mockResolvedValueOnce('Alice')
      .mockResolvedValueOnce('a: 1\n')
      .mockResolvedValueOnce('{"a":1}');

    await expect(
      runYqPreview({ expression: '.items[0].name', text: '{"items":[{"name":"Alice"}]}', language: 'json', formatting, enableNest: false, callWorker }),
    ).resolves.toEqual({ ok: true, result: 'Alice', previewLanguage: 'json' });

    await expect(
      runYqPreview({ expression: 'to_yaml', text: '{"a":1}', language: 'json', formatting, enableNest: false, callWorker }),
    ).resolves.toEqual({ ok: true, result: 'a: 1\n', previewLanguage: 'yaml' });

    await expect(
      runYqPreview({ expression: '@tomld', text: '"a = 1"', language: 'json', formatting, enableNest: false, callWorker }),
    ).resolves.toEqual({ ok: true, result: '{"a":1}', previewLanguage: 'json' });
  });
});
