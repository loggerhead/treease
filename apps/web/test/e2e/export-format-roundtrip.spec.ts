import { expect, test } from './fixtures';
import { readEditorState, setEditorContent, waitForEditorReady } from './utils';

async function selectExportFormat(page: import('@playwright/test').Page, label: string) {
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  const panel = page.getByTestId('export-panel');
  await expect(panel).toBeVisible();
  await panel.getByRole('button').first().click();
  await page.getByRole('option', { name: label, exact: true }).click();
  await expect(panel.getByRole('button').first()).toContainText(label);
}

async function downloadExport(page: import('@playwright/test').Page) {
  const panel = page.getByTestId('export-panel');
  const downloadPromise = page.waitForEvent('download');
  await panel.getByRole('button', { name: 'Download export file', exact: true }).click();
  const download = await downloadPromise;
  await expect(panel).toHaveCount(0);
  const stream = await download.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream!) {
    chunks.push(Buffer.from(chunk));
  }
  return {
    suggestedFilename: download.suggestedFilename(),
    text: Buffer.concat(chunks).toString('utf8'),
  };
}

test('exports JSON source to YAML through the export panel', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42}',
    language: 'json',
  });

  await selectExportFormat(page, 'YAML');
  const result = await downloadExport(page);

  expect(result.text).toContain('title: Example');
  expect(result.text).toContain('count: 42');
  await expect(page.getByText('Converted JSON to YAML')).toBeVisible();
  await expect(page.getByText('Downloaded YAML file')).toBeVisible();
});

test('exports JSON source to TOML through the export panel', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42}',
    language: 'json',
  });

  await selectExportFormat(page, 'TOML');
  const result = await downloadExport(page);

  expect(result.text).toContain('title = "Example"');
  expect(result.text).toContain('count = 42');
  await expect(page.getByText('Converted JSON to TOML')).toBeVisible();
  await expect(page.getByText('Downloaded TOML file')).toBeVisible();
});


test('keeps editor source stable after export to another format', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42}',
    language: 'json',
  });

  await selectExportFormat(page, 'YAML');
  await downloadExport(page);

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toBe('{"title":"Example","count":42}');
});
