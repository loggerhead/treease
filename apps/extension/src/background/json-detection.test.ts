import { describe, expect, it } from 'vitest';
import { detectStrictJson, detectStructuredCandidate } from './json-detection';

describe('detectStrictJson', () => {
  it.each([
    ['{}', 'object'], ['[]', 'array'], ['true', 'scalar'], ['"treease"', 'scalar'], ['null', 'scalar'],
  ] as const)('accepts strict %s as %s', (text, rootType) => {
    expect(detectStrictJson(text)).toEqual({ status: 'valid', rootType });
  });

  it.each(["{'name':'Ada'}", '{name:"Ada"}', 'answer: 42'])('rejects non-JSON syntax', (text) => {
    expect(detectStrictJson(text).status).toBe('invalid');
  });
});

describe('detectStructuredCandidate', () => {
  it('extracts only a strict embedded JSON block', () => {
    expect(detectStructuredCandidate('response: {"name":"Ada","active":true}')).toMatchObject({
      status: 'valid', language: 'json', candidateKind: 'embedded', text: '{"name":"Ada","active":true}', rootType: 'object',
    });
  });

  it('does not turn JavaScript object literals into JSON', () => {
    expect(detectStructuredCandidate("const item = {'name':'Ada'};").status).toBe('invalid');
  });

  it.each([['name: Ada', 'yaml'], ['name = "Ada"', 'toml']] as const)('classifies %s for Core parsing', (text, language) => {
    expect(detectStructuredCandidate(text)).toMatchObject({ status: 'candidate', language });
  });
});
