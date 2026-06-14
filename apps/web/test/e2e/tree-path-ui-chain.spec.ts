import { expect, test } from './fixtures';
import { readEditorState, readGraphHighlight, setEditorContent, setMonacoPositionByText, waitForEditorReady, waitForGraphRendered } from './utils';

const crossLanguageCases = [
  {
    label: 'python dict',
    language: 'python' as const,
    sourceText: "{'user': {'name': 'Ada'}}",
    searchText: "'name':",
    expectedPath: ['$', 'user', 'name'],
  },
  {
    label: 'javascript object literal',
    language: 'javascript' as const,
    sourceText: '{ user: { name: "Ada", }, }',
    searchText: 'name:',
    expectedPath: ['$', 'user', 'name'],
  },
  {
    label: 'toml inline table',
    language: 'toml' as const,
    sourceText: 'user = { name = "Ada" }\n',
    searchText: 'name = "Ada"',
    expectedPath: ['$', 'user', 'name'],
  },
];

test('tree path breadcrumb follows editor cursor and breadcrumb clicks reveal parent path', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{\n  "user": {\n    "name": "Alice",\n    "role": "admin"\n  },\n  "items": [1, 2, 3]\n}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await setMonacoPositionByText(page, 'source-editor', '"name":');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'user', 'name']));
  await expect
    .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({
      path: ['$', 'user', 'name'],
    }));

  await page.getByTestId('tree-path-crumb-1').click();
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'user']));
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath.includes('name'), { timeout: 5_000 })
    .toBe(false);
});

for (const testCase of crossLanguageCases) {
  test(`editor cursor sync highlights graph cell for ${testCase.label}`, async ({ page }) => {
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: testCase.sourceText,
      language: testCase.language,
    });
    await waitForGraphRendered(page);

    await setMonacoPositionByText(page, 'source-editor', testCase.searchText);

    await expect
      .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
      .toEqual(expect.arrayContaining(testCase.expectedPath));
    await expect
      .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
      .toEqual(expect.objectContaining({
        path: testCase.expectedPath,
      }));
  });
}
