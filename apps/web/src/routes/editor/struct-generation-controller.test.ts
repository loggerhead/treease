// Responsibility: unit tests for structure-generation source preparation.
import { describe, expect, it, vi } from 'vitest';
import { prepareStructGenerationSource } from './struct-generation-controller';

const formatting = {
  indent: 2,
  smart: true,
  maxLineLength: 120,
  maxInlineComplexity: 4,
  maxArrayInlineItems: 8,
  alignObjectArrays: true,
};

describe('prepareStructGenerationSource', () => {
  it('keeps JSON source unchanged without invoking the converter', async () => {
    const callWorker = vi.fn();

    await expect(prepareStructGenerationSource({
      text: '{"name":"Treease"}',
      language: 'json',
      formatting,
      callWorker,
    })).resolves.toBe('{"name":"Treease"}');

    expect(callWorker).not.toHaveBeenCalled();
  });

  it('converts a non-JSON document before structure generation', async () => {
    const callWorker = vi.fn().mockResolvedValue('{\n  "name": "Treease"\n}');

    await expect(prepareStructGenerationSource({
      text: 'name: Treease\n',
      language: 'yaml',
      formatting,
      callWorker,
    })).resolves.toBe('{\n  "name": "Treease"\n}');

    expect(callWorker).toHaveBeenCalledWith('convert', {
      sourceLanguage: 'yaml',
      targetFormat: 'json',
      text: 'name: Treease\n',
      options: formatting,
    });
  });
});
