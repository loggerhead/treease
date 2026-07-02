import { expect, test } from './fixtures';
import {
  getLatestGraphProbes,
  getMonacoScroll,
  getMonacoValue,
  setMonacoScroll,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
  waitForPreviewSettled,
  waitForSettingsReady,
} from './utils';

async function selectExportFormat(page: import('@playwright/test').Page, label: string) {
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  const panel = page.getByTestId('export-panel');
  await expect(panel).toBeVisible();
  await panel.getByRole('button', { name: 'Export format', exact: true }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
  await expect(panel.getByTestId('export-format-trigger')).toContainText(label);
}

test('export preview renders converted text in right panel', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42}',
    language: 'json',
  });

  await selectExportFormat(page, 'YAML');
  await page.getByRole('button', { name: 'Preview export result', exact: true }).click();
  await waitForPreviewSettled(page);
  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toContain('title: Example');
  await expect(page.getByText('Previewed JSON to YAML')).toBeVisible();
});

test('export preview updates right panel when format changes', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42}',
    language: 'json',
  });

  await selectExportFormat(page, 'YAML');
  await page.getByRole('button', { name: 'Preview export result', exact: true }).click();
  await waitForPreviewSettled(page);
  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toContain('title: Example');

  await page.getByRole('button', { name: 'Export', exact: true }).click();
  await selectExportFormat(page, 'TOML');
  await page.getByRole('button', { name: 'Preview export result', exact: true }).click();
  await waitForPreviewSettled(page);
  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toContain('title = "Example"');
  await expect(page.getByText('Previewed JSON to TOML')).toBeVisible();
});

test('viewport collapse toggles viewer and editor panes', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await page.getByRole('button', { name: 'Collapse viewer', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Expand viewer', exact: true })).toBeVisible();
  await expect(page.getByTestId('right-pane')).toHaveAttribute('aria-hidden', 'true');

  await page.getByRole('button', { name: 'Expand viewer', exact: true }).click();
  await expect(page.getByTestId('right-pane')).toHaveAttribute('aria-hidden', 'false');
  await expect(page.getByTestId('right-panel-dropzone')).toBeVisible();

  await page.getByRole('button', { name: 'Collapse editor', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Expand editor', exact: true })).toBeVisible();
  await expect(page.getByTestId('left-pane')).toHaveAttribute('aria-hidden', 'true');

  await page.getByRole('button', { name: 'Expand editor', exact: true }).click();
  await expect(page.getByTestId('left-pane')).toHaveAttribute('aria-hidden', 'false');
  await expect(page.getByTestId('monaco-source-editor')).toBeVisible();
});

test('graph mode remounts graph after switching back from text mode', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"title":"Example","count":42,"items":[1,2,3]}',
    language: 'json',
  });
  await waitForGraphRendered(page);
  await expect(page.getByTestId('graph-viewer-root')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible({ timeout: 5_000 });
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBeGreaterThan(0);

  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });

  await page.getByRole('button', { name: 'Graph mode', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId('graph-viewer-root')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId('graph-search-trigger')).toBeVisible({ timeout: 5_000 });
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBeGreaterThan(0);
});

test('sync scroll toggle stops and resumes left/right text scroll mirroring', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);

  const lines = Array.from({ length: 120 }, (_, index) => `line-${index + 1}: value-${index + 1}`).join('\n');
  await setEditorContent(page, {
    sourceText: lines,
    language: 'yaml',
  });

  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
  await page.getByRole('button', { name: 'Load compare file', exact: true }).click();
  await page.getByLabel('Right panel file input').setInputFiles({
    name: 'sidecar.yaml',
    mimeType: 'application/x-yaml',
    buffer: Buffer.from(lines, 'utf8'),
  });

  await expect.poll(async () => (await getMonacoValue(page, 'right-editor')).includes('line-120: value-120'), {
    timeout: 5_000,
  }).toBe(true);

  await setMonacoScroll(page, 'source-editor', 640);
  await expect.poll(async () => (await getMonacoScroll(page, 'right-editor')).scrollTop, { timeout: 5_000 }).toBe(640);

  await page.getByTestId('sync-scroll-toggle').click();
  await expect(page.getByRole('button', { name: 'Enable synchronized scrolling', exact: true })).toBeVisible();

  await setMonacoScroll(page, 'source-editor', 1280);
  await page.waitForTimeout(150);
  expect((await getMonacoScroll(page, 'right-editor')).scrollTop).toBe(640);

  await page.getByTestId('sync-scroll-toggle').click();
  await expect(page.getByRole('button', { name: 'Disable synchronized scrolling', exact: true })).toBeVisible();

  await setMonacoScroll(page, 'source-editor', 320);
  await expect.poll(async () => (await getMonacoScroll(page, 'right-editor')).scrollTop, { timeout: 5_000 }).toBe(320);
});
