import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import {
  evaluateTreease,
  getMonacoRenderedTokenColor,
  readEditorState,
  readGraphClickProbes,
  setEditorContent,
  setMonacoPosition,
  waitForEditorReady,
} from './utils';

const invalidJsonFixture = readFileSync(
  new URL('../../../../test/fixtures/json/adversarial__issue150__1000.0.json', import.meta.url),
  'utf8',
);

function resolvePosition(sourceText: string, marker: string): { lineNumber: number; column: number } {
  const offset = sourceText.indexOf(marker);
  if (offset < 0) {
    throw new Error(`Marker "${marker}" not found`);
  }
  const before = sourceText.slice(0, offset);
  const lines = before.split('\n');
  return {
    lineNumber: lines.length,
    column: (lines.at(-1)?.length ?? 0) + 1,
  };
}

async function readJsonBlockRuntime(page: Page) {
  return evaluateTreease(page, (treease) => {
    const state = treease.editor.getState();
    return {
      selection: state.jsonBlockSelection,
      streamState: treease.graph.getStreamState(),
    };
  });
}

async function waitForJsonBlockRender(page: Page, expectedText: string, expectedRootPaths: string[]) {
  await expect
    .poll(
      async () => {
        const [{ selection, streamState }, probes] = await Promise.all([readJsonBlockRuntime(page), readGraphClickProbes(page)]);
        return {
          selectedText: selection?.text ?? null,
          blockDocumentKey: selection?.blockDocumentKey ?? null,
          streamDocumentKey: streamState?.documentKey ?? null,
          finalSeen: streamState?.finalSeen ?? false,
          probePaths: probes.map((probe) => probe.path.join('.')),
        };
      },
      { timeout: 5_000 },
    )
    .toEqual(
      expect.objectContaining({
        selectedText: expectedText,
        blockDocumentKey: expect.any(String),
        streamDocumentKey: expect.any(String),
        finalSeen: true,
        probePaths: expect.arrayContaining(expectedRootPaths),
      }),
    );
  await expect
    .poll(
      async () => {
        const { selection, streamState } = await readJsonBlockRuntime(page);
        return selection?.blockDocumentKey === streamState?.documentKey;
      },
      { timeout: 5_000 },
    )
    .toBe(true);
}

async function waitForSyntaxError(page: Page) {
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.diagnostics, { timeout: 5_000 })
    .toEqual([
      expect.objectContaining({
        message: 'Syntax error',
      }),
    ]);
}

async function readEditorTokenColor(page: Page, tokenText: string, lineNumber: number): Promise<string | null> {
  return getMonacoRenderedTokenColor(page, 'source-editor', tokenText, lineNumber);
}

test.describe('invalid json graph diagnostics', () => {
  test('shows one syntax error without leaking raw graph status', async ({ page }) => {
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: invalidJsonFixture,
      language: 'json',
    });

    const graphModeButton = page.getByRole('button', { name: 'Graph mode', exact: true });
    if (await graphModeButton.isVisible().catch(() => false)) {
      await graphModeButton.click();
      await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toBeVisible({ timeout: 5_000 });
    }

    await waitForSyntaxError(page);

    await expect(page.getByTestId('graph-diagnostic-syntax-error')).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
    await expect(page.getByTestId('tree-path-crumb-0')).toHaveCount(0);
  });

  test('renders only the active JSONL row when the cursor enters a line block', async ({ page }) => {

    const sourceText = ['{"line":1,"skip":"first"}', '{"line":2,"nested":{"name":"Alice"}}', '{"line":3,"skip":"third"}'].join(
      '\n',
    );
    const expectedBlockText = '{"line":2,"nested":{"name":"Alice"}}';
    const position = resolvePosition(sourceText, 'Alice');

    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText,
      language: 'json',
    });
    await waitForSyntaxError(page);

    await setMonacoPosition(page, 'source-editor', position.lineNumber, position.column);
    await waitForJsonBlockRender(page, expectedBlockText, ['line', 'nested']);
    await expect.poll(() => readEditorTokenColor(page, '"line"', 2), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect.poll(() => readEditorTokenColor(page, '"line"', 3), { timeout: 5_000 }).not.toBe('rgb(163, 21, 21)');

    const { selection } = await readJsonBlockRuntime(page);
    expect(selection).toEqual(
      expect.objectContaining({
        text: expectedBlockText,
        startLineNumber: 2,
        endLineNumber: 2,
        startColumn: 1,
        endColumn: expectedBlockText.length + 1,
      }),
    );

    const probePaths = (await readGraphClickProbes(page)).map((probe) => probe.path.join('.'));
    expect(probePaths).not.toContain('skip');

    const firstBlockText = '{"line":1,"skip":"first"}';
    await setMonacoPosition(page, 'source-editor', 1, firstBlockText.length + 1);
    await waitForJsonBlockRender(page, firstBlockText, ['line', 'skip']);

    const { selection: boundarySelection } = await readJsonBlockRuntime(page);
    expect(boundarySelection).toEqual(
      expect.objectContaining({
        text: firstBlockText,
        startLineNumber: 1,
        endLineNumber: 1,
        startColumn: 1,
        endColumn: firstBlockText.length + 1,
      }),
    );

    const boundaryProbePaths = (await readGraphClickProbes(page)).map((probe) => probe.path.join('.'));
    expect(boundaryProbePaths).not.toContain('nested');
  });

  test('renders only the embedded JSON fragment when the cursor enters a log payload', async ({ page }) => {

    const expectedBlockText = '{"kind":"audit","user":{"name":"Alice"}}';
    const sourceText = `INFO request payload=${expectedBlockText} completed`;
    const position = resolvePosition(sourceText, 'Alice');
    const expectedStartColumn = sourceText.indexOf(expectedBlockText) + 1;
    const expectedEndColumn = expectedStartColumn + expectedBlockText.length;

    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText,
      language: 'json',
    });
    await waitForSyntaxError(page);

    await setMonacoPosition(page, 'source-editor', position.lineNumber, position.column);
    await waitForJsonBlockRender(page, expectedBlockText, ['kind', 'user']);

    const { selection } = await readJsonBlockRuntime(page);
    expect(selection).toEqual(
      expect.objectContaining({
        text: expectedBlockText,
        startLineNumber: 1,
        endLineNumber: 1,
        startColumn: expectedStartColumn,
        endColumn: expectedEndColumn,
      }),
    );

    const probePaths = (await readGraphClickProbes(page)).map((probe) => probe.path.join('.'));
    expect(probePaths).not.toContain('payload');
    expect(probePaths).not.toContain('completed');
  });

  test('keeps JSON block semantic token colors aligned after UTF-8 string values', async ({ page }) => {
    const expectedBlockText =
      '{"title":"运行环境：GPU需要多大的？","file":"2023-04-03.0009"}';
    const sourceText = `$diagnose ${expectedBlockText}`;
    const position = resolvePosition(sourceText, '"2023-04-03.0009"');

    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText,
      language: 'json',
    });
    await waitForSyntaxError(page);

    await setMonacoPosition(page, 'source-editor', position.lineNumber, position.column);
    await waitForJsonBlockRender(page, expectedBlockText, ['title', 'file']);

    await expect.poll(() => readEditorTokenColor(page, '"title"', 1), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect.poll(() => readEditorTokenColor(page, '"file"', 1), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect
      .poll(() => readEditorTokenColor(page, '"2023-04-03.0009"', 1), { timeout: 5_000 })
      .toBe('rgb(4, 81, 165)');
    await expect.poll(() => readEditorTokenColor(page, '0009', 1), { timeout: 5_000 }).toBe('rgb(4, 81, 165)');
  });
});
