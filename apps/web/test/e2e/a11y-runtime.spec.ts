import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { waitForEditorReady } from './utils';

test('editor import controls are keyboard-operable', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page, 25_000);

  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'best-practice'])
    // These composite controls intentionally contain native interactive descendants.
    .disableRules(['nested-interactive'])
    .exclude('[data-sonner-toaster]')
    .analyze();
  expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);

  const importButton = page.getByTestId('topbar-import-button');
  const exportButton = page.getByTestId('topbar-export-button');
  await expect(importButton).toBeVisible();
  await importButton.focus();
  await expect(importButton).toBeFocused();

  await page.keyboard.press('Tab');
  await expect(exportButton).toBeFocused();

  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('import-panel')).toBeVisible();
  await expect(page.getByTestId('import-drop-trigger')).toHaveAttribute('aria-label', 'Choose import file');
});
