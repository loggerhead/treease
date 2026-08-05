import { expect, test, type Page } from './fixtures';
import {
  clickGraphProbeAt,
  expectMonacoHoverContains,
  getMonacoValue,
  openCommandSearch,
  openMonacoHover,
  readEditorState,
  readGraphClickProbes,
  setMonacoValue,
  waitForColumnNavigatorSettled,
  waitForEditorReady,
  waitForGraphRendered,
  waitForMonacoHook,
} from './utils';

const LARGE_JSON_FIXTURE = '../../test/fixtures/json/2mb.1.json';
const UPDATED_EXAMPLE_VALUE = 43;
const YQ_EXPRESSION = '.Result.Blocks[9].Content.AllCompositionType[1].Key';
const YQ_RESULT = 'alkebhj_erfm_zfaej';

async function findGraphValueProbe(page: Page, path: string, text: string) {
  let probe: Awaited<ReturnType<typeof readGraphClickProbes>>[number] | undefined;
  await expect
    .poll(
      async () => {
        try {
          probe = (await readGraphClickProbes(page)).find(
            (candidate) =>
              candidate.target === 'value' &&
              candidate.path.join('.') === path &&
              candidate.text === text &&
              candidate.coord,
          );
          return Boolean(probe);
        } catch {
          return false;
        }
      },
      { timeout: 5_000 },
    )
    .toBe(true);
  if (!probe?.coord) throw new Error(`Graph value probe missing for ${path}=${text}`);
  return probe;
}

async function waitForPersistedExampleEdit(page: Page) {
  await expect
    .poll(
      async () => {
        try {
          return await page.evaluate(async (expectedValue) => {
            const database = await new Promise<IDBDatabase>((resolve, reject) => {
              const request = indexedDB.open('treease-workspace', 1);
              request.onsuccess = () => resolve(request.result);
              request.onerror = () => reject(request.error);
            });
            const stored = await new Promise<{ session?: { tabs?: Array<{ sourceText?: string }> } } | undefined>(
              (resolve, reject) => {
                const transaction = database.transaction('sessions', 'readonly');
                const request = transaction.objectStore('sessions').get('current');
                request.onsuccess = () => resolve(request.result);
                request.onerror = () => reject(request.error);
              },
            );
            database.close();
            return stored?.session?.tabs?.some(({ sourceText }) => {
              if (!sourceText) return false;
              try {
                return JSON.parse(sourceText).object?.int === expectedValue;
              } catch {
                return false;
              }
            });
          }, UPDATED_EXAMPLE_VALUE);
        } catch {
          return false;
        }
      },
      { timeout: 5_000 },
    )
    .toBe(true);
}

async function installGraphLoadingObservation(page: Page) {
  await page.evaluate(() => {
    const runtimeWindow = window as Window & { __coreWorkflowSawGraphLoading?: boolean };
    let stopped = false;
    const sample = () => {
      if (document.querySelector('[aria-label="Graph loading status"]')) {
        runtimeWindow.__coreWorkflowSawGraphLoading = true;
      }
      if (!stopped && !runtimeWindow.__coreWorkflowSawGraphLoading) requestAnimationFrame(sample);
    };
    runtimeWindow.__coreWorkflowSawGraphLoading = false;
    const observer = new MutationObserver(() => {
      sample();
    });
    observer.observe(document.documentElement, { attributes: true, childList: true, subtree: true });
    requestAnimationFrame(sample);
    window.addEventListener(
      'pagehide',
      () => {
        stopped = true;
        observer.disconnect();
      },
      { once: true },
    );
  });
}

test('completes the core editor workflow from example through large-file yq preview', async ({ page }) => {
  test.setTimeout(150_000);
  const workflowFailures: string[] = [];

  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForGraphRendered(page);

  const initialSourceText = (await readEditorState(page)).sourceText;
  const initialDocument = JSON.parse(initialSourceText);
  expect(initialDocument.object.int).toBe(42);
  expect(initialDocument.preview.color).toBe('#4f46e5');
  await findGraphValueProbe(page, 'object.int', '42');
  await findGraphValueProbe(page, 'preview.color', '#4f46e5');

  const colorLineIndex = initialSourceText.split('\n').findIndex((line) => line.includes('#4f46e5'));
  expect(colorLineIndex).toBeGreaterThanOrEqual(0);
  await openMonacoHover(page, {
    hookId: 'source-editor',
    lineNumber: colorLineIndex + 1,
    column: 16,
    hoverText: '#4f46e5',
  });
  await expectMonacoHoverContains(page, ['HEX', '#4f46e5', 'rgb(79, 70, 229)']);

  const intProbe = await findGraphValueProbe(page, 'object.int', '42');
  await clickGraphProbeAt(page, intProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:object|k:int');

  const navigator = page.getByTestId('column-navigator-graph');
  await expect(navigator).toBeVisible();
  await expect(page.getByTestId('tree-path-bar').getByTestId('tree-path-crumb-1')).toHaveText('object');
  await expect(page.getByTestId('tree-path-bar').getByTestId('tree-path-crumb-2')).toHaveText('int');
  await expect(navigator.locator('[data-column-navigator-selected="true"]')).toHaveAttribute(
    'data-column-navigator-item-path-key',
    'k:object|k:int',
  );

  const detailHookId = 'column-navigator-content:k:object|k:int';
  await expect(navigator.getByTestId('column-navigator-monaco-editor')).toBeVisible();
  await expect.poll(() => getMonacoValue(page, detailHookId), { timeout: 5_000 }).toBe('42');
  await setMonacoValue(page, detailHookId, String(UPDATED_EXAMPLE_VALUE));

  await expect
    .poll(async () => JSON.parse((await readEditorState(page)).sourceText).object.int, { timeout: 5_000 })
    .toBe(UPDATED_EXAMPLE_VALUE);
  await findGraphValueProbe(page, 'object.int', String(UPDATED_EXAMPLE_VALUE));

  await waitForPersistedExampleEdit(page);
  await page.reload();
  await waitForEditorReady(page);
  await waitForGraphRendered(page);
  await expect
    .poll(async () => JSON.parse((await readEditorState(page)).sourceText).object.int, { timeout: 5_000 })
    .toBe(UPDATED_EXAMPLE_VALUE);
  await findGraphValueProbe(page, 'object.int', String(UPDATED_EXAMPLE_VALUE));

  await page.getByTestId('new-tab-button').click();
  await expect.poll(() => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('');
  await expect(page.getByRole('button', { name: 'Choose a file or drag one into this editor' })).toBeVisible();
  await expect.poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 }).toBe(0);

  await installGraphLoadingObservation(page);
  const fileChooserPromise = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Choose a file or drag one into this editor' }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(LARGE_JSON_FIXTURE);

  await Promise.all([
    expect
      .poll(
        () =>
          page.evaluate(
            () => (window as Window & { __coreWorkflowSawGraphLoading?: boolean }).__coreWorkflowSawGraphLoading,
          ),
        { timeout: 5_000 },
      )
      .toBe(true)
      .catch(() => {
        workflowFailures.push('Graph loading status did not appear after importing 2mb.1.json');
      }),
    waitForGraphRendered(page, 5_000),
  ]);
  await expect(page.getByRole('status', { name: 'Graph loading status' })).toHaveCount(0, { timeout: 5_000 });
  await expect.poll(async () => (await readEditorState(page)).sourceText.length, { timeout: 5_000 }).toBeGreaterThan(1_000_000);

  const minifyCommandInput = await openCommandSearch(page);
  await minifyCommandInput.fill('minify');
  await page.getByRole('option', { name: 'Minify', exact: true }).click();
  try {
    await expect
      .poll(
        async () => {
          const sourceText = (await readEditorState(page)).sourceText;
          return {
            hasLineBreak: /[\r\n]/.test(sourceText),
            parseable: (() => {
              try {
                JSON.parse(sourceText);
                return true;
              } catch {
                return false;
              }
            })(),
          };
        },
        { timeout: 60_000 },
      )
      .toEqual({ hasLineBreak: false, parseable: true });
  } catch {
    workflowFailures.push('Minify did not convert the imported JSON to one line within 60 seconds');
  }
  const minifiedSourceText = (await readEditorState(page)).sourceText;

  const yqCommandInput = await openCommandSearch(page);
  await yqCommandInput.fill('yq');
  await yqCommandInput.press('Enter');
  await expect(page.getByTestId('yq-expression-panel')).toBeVisible({ timeout: 5_000 });
  await waitForMonacoHook(page, 'yq-input-box');
  await expect
    .poll(
      () =>
        page.getByTestId('monaco-yq-input-box').evaluate((container) => container.contains(document.activeElement)),
      { timeout: 5_000 },
    )
    .toBe(true);

  await page.keyboard.insertText(YQ_EXPRESSION);
  await page.getByRole('button', { name: 'Run', exact: true }).click();

  await expect(page.getByTestId('graph-surface-compare')).toHaveAttribute('aria-selected', 'true', {
    timeout: 5_000,
  });
  await expect.poll(async () => (await getMonacoValue(page, 'right-editor')).trim(), { timeout: 5_000 }).toBe(YQ_RESULT);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(minifiedSourceText);
  expect(workflowFailures).toEqual([]);
});
