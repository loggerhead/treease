import { describe, expect, it } from 'vitest';
import { extractCandidate } from './extract-candidate';

describe('extractCandidate', () => {
  it('reads and unfences JSON in a code element', () => {
    document.body.innerHTML = '<pre><code>```json\n\uFEFF {"name":"Ada"}\n```</code></pre>';
    expect(extractCandidate(document.querySelector('code'))).toEqual({
      status: 'candidate', text: '{"name":"Ada"}', sourceTag: 'code', sourceLength: 14,
    });
  });

  it('removes YAML and TOML Markdown fences before Core classification', () => {
    document.body.innerHTML = '<pre>```yaml\nname: Ada\n```</pre>';
    expect(extractCandidate(document.querySelector('pre'))).toMatchObject({ status: 'candidate', text: 'name: Ada' });
  });

  it('prefers a textarea value', () => {
    document.body.innerHTML = '<textarea>{"enabled":true}</textarea>';
    expect(extractCandidate(document.querySelector('textarea'))).toMatchObject({
      status: 'candidate', text: '{"enabled":true}', sourceTag: 'textarea',
    });
  });

  it('walks past a GitHub-style highlighted token to the complete code cell', () => {
    document.body.innerHTML = '<table><tbody><tr><td class="blob-code">[{"id":"<span class="pl-s">V59FY2YF62</span>","ok":true}]</td></tr></tbody></table>';
    const token = document.querySelector('.pl-s');
    expect(extractCandidate(token)).toEqual({
      status: 'candidate',
      text: '[{"id":"V59FY2YF62","ok":true}]',
      sourceTag: 'td',
      sourceLength: 31,
    });
  });

  it('extracts JSON from a Feishu code block without reading the surrounding document', () => {
    document.body.innerHTML = '<div class="editor-kit-code-block code-block"><div class="code-block-header"><span>JSON</span></div><div class="code-block-content"><div class="zone-container code-block-zone-container" contenteditable="true"><div class="ace-line"><span class="code-hljs-punctuation">{</span><span class="code-hljs-attr">"ok"</span><span>:</span><span>true}</span></div></div></div></div>';
    const token = document.querySelector('.code-hljs-attr');
    expect(extractCandidate(token)).toEqual({
      status: 'candidate', text: '{"ok":true}', sourceTag: 'div', sourceLength: 11,
    });
  });

  it('accepts a raw-page pre containing a 128 KB JSON document', () => {
    const pre = document.createElement('pre');
    pre.textContent = JSON.stringify([{ payload: 'x'.repeat(128 * 1024) }]);
    document.body.replaceChildren(pre);
    const result = extractCandidate(pre);
    expect(result).toMatchObject({ status: 'candidate', sourceTag: 'pre' });
    if (result.status === 'candidate') expect(result.sourceLength).toBeLessThan(1024 * 1024);
  });

  it('never reads passwords or the document body', () => {
    document.body.innerHTML = '<input type="password" value="{&quot;secret&quot;:true}"><span>plain text</span>';
    expect(extractCandidate(document.querySelector('input'))).toEqual({ status: 'none' });
    expect(extractCandidate(document.body)).toEqual({ status: 'none' });
  });

  it('rejects a candidate above the byte limit', () => {
    const pre = document.createElement('pre');
    pre.textContent = 'x'.repeat(1024 * 1024 + 1);
    document.body.replaceChildren(pre);
    expect(extractCandidate(pre)).toMatchObject({ status: 'too_large', sourceTag: 'pre' });
  });

  it('keeps ordinary click extraction below the 5 ms per-click budget', () => {
    document.body.innerHTML = '<main><section><span>ordinary text</span></section></main>';
    const target = document.querySelector('span');
    const startedAt = performance.now();
    for (let index = 0; index < 1_000; index += 1) extractCandidate(target);
    expect((performance.now() - startedAt) / 1_000).toBeLessThan(5);
  });

  it('can extract a target inside an open shadow root without scanning the host page', () => {
    const host = document.createElement('div');
    const root = host.attachShadow({ mode: 'open' });
    const code = document.createElement('code');
    code.textContent = '{"shadow":true}';
    root.append(code);
    document.body.replaceChildren(host);
    expect(extractCandidate(code)).toMatchObject({ status: 'candidate', text: '{"shadow":true}', sourceTag: 'code' });
  });

  it('does not expose text from a closed shadow root through its host', () => {
    const host = document.createElement('div');
    const root = host.attachShadow({ mode: 'closed' });
    const code = document.createElement('code');
    code.textContent = '{"privateShadow":true}';
    root.append(code);
    document.body.replaceChildren(host);
    expect(extractCandidate(host)).toEqual({ status: 'none' });
  });
});
