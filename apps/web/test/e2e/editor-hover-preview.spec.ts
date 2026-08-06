import { expect, test } from './fixtures';
import {
  expectMonacoHoverContains,
  openMonacoHover,
  readEditorState,
  setEditorContent,
  waitForEditorReady,
} from './utils';

const multiLanguageColorCases = [
  { label: 'YAML', languageId: 'yaml', sourceText: 'color: "#4f46e5"' },
] as const;

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
  // This case owns Monaco's TOML hover only. The graph runtime currently emits
  // a diagnostic for the same TOML source, which is covered by graph tests.
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });

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
