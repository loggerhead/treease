// 职责：preview 模块集成测试：各类型预览值生成
import { describe, expect, it } from 'vitest';
import { valueToTreeNode } from '../../shared/tree-node-value';
import { generatePreview } from './index';

describe('generatePreview', () => {
  const sampleJwt =
    'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJuYW1lIjoiQWxpY2UifQ.signature';

  it('prefers image preview over generic URL preview', async () => {
    const node = valueToTreeNode('https://example.com/avatar.png');

    const preview = await generatePreview({
      node,
      value: 'https://example.com/avatar.png',
      rawValue: '"https://example.com/avatar.png"',
      language: 'json',
    });

    expect(preview).toBe(`<img src="https://example.com/avatar.png">`);
  });

  it('keeps non-image urls on the url preview branch', async () => {
    const node = valueToTreeNode('https://example.com/docs?tab=preview');

    const preview = await generatePreview({
      node,
      value: 'https://example.com/docs?tab=preview',
      rawValue: '"https://example.com/docs?tab=preview"',
      language: 'json',
    });

    expect(preview).toEqual([
      '<table><tr><td><strong>Protocol</strong></td><td> \t　</td><td>https</td></tr><tr><td><strong>Host</strong></td><td> \t　</td><td>example.com</td></tr><tr><td><strong>Path</strong></td><td> \t　</td><td>/docs</td></tr></table>',
      '<div><strong>Query</strong></div>',
      '<table><tr><td><strong>tab</strong></td><td> \t　</td><td>preview</td></tr></table>',
    ]);
  });

  it('keeps full URLs on the URL preview branch before URI decoding', async () => {
    const node = valueToTreeNode('https://example.com/docs?tab=preview');

    const preview = await generatePreview({
      node,
      value: 'https://example.com/docs?tab=preview',
      rawValue: '"https://example.com/docs?tab=preview"',
      language: 'json',
    });

    expect(Array.isArray(preview)).toBe(true);
    expect((preview as string[]).join('\n')).toContain('Host');
    expect((preview as string[]).join('\n')).not.toContain('URI Decoded');
  });

  it('detects color preview for hexa values', async () => {
    const node = valueToTreeNode('#4f46e580');

    const preview = await generatePreview({
      node,
      value: '#4f46e580',
      rawValue: '"#4f46e580"',
      language: 'json',
    });

    expect(preview).toEqual([
      '<div style="width:128px;height:16px;background-color:#4f46e580;border:1px solid #cbd5e1;border-radius:4px;"></div>',
      '<table><tr><td><strong>HEX</strong></td><td> \t　</td><td>#4f46e580</td></tr><tr><td><strong>RGB</strong></td><td> \t　</td><td>rgba(79, 70, 229, 0.5)</td></tr><tr><td><strong>HSL</strong></td><td> \t　</td><td>hsla(243, 75%, 59%, 0.5)</td></tr></table>',
    ]);
  });

  it('detects unicode preview from raw literal text', async () => {
    const node = valueToTreeNode('你好');

    const preview = await generatePreview({
      node,
      value: '你好',
      rawValue: '"\\u4f60\\u597d"',
      language: 'json',
    });

    expect(preview).toBe('<pre>你好</pre>');
  });

  it('keeps JWT values on the JWT preview branch before generic base64', async () => {
    const node = valueToTreeNode(sampleJwt);

    const preview = await generatePreview({
      node,
      value: sampleJwt,
      rawValue: JSON.stringify(sampleJwt),
      language: 'json',
    });

    expect(Array.isArray(preview)).toBe(true);
    expect((preview as string[]).join('\n')).toContain('JWT Header');
    expect((preview as string[]).join('\n')).not.toContain('Base64 Decoded');
  });

  it('decodes standalone base64 strings', async () => {
    const node = valueToTreeNode('SGVsbG8gd29ybGQ=');

    const preview = await generatePreview({
      node,
      value: 'SGVsbG8gd29ybGQ=',
      rawValue: '"SGVsbG8gd29ybGQ="',
      language: 'json',
    });

    expect(preview).toEqual(['<div><strong>Base64 Decoded</strong></div>', '<pre>Hello world</pre>']);
  });

  it('decodes percent-encoded URI fragments that are not full URLs', async () => {
    const node = valueToTreeNode('hello%20world%2Ftree');

    const preview = await generatePreview({
      node,
      value: 'hello%20world%2Ftree',
      rawValue: '"hello%20world%2Ftree"',
      language: 'json',
    });

    expect(preview).toEqual(['<div><strong>URI Decoded</strong></div>', '<pre>hello world/tree</pre>']);
  });

  it('renders stable date preview fields for ISO-like values', async () => {
    const node = valueToTreeNode('2026-04-13');

    const preview = await generatePreview({
      node,
      value: '2026-04-13',
      rawValue: '"2026-04-13"',
      language: 'json',
    });

    expect(typeof preview).toBe('string');
    expect(preview).toContain('<strong>ISO</strong>');
    expect(preview).toContain('2026-04-13T');
    expect(preview).toContain('<strong>Timestamp</strong>');
    expect(preview).toContain('<strong>RelativeTime</strong>');
  });
});
