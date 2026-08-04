import { expect, test } from './fixtures';
import {
  getMonacoRenderedTokenColor,
  getMonacoValue,
  openCommandSearch,
  readEditorState,
  setEditorContent,
  setMonacoPosition,
  setMonacoValue,
  waitForEditorReady,
  waitForImportSettled,
  waitForMonacoHook,
  waitForSettingsReady,
} from './utils';

async function resetSettingsStore(page: import('@playwright/test').Page) {
  await page.evaluate(
    async () =>
      await new Promise<void>((resolve, reject) => {
        const closeAndDelete = () => {
          const request = indexedDB.deleteDatabase('treease-settings');
          request.onerror = () => reject(request.error ?? new Error('indexedDB delete failed'));
          request.onsuccess = () => resolve();
          request.onblocked = () => resolve();
        };

        const open = indexedDB.open('treease-settings', 1);
        open.onerror = () => closeAndDelete();
        open.onsuccess = () => {
          open.result.close();
          closeAndDelete();
        };
        open.onupgradeneeded = () => {
          open.result.close();
          closeAndDelete();
        };
      }),
  );

  await page.reload();
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
}

async function setNestSetting(page: import('@playwright/test').Page, enableNest: boolean) {
  await page.evaluate(
    async ({ enableNest }) => {
      const treease = window._treease;
      if (!treease) throw new Error('window._treease is unavailable');
      const current = treease.settings.getState().settings;
      await treease.settings.save({
        ...current,
        parser: {
          ...current.parser,
          enableNest,
        },
      });
    },
    { enableNest },
  );
  await waitForSettingsReady(page);
}

test('runs format and minify via UI controls', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);

  await setEditorContent(page, {
    sourceText: '{"z":3,"a":1,"m":2}',
  });
  await waitForEditorReady(page);
  await setMonacoPosition(page, 'source-editor', 1, 5);

  await page.getByRole('button', { name: 'Format', exact: true }).click();
  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('\n');
  await expect.poll(async () => (await readEditorState(page)).tempModel.cursor, { timeout: 5_000 }).toBe('Ln 1, Col 1');

  await setMonacoPosition(page, 'source-editor', 2, 3);

  await page.getByRole('button', { name: 'Minify', exact: true }).click();
  await expect
    .poll(async () => (await readEditorState(page)).sourceText.trim(), { timeout: 5_000 })
    .toBe('{"z":3,"a":1,"m":2}');
  await expect.poll(async () => (await readEditorState(page)).tempModel.cursor, { timeout: 5_000 }).toBe('Ln 1, Col 1');
});

test('auto formats whole-document replacements when smart formatting is enabled', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);
  await waitForMonacoHook(page, 'source-editor');
  await waitForImportSettled(page, 5_000);

  await setMonacoValue(
    page,
    'source-editor',
    '{"title":"运行环境：GPU需要多大的？","file":"2023-04-03.0009"}',
  );

  await expect
    .poll(async () => await getMonacoValue(page, 'source-editor'), { timeout: 5_000 })
    .toBe('{"title": "运行环境：GPU需要多大的？", "file": "2023-04-03.0009"}\n');
});

test('runs sort from command search and updates editor text', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);

  await setEditorContent(page, {
    sourceText: '{"object":{"int":42},"table_without_header":["a","b"],"table_with_header":[{"h1":11}],"preview":{"color":"#4f46e5"}}',
  });
  await setMonacoPosition(page, 'source-editor', 1, 7);

  const commandInput = await openCommandSearch(page);
  await commandInput.fill('sort');
  await commandInput.press('Enter');
  await commandInput.press('Enter');

  await expect
    .poll(async () => Object.keys(JSON.parse((await readEditorState(page)).sourceText)).join(','), { timeout: 5_000 })
    .toBe('object,preview,table_with_header,table_without_header');
  await expect.poll(async () => (await readEditorState(page)).tempModel.cursor, { timeout: 5_000 }).toBe('Ln 1, Col 1');
});

test('runs yq preview from command search without overwriting source editor', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);
  await waitForEditorReady(page);

  const sourceText = '{"items":[{"name":"Alice"},{"name":"Bob"}]}'
  await setEditorContent(page, {
    sourceText,
  });

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('items');

  const commandInput = await openCommandSearch(page);
  await commandInput.fill('yq');
  await commandInput.press('Enter');

  await expect(page.getByTestId('yq-expression-panel')).toBeVisible({ timeout: 5_000 });
  await waitForMonacoHook(page, 'yq-input-box');

  await page.keyboard.type('.items[0]');
  await page.getByRole('button', { name: 'Run', exact: true }).click();

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.scratchText, { timeout: 5_000 })
    .toContain('Alice');

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('items');

  await expect(page.getByTestId('graph-surface-graph')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId('yq-expression-error')).toHaveCount(0);
});

test('format keeps TOML literal dotted key', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);

  await setEditorContent(page, {
    language: 'toml',
    sourceText: '[meta]\nid = "item-001"\n["meta.profile"]\nname = "Alice"\n',
  });

  await page.getByRole('button', { name: 'Format', exact: true }).click();

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('["meta.profile"]');

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('name = "Alice"');
});

test('format uses parser.enableNest setting for nested JSON strings', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);

  const nestedJsonText = '{"a":"{\\"b\\":1}"}';

  await setNestSetting(page, false);
  await setEditorContent(page, {
    language: 'json',
    sourceText: nestedJsonText,
  });
  await page.getByRole('button', { name: 'Format', exact: true }).click();

  await expect
    .poll(async () => {
      const text = (await readEditorState(page)).sourceText;
      return JSON.parse(text) as { a: unknown };
    }, { timeout: 5_000 })
    .toEqual({ a: '{"b":1}' });

  await setEditorContent(page, {
    language: 'json',
    sourceText: nestedJsonText,
  });
  await setNestSetting(page, true);
  await page.getByRole('button', { name: 'Format', exact: true }).click();

  await expect
    .poll(async () => {
      const text = (await readEditorState(page)).sourceText;
      const value = JSON.parse(text) as { a: unknown };
      if (typeof value.a === 'string') {
        return JSON.stringify(JSON.parse(value.a)) === JSON.stringify({ b: 1 });
      }
      return JSON.stringify(value.a) === JSON.stringify({ b: 1 });
    }, { timeout: 5_000 })
    .toBe(true);
});

test('whole-document replacement writes back nest-expanded source text', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);
  await setNestSetting(page, true);

  await setMonacoValue(page, 'source-editor', '"{\\"a\\":1}"');

  await expect
    .poll(async () => {
      const text = await getMonacoValue(page, 'source-editor');
      const parsed = JSON.parse(text);
      return {
        modelText: text,
        storeText: (await readEditorState(page)).sourceText,
        normalizedModelText: text.trimEnd(),
        normalizedStoreText: (await readEditorState(page)).sourceText.trimEnd(),
        parsedType: typeof parsed,
        parsedValue: parsed,
        keyColor: await getMonacoRenderedTokenColor(page, 'source-editor', 'a', 1),
        numberColor: await getMonacoRenderedTokenColor(page, 'source-editor', '1', 1),
      };
    }, { timeout: 5_000 })
    .toEqual({
      modelText: '{"a": 1}\n',
      storeText: '{"a": 1}\n',
      normalizedModelText: '{"a": 1}',
      normalizedStoreText: '{"a": 1}',
      parsedType: 'object',
      parsedValue: { a: 1 },
      keyColor: 'rgb(163, 21, 21)',
      numberColor: 'rgb(9, 134, 88)',
    });
});

test('escape preserves submitted source text when nest parse is enabled', async ({ page }) => {
  await page.goto('/editor');
  await resetSettingsStore(page);
  await setNestSetting(page, true);

  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"a":1}',
  });

  const commandInput = await openCommandSearch(page);
  await commandInput.fill('escape');
  await page.getByText('Escape', { exact: true }).click();

  await expect
    .poll(async () => {
      const text = await getMonacoValue(page, 'source-editor');
      const parsed = JSON.parse(text);
      return {
        modelText: text,
        storeText: (await readEditorState(page)).sourceText,
        parsedType: typeof parsed,
        parsedValue: parsed,
      };
    }, { timeout: 5_000 })
    .toEqual({
      modelText: '"{\\"a\\":1}"',
      storeText: '"{\\"a\\":1}"',
      parsedType: 'string',
      parsedValue: '{"a":1}',
    });
});
