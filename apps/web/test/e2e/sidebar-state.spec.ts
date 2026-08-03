import { expect, test } from './fixtures';

test('sidebar expansion only changes through its toggle', async ({ page }) => {
  await page.goto('/editor');

  const sidebar = page.locator('.editor-sidebar');
  const sidebarHost = page.locator('.editor-sidebar-host');
  const importButton = page.getByRole('button', { name: 'Import', exact: true });
  await expect(page.getByTestId('sidebar-collapse-toggle')).toBeVisible();
  await expect(sidebarHost).toHaveAttribute('data-expanded', 'true');
  await expect(importButton).toHaveCSS('width', '174px');

  await sidebar.hover();
  await expect(sidebarHost).toHaveAttribute('data-expanded', 'true');

  await page.getByTestId('sidebar-collapse-toggle').click();
  await expect(sidebarHost).toHaveAttribute('data-expanded', 'false');

  await expect(importButton).toHaveCSS('width', '32px');
  await expect(importButton).not.toHaveAttribute('title');
  await importButton.hover();
  await expect(page.getByRole('tooltip', { name: 'Import', exact: true })).toBeVisible();

  await sidebar.hover();
  await expect(sidebarHost).toHaveAttribute('data-expanded', 'false');

  await page.getByTestId('sidebar-collapse-toggle').click();
  await expect(sidebarHost).toHaveAttribute('data-expanded', 'true');
});
