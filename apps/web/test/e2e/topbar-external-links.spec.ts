import { expect, test } from './fixtures';

test('tutorial and feedback links open in a separate page', async ({ page }) => {
  await page.goto('/editor');

  const tutorial = page.getByTestId('topbar-tutorial-link');
  const feedback = page.getByTestId('topbar-feedback-link');

  await expect(tutorial).toHaveAttribute('href', '/tutorial');
  await expect(tutorial).toHaveAttribute('target', '_blank');
  await expect(tutorial).toHaveAttribute('rel', 'noopener noreferrer');
  await expect(feedback).toHaveAttribute('href', 'https://github.com/loggerhead/treease/issues/new');
  await expect(feedback).toHaveAttribute('target', '_blank');
  await expect(feedback).toHaveAttribute('rel', 'noopener noreferrer');
});
