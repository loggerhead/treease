import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import {
  countMonacoElements,
  evaluateTreease,
  getMonacoMarkers,
  getMonacoValue,
  getMonacoRenderedTokenColorAtPosition,
  readEditorState,
  readGraphClickProbes,
  setEditorContent,
  setMonacoPosition,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const invalidJsonFixture = readFileSync(
  new URL('../../../../test/fixtures/json/adversarial__issue150__1000.0.json', import.meta.url),
  'utf8',
);
const promptDiffEventsFixture = readFileSync(
  new URL('../../../../test/fixtures/json/prompt_diff_events.1.json', import.meta.url),
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

async function readGraphEdgePaths(page: Page): Promise<Array<{ from: string; to: string }>> {
  return evaluateTreease(page, (treease) => {
    const graph = treease.graph.getLastGraphData?.() as unknown as {
      edges?: Array<{
        from?: { path?: Array<{ key?: string; index?: number }> };
        to?: { path?: Array<{ key?: string; index?: number }> };
      }>;
    } | null;
    const formatPath = (path: Array<{ key?: string; index?: number }> = []) => path
      .map((segment) => segment.key || `[${segment.index}]`)
      .join('.');
    return (graph?.edges ?? []).map((edge) => ({
      from: formatPath(edge.from?.path),
      to: formatPath(edge.to?.path),
    }));
  });
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

function resolveNthPosition(sourceText: string, marker: string, occurrence = 1): { lineNumber: number; column: number } {
  let offset = -1;
  let searchFrom = 0;
  for (let index = 0; index < occurrence; index += 1) {
    offset = sourceText.indexOf(marker, searchFrom);
    if (offset < 0) {
      throw new Error(`Marker "${marker}" occurrence ${occurrence} not found`);
    }
    searchFrom = offset + marker.length;
  }
  const before = sourceText.slice(0, offset);
  const lines = before.split('\n');
  return {
    lineNumber: lines.length,
    column: (lines.at(-1)?.length ?? 0) + 1,
  };
}

async function readEditorTokenColorAtMarker(
  page: Page,
  sourceText: string,
  marker: string,
  occurrence = 1,
): Promise<string | null> {
  const position = resolveNthPosition(sourceText, marker, occurrence);
  return getMonacoRenderedTokenColorAtPosition(page, 'source-editor', position.lineNumber, position.column, marker);
}

test.describe('invalid json graph diagnostics', () => {
  test('recovers whole-document JSON after deleting and restoring the closing brace', async ({ page }) => {
    test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
    const sourceText = promptDiffEventsFixture;
    const lastBraceOffset = sourceText.lastIndexOf('}');
    const beforeLastBrace = sourceText.slice(0, lastBraceOffset);
    const lastBraceLineNumber = beforeLastBrace.split('\n').length;
    const lastBraceColumn = beforeLastBrace.length - beforeLastBrace.lastIndexOf('\n');

    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText,
      language: 'json',
    });
    await waitForGraphRendered(page);

    await expect
      .poll(async () => (await readGraphClickProbes(page)).map((probe) => probe.path.join('.')), { timeout: 5_000 })
      .toEqual(expect.arrayContaining(['[0].type', '[1].type']));

    await setMonacoPosition(page, 'source-editor', lastBraceLineNumber, lastBraceColumn + 1);
    await page.keyboard.press('Backspace');

    await waitForSyntaxError(page);
    await expect
      .poll(async () => (await getMonacoMarkers(page, 'source-editor')).map((marker) => marker.message), {
        timeout: 5_000,
      })
      .toContain('Syntax error');
    await expect.poll(async () => (await readJsonBlockRuntime(page)).selection, { timeout: 5_000 }).toBeNull();
    await expect.poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 }).toBe(0);
    await setMonacoPosition(page, 'source-editor', 1, sourceText.indexOf('"prompt_key"') + 2);
    await expect.poll(async () => (await readJsonBlockRuntime(page)).selection, { timeout: 5_000 }).toBeNull();
    await expect
      .poll(async () => (await readJsonBlockRuntime(page)).streamState?.mode ?? null, { timeout: 5_000 })
      .not.toBe('json-block');

    await setMonacoPosition(page, 'source-editor', lastBraceLineNumber, lastBraceColumn);
    await page.keyboard.type('}');

    await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe(sourceText);
    await expect.poll(async () => (await readEditorState(page)).tempModel.diagnostics, { timeout: 5_000 }).toEqual([]);
    await expect.poll(async () => getMonacoMarkers(page, 'source-editor'), { timeout: 5_000 }).toEqual([]);
    await waitForGraphRendered(page);
    await expect.poll(async () => (await readJsonBlockRuntime(page)).selection, { timeout: 5_000 }).toBeNull();
    await expect
      .poll(async () => countMonacoElements(page, 'source-editor', '.treease-json-block-highlight'), {
        timeout: 5_000,
      })
      .toBe(0);
    await setMonacoPosition(page, 'source-editor', 1, sourceText.indexOf('"prompt_key"') + 2);
    await expect.poll(async () => (await readJsonBlockRuntime(page)).selection, { timeout: 5_000 }).toBeNull();
    await expect
      .poll(async () => countMonacoElements(page, 'source-editor', '.treease-json-block-highlight'), {
        timeout: 5_000,
      })
      .toBe(0);
    await expect
      .poll(async () => (await readGraphClickProbes(page)).map((probe) => probe.path.join('.')), { timeout: 5_000 })
      .toEqual(expect.arrayContaining(['[0].type', '[1].type']));
  });

  test('loads prompt diff events fixture with graph and semantic tokens intact', async ({ page }, testInfo) => {
    testInfo.annotations.push({ type: 'allow-browser-error', description: '[Cloudflare Turnstile]' });
    testInfo.annotations.push({ type: 'allow-browser-error', description: 'challenges.cloudflare.com/cdn-cgi' });
    await page.goto('/editor');
    await waitForEditorReady(page);
    await setEditorContent(page, {
      sourceText: promptDiffEventsFixture,
      language: 'json',
    });

    await waitForGraphRendered(page);
    await expect.poll(async () => (await readEditorState(page)).tempModel.diagnostics, { timeout: 5_000 }).toEqual([]);
    await expect.poll(async () => getMonacoMarkers(page, 'source-editor'), { timeout: 5_000 }).toEqual([]);
    await expect
      .poll(
        async () => ({
          keyColor: await readEditorTokenColorAtMarker(page, promptDiffEventsFixture, '"type"'),
          stringColor: await readEditorTokenColorAtMarker(page, promptDiffEventsFixture, '"Mcrguz"'),
        }),
        { timeout: 5_000 },
      )
      .toEqual({
        keyColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
        stringColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
      });
    await expect
      .poll(async () => (await readGraphClickProbes(page)).map((probe) => probe.path.join('.')), { timeout: 5_000 })
      .toEqual(expect.arrayContaining(['[0].type', '[1].type']));
    await expect
      .poll(() => readGraphEdgePaths(page), { timeout: 5_000 })
      .toEqual(expect.arrayContaining([{ from: '[1]', to: '[1].value' }]));
    expect(await readGraphEdgePaths(page)).not.toEqual(expect.arrayContaining([{ from: '', to: '[1].value' }]));
  });

  test('shows one syntax error with a recoverable graph failure state', async ({ page }) => {
    test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: invalidJsonFixture,
      language: 'json',
    });

    await page.getByTestId('graph-surface-graph').click();

    await waitForSyntaxError(page);

    await expect(page.getByTestId('graph-diagnostic-syntax-error')).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
    await expect(page.getByTestId('tree-path-crumb-0')).toHaveCount(1);
  });

  test('renders only the active JSONL row when the cursor enters a line block', async ({ page }) => {
    test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });

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
    await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, '"line"', 2), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, '"line"', 3), { timeout: 5_000 }).not.toBe('rgb(163, 21, 21)');

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

    // A cursor immediately after a container belongs to neither adjacent JSON block.
    // This is the boundary contract from Core's findJsonBlockAtPosition.
    await setMonacoPosition(page, 'source-editor', 1, '{"line":1,"skip":"first"}'.length + 1);
    await expect.poll(async () => (await readJsonBlockRuntime(page)).selection, { timeout: 5_000 }).toBeNull();
  });

  test('renders only the embedded JSON fragment when the cursor enters a log payload', async ({ page }) => {
    test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });

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
    test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
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

    await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, '"title"'), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, '"file"'), { timeout: 5_000 }).toBe('rgb(163, 21, 21)');
    await expect
      .poll(() => readEditorTokenColorAtMarker(page, sourceText, '"2023-04-03.0009"'), { timeout: 5_000 })
      .toBe('rgb(4, 81, 165)');
    await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, '0009'), { timeout: 5_000 }).toBe('rgb(4, 81, 165)');
  });
});
