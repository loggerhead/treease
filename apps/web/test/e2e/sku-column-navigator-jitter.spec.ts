import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test, type Page } from './fixtures';
import {
  clickGraphProbeAt,
  readGraphClickProbes,
  setEditorContent,
  waitForColumnNavigatorSettled,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const SKU_CONFIG_FIXTURE = resolve(process.cwd(), '../../test/fixtures/json/sku_config.1.json');
const EDITOR_URL = process.env.TREEASE_E2E_BASE_URL ?? '/editor';

type ViewportSample = {
  x: number;
  y: number;
  scaleX: number;
  scaleY: number;
};

async function startViewportSampling(page: Page): Promise<void> {
  await page.evaluate(() => {
    const recorder = { active: true, samples: [] as ViewportSample[] };
    Object.assign(window, { __treeaseViewportRecorder: recorder });
    const sample = () => {
      const graph = window._treease?.graph as unknown as {
        refs?: { leafer?: { zoomLayer?: Partial<ViewportSample> } | null };
      };
      const layer = graph.refs?.leafer?.zoomLayer;
      if (layer) {
        recorder.samples.push({
          x: Number(layer.x ?? 0),
          y: Number(layer.y ?? 0),
          scaleX: Number(layer.scaleX ?? 1),
          scaleY: Number(layer.scaleY ?? 1),
        });
      }
      if (recorder.active) requestAnimationFrame(sample);
    };
    sample();
  });
}

async function stopViewportSampling(page: Page): Promise<ViewportSample[]> {
  return page.evaluate(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const recorder = (window as unknown as { __treeaseViewportRecorder?: { active: boolean; samples: ViewportSample[] } })
      .__treeaseViewportRecorder;
    if (!recorder) return [];
    recorder.active = false;
    return recorder.samples;
  });
}

function greatestViewportFrameStep(samples: ViewportSample[]): number {
  return Math.max(
    0,
    ...samples.slice(1).map((sample, index) => {
      const previous = samples[index]!;
      return Math.hypot(sample.x - previous.x, sample.y - previous.y);
    }),
  );
}

test('moves the root graph smoothly during rapid SKU Price navigation', async ({ page }, testInfo) => {
  test.setTimeout(60_000);
  testInfo.annotations.push({ type: 'allow-browser-error', description: 'localhost:3000/v1/usage/events' });
  await page.goto(EDITOR_URL);
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: readFileSync(SKU_CONFIG_FIXTURE, 'utf8'),
    language: 'json',
  });
  await waitForGraphRendered(page, 30_000);

  const firstRootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'AE' && probe.coord,
  );
  expect(firstRootProbe, 'first root graph cell').toBeTruthy();
  if (!firstRootProbe?.coord) throw new Error('first root graph cell has no coordinate');

  await clickGraphProbeAt(page, firstRootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:AE', 20_000);

  await page.locator('[data-column-navigator-item-path-key="k:QA"]').click();
  await waitForColumnNavigatorSettled(page, 'k:QA', 20_000);

  await page.locator('[data-column-navigator-item-path-key="k:QA|k:Prices"]').click();
  await waitForColumnNavigatorSettled(page, 'k:QA|k:Prices', 20_000);

  await page.locator('[data-column-navigator-item-path-key="k:QA|k:Prices|i:0"]').click();
  await waitForColumnNavigatorSettled(page, 'k:QA|k:Prices|i:0', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  await workspace.focus();
  await startViewportSampling(page);
  await workspace.evaluate((element) => {
    for (let index = 0; index < 30; index += 1) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    }
  });
  await waitForColumnNavigatorSettled(page, undefined, 20_000);
  const samples = await stopViewportSampling(page);
  await testInfo.attach('root-graph-viewport-samples', {
    body: JSON.stringify(samples),
    contentType: 'application/json',
  });

  expect(samples, 'root graph viewport samples').not.toHaveLength(0);
  expect(greatestViewportFrameStep(samples), 'root graph viewport must not jump between animation frames').toBeLessThanOrEqual(80);
});
