import { expect, test } from './fixtures';

type DemoPlayWindow = Window & typeof globalThis & { __treeaseDemoPlayCalls: string[] };
type DemoFrameWindow = Window & typeof globalThis & {
  __treeaseDemoFrameCallbacks: VideoFrameRequestCallback[];
};

test('the demo deck autoplays on load, then stops rotating after interaction', async ({ page }) => {
  test.setTimeout(20_000);
  await page.addInitScript(() => {
    const play = HTMLMediaElement.prototype.play;
    const playCalls: string[] = [];
    Object.defineProperty(window as DemoPlayWindow, '__treeaseDemoPlayCalls', { value: playCalls });
    HTMLMediaElement.prototype.play = function() {
      playCalls.push(this.currentSrc);
      return play.call(this);
    };
  });
  await page.goto('/');
  await expect.poll(() => page.evaluate(() => Boolean(window._treease)), { timeout: 5_000 }).toBe(true);

  const graphDemo = page.getByRole('button', { name: 'Graph demo', exact: true });
  const graphCard = graphDemo.locator('xpath=ancestor::article');
  const compareDemo = page.getByRole('button', { name: 'Compare demo', exact: true });
  const compareCard = compareDemo.locator('xpath=ancestor::article');
  const compareControl = page.getByRole('button', { name: 'Show Compare demo', exact: true });
  const homeLink = page.getByRole('link', { name: 'Treease home', exact: true });

  await expect(graphDemo).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => (window as DemoPlayWindow).__treeaseDemoPlayCalls.length), { timeout: 5_000 })
    .toBe(1);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    await graphDemo.hover();
    await expect(graphCard).toHaveClass(/demo-card--active/);
    await expect
      .poll(() => page.evaluate(() => (window as DemoPlayWindow).__treeaseDemoPlayCalls.length), { timeout: 5_000 })
      .toBe(attempt + 2);
    if (attempt === 0) {
      await page.waitForTimeout(4_500);
      await expect(graphCard).toHaveClass(/demo-card--active/);
    }
    await homeLink.hover();
  }

  await compareControl.click();
  await expect(compareCard).toHaveClass(/demo-card--active/);
  await expect(compareControl).toHaveAttribute('aria-pressed', 'true');
});

test('a late video-ready event cannot reveal a demo after the pointer leaves', async ({ page }) => {
  await page.goto('/');
  await expect.poll(() => page.evaluate(() => Boolean(window._treease)), { timeout: 5_000 }).toBe(true);

  const graphDemo = page.getByRole('button', { name: 'Graph demo', exact: true });
  const graphVideo = graphDemo.locator('xpath=ancestor::article').locator('video');
  const homeLink = page.getByRole('link', { name: 'Treease home', exact: true });

  await graphDemo.hover();
  await homeLink.hover();
  await graphVideo.evaluate((video) => video.dispatchEvent(new Event('loadeddata')));
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));

  await expect(graphVideo).toHaveCSS('opacity', '0');
});

test('the video stays behind its demo image until its first frame is presented', async ({ page }) => {
  await page.addInitScript(() => {
    const callbacks: VideoFrameRequestCallback[] = [];
    Object.defineProperty(window as DemoFrameWindow, '__treeaseDemoFrameCallbacks', {
      value: callbacks
    });
    HTMLVideoElement.prototype.requestVideoFrameCallback = function(callback) {
      callbacks.push(callback);
      return callbacks.length;
    };
  });
  await page.goto('/');
  await expect.poll(() => page.evaluate(() => Boolean(window._treease)), { timeout: 5_000 }).toBe(true);

  const graphDemo = page.getByRole('button', { name: 'Graph demo', exact: true });
  const graphVideo = graphDemo.locator('xpath=ancestor::article').locator('video');

  await expect
    .poll(() => page.evaluate(() => (window as DemoFrameWindow).__treeaseDemoFrameCallbacks.length))
    .toBeGreaterThan(0);
  await expect(graphVideo).toHaveCSS('opacity', '0');

  await page.evaluate(() => {
    const callback = (window as DemoFrameWindow).__treeaseDemoFrameCallbacks.shift();
    callback?.(performance.now(), {} as VideoFrameCallbackMetadata);
  });

  await expect(graphVideo).toHaveCSS('opacity', '1');
});
