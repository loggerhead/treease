import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

const publicPages = ['/', '/about', '/tutorial', '/changelog', '/terms', '/privacy'];

test.describe('public accessibility contract', () => {
  for (const path of publicPages) {
    test(`${path} passes automated accessibility checks`, async ({ page }) => {
      await page.goto(path);

      const results = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'best-practice'])
        .analyze();
      expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
    });
  }

  test('keyboard navigation reaches the main content', async ({ page }) => {
    await page.goto('/');

    await page.locator('body').focus();
    await page.keyboard.press('Tab');
    await expect(page.locator('.skip-link')).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(page.locator('#main-content')).toBeFocused();
  });

  test('mobile pages keep core content within the viewport', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/');

    await expect(page.locator('h1')).toBeVisible();
    await expect(page.getByRole('link', { name: 'Open Editor' }).first()).toBeVisible();
    await expect
      .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
      .toBe(true);

    await page.getByRole('heading', { name: 'FAQ' }).scrollIntoViewIfNeeded();
    const faq = page.locator('details').first();
    await faq.locator('summary').click();
    await expect(faq).toHaveAttribute('open', '');
  });

  test('Changelog year tabs expose a keyboard-operable tab panel', async ({ page }) => {
    await page.goto('/changelog');

    const selectedTab = page.getByRole('tab', { selected: true });
    await expect(selectedTab).toHaveAttribute('aria-controls', 'changelog-year-panel');
    await expect(page.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', await selectedTab.getAttribute('id'));

    await selectedTab.focus();
    await page.keyboard.press('Tab');
    await expect(page.getByRole('tabpanel')).toBeFocused();
  });

  test('public content remains available with JavaScript disabled', async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto('/tutorial');
    await expect(page.locator('h1')).toContainText('JSON viewer, formatter, and compare tutorials');
    await expect(page.locator('a[href="/tutorial/json-viewer"]').first()).toBeVisible();
    await context.close();
  });

  test('reduced-motion preference removes non-essential animation timing', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/changelog');

    const activeMotion = await page.locator('*').evaluateAll((elements) =>
      elements
        .map((element) => {
          const style = getComputedStyle(element);
          return {
            animationDuration: Number.parseFloat(style.animationDuration),
            transitionDuration: Number.parseFloat(style.transitionDuration),
          };
        })
        .filter(({ animationDuration, transitionDuration }) => animationDuration > 0.02 || transitionDuration > 0.02),
    );
    expect(activeMotion).toEqual([]);
  });
});
