import { expect, test } from './fixtures';
import { waitForEditorReady, waitForSettingsReady } from './utils';

test('settings reset reproduces the Monaco errors reported by Sentry', async ({ page }, testInfo) => {
  // This is an executable reproduction of TREEASE-WEB-Y/Z/10. The fixture
  // normally fails on any browser error, so allow the known errors while the
  // test asserts that each one is actually observed.
  testInfo.annotations.push(
    { type: 'allow-browser-error', description: 'UNKNOWN service ICodeLensCache' },
    { type: 'allow-browser-error', description: 'UNKNOWN service treeViewsDndService' },
    { type: 'allow-browser-error', description: 'Canceled' },
  );

  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));

  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);

  await page.getByRole('button', { name: 'Settings', exact: true }).click();

  const settingsDialog = page.getByTestId('settings-dialog');
  await expect(settingsDialog).toBeVisible();
  await expect(page.getByTestId('monaco-settings-editor')).toBeVisible();

  await expect
    .poll(() => pageErrors.some((error) => error.includes('UNKNOWN service ICodeLensCache')), { timeout: 5_000 })
    .toBe(true);
  await expect
    .poll(() => pageErrors.some((error) => error.includes('UNKNOWN service treeViewsDndService')), { timeout: 5_000 })
    .toBe(true);

  await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
  await expect(settingsDialog).toHaveCount(0);
  await expect.poll(() => pageErrors.some((error) => error === 'Canceled'), { timeout: 5_000 }).toBe(true);
});
