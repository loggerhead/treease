import { describe, expect, it } from 'vitest';
import { extractPureJsonPage } from './pure-json-page';

describe('extractPureJsonPage', () => {
  it('recognizes Chrome raw JSON documents without scanning ordinary pages', () => {
    Object.defineProperty(document, 'contentType', { configurable: true, value: 'text/plain' });
    document.body.innerHTML = '<pre>{"enabled":true}</pre>';
    expect(extractPureJsonPage()).toMatchObject({ status: 'candidate', text: '{"enabled":true}', sourceTag: 'pre' });
    document.body.innerHTML = '<main><pre>{"enabled":true}</pre></main>';
    expect(extractPureJsonPage()).toEqual({ status: 'none' });
  });

  it('rejects a raw document that is not strict JSON', () => {
    Object.defineProperty(document, 'contentType', { configurable: true, value: 'application/json' });
    document.body.innerHTML = '<pre>{enabled: true}</pre>';
    expect(extractPureJsonPage()).toEqual({ status: 'none' });
  });
});
