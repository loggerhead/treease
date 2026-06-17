import { expect, test } from './fixtures';
import { readGraphClickProbes, waitForEditorReady, waitForGraphRendered } from './utils';

test('editor and graph loading skeletons clear together on first paint', async ({ page }, testInfo) => {
  testInfo.annotations.push({
    type: 'allow-browser-error',
    description: 'Failed to load resource: the server responded with a status of 404 (Not Found)',
  });

  await page.addInitScript(() => {
    window.__treeaseEditorRuntimeStartupDelayMs = 400;

    const observation = {
      sawGraphHiddenWhileEditorVisible: false,
      sawEditorHiddenWhileGraphVisible: false,
      samples: [] as Array<{ editorVisible: boolean; graphVisible: boolean }>,
    };

    const readVisibleState = () => ({
      editorVisible: Boolean(document.querySelector('[aria-label="Editor loading status"]')),
      graphVisible: Boolean(document.querySelector('[aria-label="Graph loading status"]')),
    });

    const sample = () => {
      const current = readVisibleState();
      const last = observation.samples.at(-1);
      if (!last || last.editorVisible !== current.editorVisible || last.graphVisible !== current.graphVisible) {
        observation.samples.push(current);
      }
      if (current.editorVisible && !current.graphVisible) {
        observation.sawGraphHiddenWhileEditorVisible = true;
      }
      if (!current.editorVisible && current.graphVisible) {
        observation.sawEditorHiddenWhileGraphVisible = true;
      }
    };

    const start = () => {
      sample();
      const observer = new MutationObserver(() => sample());
      observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['class', 'style', 'aria-hidden'],
      });

      const frameSample = () => {
        sample();
        const current = readVisibleState();
        if (current.editorVisible || current.graphVisible) {
          requestAnimationFrame(frameSample);
          return;
        }
        observer.disconnect();
      };

      requestAnimationFrame(frameSample);
      (window as typeof window & { __treeaseLoadingSyncObservation?: typeof observation }).__treeaseLoadingSyncObservation =
        observation;
    };

    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', start, { once: true });
      return;
    }

    start();
  });

  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForGraphRendered(page);

  const observation = await page.evaluate(() => {
    return (window as typeof window & {
      __treeaseLoadingSyncObservation?: {
        sawGraphHiddenWhileEditorVisible: boolean;
        sawEditorHiddenWhileGraphVisible: boolean;
        samples: Array<{ editorVisible: boolean; graphVisible: boolean }>;
      };
    }).__treeaseLoadingSyncObservation ?? null;
  });

  expect(observation).not.toBeNull();
  expect((observation?.samples.length ?? 0) > 0).toBe(true);
  expect(observation?.sawGraphHiddenWhileEditorVisible).toBe(false);
  expect(observation?.sawEditorHiddenWhileGraphVisible).toBe(false);
  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
  await expect
    .poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);
});
