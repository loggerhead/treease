import { expect, test } from './fixtures';
import {
  expectMonacoHoverContains,
  openMonacoHover,
  readEditorState,
  setEditorContent,
  waitForEditorReady,
} from './utils';

const SAMPLE_JWT = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJuYW1lIjoiQWxpY2UifQ.signature';
const multiLanguageColorCases = [
  { label: 'YAML', languageId: 'yaml', sourceText: 'color: "#4f46e5"' },
  { label: 'TOML', languageId: 'toml', sourceText: 'color = "#4f46e5"' },
] as const;

async function renderPreview(page: import('@playwright/test').Page, value: string, rawValue = JSON.stringify(value)) {
  await page.goto('/editor');
  await waitForEditorReady(page);
  return page.evaluate(
    async ({ value, rawValue }) => {
      const treease = window._treease;
      if (!treease) throw new Error('window._treease is unavailable');
      return treease.preview.generate({
        value,
        rawValue,
        language: 'json',
      });
    },
    { value, rawValue },
  );
}

test('shows URL preview content', async ({ page }) => {
  const preview = await renderPreview(page, 'https://example.com/docs?tab=preview');
  expect(preview).toEqual(
    expect.arrayContaining([
      expect.stringContaining('<strong>Protocol</strong>'),
      expect.stringContaining('https'),
      expect.stringContaining('<strong>Host</strong>'),
      expect.stringContaining('example.com'),
      expect.stringContaining('<strong>Path</strong>'),
      expect.stringContaining('/docs'),
      expect.stringContaining('<strong>Query</strong>'),
      expect.stringContaining('<strong>tab</strong>'),
      expect.stringContaining('preview'),
    ]),
  );
});

test('shows color preview content', async ({ page }) => {
  const preview = await renderPreview(page, '#ff0000');
  expect(preview).toEqual(
    expect.arrayContaining([
      expect.stringContaining('background-color:#ff0000'),
      expect.stringContaining('<strong>HEX</strong>'),
      expect.stringContaining('#ff0000'),
      expect.stringContaining('<strong>RGB</strong>'),
      expect.stringContaining('rgb(255, 0, 0)'),
      expect.stringContaining('<strong>HSL</strong>'),
      expect.stringContaining('hsl(0, 100%, 50%)'),
    ]),
  );
});

test('shows unicode preview content', async ({ page }) => {
  const preview = await renderPreview(page, '你好', '"\\u4f60\\u597d"');
  expect(preview).toBe('<pre>你好</pre>');
});

test('shows jwt preview content', async ({ page }) => {
  const preview = await renderPreview(page, SAMPLE_JWT);
  expect(preview).toEqual([
    '<div><strong>JWT Header</strong></div>',
    '<pre>{\n  &quot;alg&quot;: &quot;HS256&quot;,\n  &quot;typ&quot;: &quot;JWT&quot;\n}</pre>',
    '<div><strong>JWT Payload</strong></div>',
    '<pre>{\n  &quot;sub&quot;: &quot;123&quot;,\n  &quot;name&quot;: &quot;Alice&quot;\n}</pre>',
    '<div><strong>Signature Length: 9</strong></div>',
  ]);
});

test('shows base64 preview content', async ({ page }) => {
  const preview = await renderPreview(page, 'SGVsbG8gd29ybGQ=');
  expect(preview).toEqual(['<div><strong>Base64 Decoded</strong></div>', '<pre>Hello world</pre>']);
});

test('shows uri preview content', async ({ page }) => {
  const preview = await renderPreview(page, 'hello%20world%2Ftree');
  expect(preview).toEqual(['<div><strong>URI Decoded</strong></div>', '<pre>hello world/tree</pre>']);
});

test('shows image preview content', async ({ page }) => {
  const preview = await renderPreview(page, 'https://example.com/avatar.png');
  expect(preview).toBe('<img src="https://example.com/avatar.png">');
});

test('shows date preview content with stable fields', async ({ page }) => {
  const preview = await renderPreview(page, '2026-04-13');
  expect(Array.isArray(preview)).toBe(false);
  expect(typeof preview).toBe('string');
  const text = String(preview);
  expect(text).toContain('<strong>ISO</strong>');
  expect(text).toContain('2026-04-13T');
  expect(text).toContain('<strong>Timestamp</strong>');
  expect(text).toContain('<strong>RelativeTime</strong>');
});

async function replaceSourceEditorText(page: import('@playwright/test').Page, sourceText: string) {
  await setEditorContent(page, { sourceText });
}

async function selectLanguage(page: import('@playwright/test').Page, label: string) {
  await page.getByRole('button', { name: 'Language', exact: true }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
}

test('color picker and hover preview coexist in the source editor', async ({ page, browserName }) => {
  test.skip(browserName !== 'chromium', 'Monaco color picker assertions are only covered in chromium');

  await page.goto('/editor');
  await waitForEditorReady(page);
  await replaceSourceEditorText(page, '{"color":"#4f46e5"}');

  const decoration = page.locator('.colorpicker-color-decoration').first();
  await expect(decoration).toBeVisible({ timeout: 5_000 });
  await decoration.click();
  await expect(page.locator('.colorpicker-widget')).toBeVisible({ timeout: 5_000 });

  await expectMonacoHoverContains(page, ['HEX', '#4f46e5', 'rgb(79, 70, 229)', 'hsl(243, 75%, 59%)']);
});

test('shows hover preview for TOML color values', async ({ page, browserName }) => {
  test.skip(browserName !== 'chromium', 'Monaco hover assertions are only covered in chromium');

  await page.goto('/editor');
  await waitForEditorReady(page);
  await selectLanguage(page, 'TOML');
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('toml');
  await replaceSourceEditorText(page, 'color = "#4f46e5"');

  await openMonacoHover(page, {
    hookId: 'source-editor',
    lineNumber: 1,
    column: 12,
    hoverText: '#4f46e5',
  });
  await expectMonacoHoverContains(page, ['HEX', '#4f46e5', 'rgb(79, 70, 229)', 'hsl(243, 75%, 59%)']);
});

for (const testCase of multiLanguageColorCases) {
  test(`color picker appears for ${testCase.label}`, async ({ page, browserName }) => {
    test.skip(browserName !== 'chromium', 'Monaco color picker assertions are only covered in chromium');

    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText: testCase.sourceText,
      language: testCase.languageId,
    });
    await expect
      .poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 })
      .toBe(testCase.languageId);

    const decoration = page.locator('.colorpicker-color-decoration').first();
    await expect(decoration).toBeVisible({ timeout: 5_000 });
    await decoration.click();
    await expect(page.locator('.colorpicker-widget')).toBeVisible({ timeout: 5_000 });
    await expectMonacoHoverContains(page, ['HEX', '#4f46e5']);
  });
}
