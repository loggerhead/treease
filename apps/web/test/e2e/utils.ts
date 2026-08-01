import { expect, type Page } from '@playwright/test';

import type {
  TreeaseGraphBuildResult,
  TreeaseRuntimePathSeg,
  TreeaseRuntimeReadiness,
  TreeaseTestGraphEditEvent,
  TreeaseTestGraphEditEventDetail,
  WindowTreease,
} from '../../src/lib/test-bridge/types';
import type { PathSpan } from '@core-wasm/index';

const DEFAULT_UI_TIMEOUT = 10_000;
const CURSOR_PATH_SYNC_TIMEOUT = 5_000;

type EditorSnapshot = {
  sourceText: string;
  languageId: string;
  documentKey: string;
  compareEditToken: number;
  editorRevision: number;
  graphAppliedRevision: number;
  tempModel: {
    treePath: unknown[];
    diagnostics: Array<{ message: string; startLineNumber: number; startColumn: number }>;
    cursor: string;
    selectionLength: number;
    scratchText: string;
  };
};

const CURSOR_RE = /Ln\s+(\d+),\s*Col\s+(\d+)/;

function monacoHook(page: Page, hookId: string) {
  return page.getByTestId(`monaco-${hookId}`).last();
}

export async function evaluateTreease<T, P = undefined>(
  page: Page,
  evaluator: (treease: WindowTreease, payload: P) => T | Promise<T>,
  payload?: P,
): Promise<Awaited<T>> {
  return page.evaluate(
    async (args) => {
      const treease = window._treease;
      if (!treease) {
        throw new Error('window._treease is unavailable');
      }
      return new Function('treease', 'payload', `return (${args.fnSource})(treease, payload);`)(
        treease,
        args.payload,
      ) as T;
    },
    { fnSource: evaluator.toString(), payload },
  ) as Promise<Awaited<T>>;
}

function readPathKey(value: unknown): string {
  if (typeof value !== 'string') {
    throw new Error(`Expected path key string, received ${Object.prototype.toString.call(value)}`);
  }
  return value;
}

function formatPathSegment(segment: { key?: unknown; index?: number }): string {
  const key = typeof segment?.key === 'undefined' ? '' : readPathKey(segment.key);
  if (key.length > 0) return key;
  if (typeof segment?.index === 'number') return `[${segment.index}]`;
  return '';
}

function normalizeBridgePath(path: Array<{ key?: unknown; index?: number; tag?: number }>): Array<{
  key?: string;
  index?: number;
  tag?: number;
}> {
  return (path ?? []).map((segment) => ({
    tag:
      typeof segment?.tag === 'number'
        ? segment.tag
        : typeof segment?.key === 'string'
          ? 0
          : typeof segment?.index === 'number'
            ? 1
            : undefined,
    index: segment?.index,
    key: typeof segment?.key === 'undefined' ? undefined : readPathKey(segment.key),
  }));
}

export async function waitForMonacoHook(page: Page, hookId: string, timeout = DEFAULT_UI_TIMEOUT) {
  const locator = monacoHook(page, hookId);
  await expect
    .poll(
      async () => {
        const count = await locator.count();
        if (count === 0) {
          return false;
        }
        return evaluateTreease(page, (treease, nextHookId) => treease.editor.isReady(nextHookId), hookId);
      },
      { timeout },
    )
    .toBe(true);
  return locator;
}

async function readCursorText(page: Page): Promise<string> {
  const candidates = page.locator('main').getByText(CURSOR_RE);
  const count = await candidates.count();
  if (count === 0) return '';
  return (await candidates.last().textContent())?.trim() ?? '';
}

export async function readEditorState(page: Page): Promise<EditorSnapshot> {
  const state = await evaluateTreease(page, (treease) => treease.editor.getState());
  return {
    sourceText: state.sourceText,
    languageId: state.languageId,
    documentKey: state.documentKey,
    compareEditToken: state.compareEditToken,
    editorRevision: state.editorRevision,
    graphAppliedRevision: state.graphAppliedRevision,
    tempModel: {
      treePath:
        state.tempModel.treePath.length > 0
          ? [
              '$',
              ...state.tempModel.treePath
                .map((segment) => formatPathSegment(segment))
                .filter(Boolean),
            ]
          : [],
      diagnostics: state.tempModel.diagnostics,
      cursor: state.tempModel.cursor || (await readCursorText(page)),
      selectionLength: state.tempModel.selectionLength,
      scratchText: state.tempModel.scratchText,
    },
  };
}

export async function readEditorWorkspace(page: Page) {
  return evaluateTreease(page, (treease) => treease.editor.getWorkspace());
}

export async function readRuntimeReadiness(page: Page): Promise<TreeaseRuntimeReadiness> {
  return evaluateTreease(page, (treease) => treease.graph.getRuntimeReadiness());
}

async function waitForRuntimeReadiness(
  page: Page,
  predicate: (readiness: TreeaseRuntimeReadiness) => boolean,
  timeout = DEFAULT_UI_TIMEOUT,
) {
  await expect.poll(async () => predicate(await readRuntimeReadiness(page)), { timeout }).toBe(true);
}

export async function setEditorContent(page: Page, payload: { sourceText: string; language?: string }) {
  await waitForMonacoHook(page, 'source-editor');
  await expect
    .poll(
      async () => {
        const [modelText, state] = await Promise.all([
          getMonacoValue(page, 'source-editor'),
          readEditorState(page),
        ]);
        return {
          editorReady: state.editorRevision > 0,
          modelSynced: state.sourceText === modelText,
        };
      },
      { timeout: DEFAULT_UI_TIMEOUT },
    )
    .toEqual({ editorReady: true, modelSynced: true });

  if (payload.language) {
    await evaluateTreease(page, (treease, language) => {
      treease.editor.setLanguageId(language);
    }, payload.language);
  }
  await evaluateTreease(
    page,
    (treease, args: { hookId: string; value: string }) => {
      treease.editor.setValueExact?.(args.hookId, args.value);
    },
    { hookId: 'source-editor', value: payload.sourceText },
  );
  await expect
    .poll(
      async () => {
        const [modelText, state] = await Promise.all([
          getMonacoValue(page, 'source-editor'),
          readEditorState(page),
        ]);
        // Monaco may normalize some control-leading inputs (for example a
        // standalone UTF-8 BOM). The helper should wait for the editor/store
        // to converge on the editor-accepted text, not on the original payload.
        // Import/graph settlement is a separate concern and should be awaited
        // explicitly by the caller through waitForImportSettled /
        // waitForGraphRendered when needed.
        return {
          modelSynced: state.sourceText === modelText,
          storeSynced: state.sourceText === modelText,
          languageSynced: payload.language ? state.languageId === payload.language : true,
        };
      },
      { timeout: DEFAULT_UI_TIMEOUT },
    )
    .toEqual({ modelSynced: true, storeSynced: true, languageSynced: true });
}

export async function setMonacoValue(page: Page, hookId: string, value: string) {
  await waitForMonacoHook(page, hookId);
  await evaluateTreease(
    page,
    (treease, args: { hookId: string; value: string }) => {
      treease.editor.setValue(args.hookId, args.value);
    },
    { hookId, value },
  );
}

export async function setMonacoPosition(page: Page, hookId: string, lineNumber: number, column: number) {
  await waitForMonacoHook(page, hookId);
  await evaluateTreease(
    page,
    (treease, payload: { hookId: string; lineNumber: number; column: number }) => {
      treease.editor.setPosition(payload.hookId, payload.lineNumber, payload.column);
    },
    { hookId, lineNumber, column },
  );
}

async function setMonacoPositionByTextRaw(page: Page, hookId: string, searchText: string) {
  return evaluateTreease(
    page,
    (treease, payload: { hookId: string; searchText: string }) => {
      const text = treease.editor.getValue(payload.hookId);
      const idx = text.indexOf(payload.searchText);
      if (idx < 0) throw new Error(`Text not found: ${payload.searchText}`);
      const before = text.slice(0, idx);
      const lineNumber = before.split('\n').length;
      const lastNewline = before.lastIndexOf('\n');
      const column = idx - (lastNewline + 1) + 1;
      treease.editor.setPosition(payload.hookId, lineNumber, column);
      return { lineNumber, column };
    },
    { hookId, searchText },
  );
}

export async function setMonacoPositionByText(page: Page, hookId: string, searchText: string) {
  await waitForMonacoHook(page, hookId);
  await setMonacoPositionByTextRaw(page, hookId, searchText);
}

export async function setMonacoPositionByTextAndWaitForTreePath(
  page: Page,
  hookId: string,
  searchText: string,
  expectedPath: string[],
  timeout = CURSOR_PATH_SYNC_TIMEOUT,
) {
  await waitForMonacoHook(page, hookId);
  const previousReadiness = await readRuntimeReadiness(page);
  const expectedPathKey = expectedPath.join('\u0000');
  const position = await setMonacoPositionByTextRaw(page, hookId, searchText);
  await expect
    .poll(
      async () => {
        const [readiness, state] = await Promise.all([
          readRuntimeReadiness(page),
          readEditorState(page),
        ]);
        const cursorPath = readiness.cursorPath;
        const currentPathKey = state.tempModel.treePath.join('\u0000');
        const sameCursorPosition =
          cursorPath.lineNumber === position.lineNumber && cursorPath.column === position.column;
        const requestObserved =
          cursorPath.requestId > previousReadiness.cursorPath.requestId ||
          (sameCursorPosition && currentPathKey === expectedPathKey);
        return {
          cursorSettled:
            requestObserved &&
            cursorPath.settled &&
            cursorPath.documentKey === state.documentKey &&
            cursorPath.revision > 0 &&
            cursorPath.revision <= state.editorRevision &&
            sameCursorPosition,
          treePathSynced: currentPathKey === expectedPathKey,
        };
      },
      { timeout },
    )
    .toEqual({ cursorSettled: true, treePathSynced: true });
}

export async function getMonacoValue(page: Page, hookId: string) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(page, (treease, nextHookId) => treease.editor.getValue(nextHookId), hookId);
}

export async function applyMonacoEdits(
  page: Page,
  hookId: string,
  edits: Array<{
    range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number };
    text: string;
  }>,
) {
  await waitForMonacoHook(page, hookId);
  await evaluateTreease(
    page,
    (
      treease,
      payload: {
        hookId: string;
        edits: Array<{
          range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number };
          text: string;
        }>;
      },
    ) => {
      treease.editor.applyEdits(payload.hookId, payload.edits);
    },
    { hookId, edits },
  );
}

export async function getMonacoScroll(page: Page, hookId: string) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(page, (treease, nextHookId) => treease.editor.getScroll(nextHookId), hookId);
}

export async function getMonacoLanguage(page: Page, hookId: string) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(page, (treease, nextHookId) => treease.editor.getLanguage(nextHookId), hookId);
}

export async function getMonacoMarkers(page: Page, hookId: string) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(page, (treease, nextHookId) => treease.editor.getMarkers?.(nextHookId) ?? [], hookId);
}

export async function countMonacoElements(page: Page, hookId: string, selector: string) {
  const locator = await waitForMonacoHook(page, hookId);
  return locator.locator(selector).count();
}

export async function getMonacoRenderedTokenColor(
  page: Page,
  hookId: string,
  tokenText: string,
  lineNumber?: number,
) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(
    page,
    (treease, payload: { hookId: string; tokenText: string; lineNumber?: number }) =>
      treease.editor.getRenderedTokenColor(payload.hookId, payload.tokenText, payload.lineNumber),
    { hookId, tokenText, lineNumber },
  );
}

export async function getMonacoRenderedTokenColorAtPosition(
  page: Page,
  hookId: string,
  lineNumber: number,
  column: number,
  tokenText?: string,
) {
  await waitForMonacoHook(page, hookId);
  return evaluateTreease(
    page,
    (
      treease,
      payload: { hookId: string; lineNumber: number; column: number; tokenText?: string },
    ) => treease.editor.getRenderedTokenColorAtPosition(payload.hookId, payload.lineNumber, payload.column, payload.tokenText),
    { hookId, lineNumber, column, tokenText },
  );
}

export async function getMonacoInlineClassColor(page: Page, hookId: string, className: string) {
  await waitForMonacoHook(page, hookId);
  const locator = page.getByTestId(`monaco-${hookId}`).locator(`.${className}`).first();
  return locator.evaluate((element) => getComputedStyle(element as HTMLElement).color);
}

export async function setMonacoScroll(page: Page, hookId: string, scrollTop: number, scrollLeft = 0) {
  await waitForMonacoHook(page, hookId);
  await evaluateTreease(
    page,
    (treease, payload: { hookId: string; scrollTop: number; scrollLeft: number }) => {
      treease.editor.setScroll(payload.hookId, payload.scrollTop, payload.scrollLeft);
    },
    { hookId, scrollTop, scrollLeft },
  );
}

export async function chooseFile(
  page: Page,
  options: {
    triggerLabel?: string;
    triggerTestId?: string;
    inputLabel?: string;
    inputIndex?: number;
    fileName: string;
    content: string;
    mimeType?: string;
  },
) {
  if (options.triggerLabel) {
    await page.getByRole('button', { name: options.triggerLabel, exact: true }).click();
  }
  if (options.triggerTestId) {
    await page.getByTestId(options.triggerTestId).click();
  }
  const input = options.inputLabel
    ? page.getByLabel(options.inputLabel).nth(options.inputIndex ?? 0)
    : page.locator('input[type="file"]').nth(options.inputIndex ?? 0);
  await input.setInputFiles({
    name: options.fileName,
    mimeType: options.mimeType ?? 'text/plain',
    buffer: Buffer.from(options.content),
  });
}

export async function dropFile(
  page: Page,
  options: {
    targetTestId: string;
    fileName: string;
    content: string;
    mimeType?: string;
  },
) {
  const target = page.getByTestId(options.targetTestId);
  await expect(target).toBeVisible({ timeout: DEFAULT_UI_TIMEOUT });
  const tracksImport = options.targetTestId === 'source-editor-region' || options.targetTestId === 'import-drop-trigger';
  const previousImportRevision = tracksImport
    ? (await readRuntimeReadiness(page)).import.requestedRevision
    : null;
  await target.evaluate(
    (node, payload) => {
      const dataTransfer = new DataTransfer();
      const file = new File([payload.content], payload.fileName, { type: payload.mimeType ?? 'text/plain' });
      dataTransfer.items.add(file);
      node.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer }));
      node.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer }));
    },
    {
      fileName: options.fileName,
      content: options.content,
      mimeType: options.mimeType ?? 'text/plain',
    },
  );
  if (previousImportRevision !== null) {
    // The drop handler is async; wait until this event owns a new import revision.
    // Otherwise callers can observe the initial settled=true state before import starts.
    await expect
      .poll(
        async () => (await readRuntimeReadiness(page)).import.requestedRevision > previousImportRevision,
        { timeout: DEFAULT_UI_TIMEOUT },
      )
      .toBe(true);
  }
}

export async function waitForEditorRuntimeReady(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await waitForMonacoHook(page, 'source-editor', timeout);
  await expect(page.getByRole('status', { name: 'Editor loading status' })).toHaveCount(0, { timeout });
  await expect(page.getByTestId('editor-runtime-error')).toHaveCount(0, { timeout });
}

export async function waitForEditorReady(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await waitForEditorRuntimeReady(page, timeout);
  await expect
    .poll(
      async () => {
        const workspace = await readEditorWorkspace(page);
        const activeTab = workspace.tabsById[workspace.activeTabId];
        const binding = activeTab
          ? workspace.snapshotBindingsByDocumentKey[activeTab.documentKey]
          : null;
        return {
          hasActiveSnapshot: activeTab?.snapshotId != null,
          hasBoundSnapshot: binding?.snapshotId != null,
        };
      },
      { timeout },
    )
    .toEqual({ hasActiveSnapshot: true, hasBoundSnapshot: true });
  await expect(
    page
      .getByRole('button', { name: 'Graph mode', exact: true })
      .or(page.getByRole('button', { name: 'Text mode', exact: true })),
  ).toBeVisible({ timeout });
}

export async function waitForSettingsReady(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await expect
    .poll(
      async () => evaluateTreease(page, (treease) => treease.settings.getStatus()),
      { timeout },
    )
    .toBe('ready');
}

export async function waitForGraphRendered(
  page: Page,
  timeout = DEFAULT_UI_TIMEOUT,
  target?: { documentKey: string; revision: number },
) {
  await waitForRuntimeReadiness(
    page,
    (lastReadiness) => {
      const targetDocumentKey = target?.documentKey ?? lastReadiness.documentKey;
      const targetRevision = Math.max(1, target?.revision ?? lastReadiness.graph.requestedRevision);
      return (
        lastReadiness.documentKey === targetDocumentKey &&
        lastReadiness.import.settled &&
        lastReadiness.graph.requestedRevision >= targetRevision &&
        lastReadiness.graph.appliedRevision >= targetRevision &&
        lastReadiness.graph.flushedRevision >= targetRevision &&
        lastReadiness.graph.interactiveRevision >= targetRevision &&
        lastReadiness.graph.settled &&
        lastReadiness.graph.settledRevision >= targetRevision
      );
    },
    timeout,
  );
}

export async function waitForImportSettled(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await waitForRuntimeReadiness(page, (readiness) => readiness.import.settled, timeout);
}

export async function waitForPreviewSettled(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await waitForRuntimeReadiness(page, (readiness) => readiness.preview.settled, timeout);
}

export async function waitForSidecarSettled(page: Page, hookId = 'right-editor', timeout = DEFAULT_UI_TIMEOUT) {
  await waitForRuntimeReadiness(
    page,
    (readiness) => readiness.sidecar.settled && readiness.sidecar.hookId === hookId,
    timeout,
  );
}

export async function waitForColumnNavigatorSettled(page: Page, pathKey?: string, timeout = DEFAULT_UI_TIMEOUT) {
  await waitForRuntimeReadiness(
    page,
    (readiness) => readiness.subgraph.settled && (pathKey ? readiness.subgraph.pathKey === pathKey : true),
    timeout,
  );
}

export async function waitForDiagnostics(page: Page, timeout = DEFAULT_UI_TIMEOUT) {
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.diagnostics.length, { timeout })
    .toBeGreaterThan(0);
}

export async function waitForTreePath(page: Page, expectedPath: string[], timeout = DEFAULT_UI_TIMEOUT) {
  await expect.poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout }).toEqual(expectedPath);
}

export async function openMonacoHover(
  page: Page,
  options: { hookId: string; lineNumber: number; column: number; hoverText: string; timeout?: number },
) {
  const timeout = options.timeout ?? DEFAULT_UI_TIMEOUT;
  await setMonacoPosition(page, options.hookId, options.lineNumber, options.column);
  const locator = monacoHook(page, options.hookId);
  await expect(locator).toBeVisible({ timeout });
  const point = await locator.evaluate((node, hoverText) => {
    const editorNode = node as HTMLElement;
    const spans = Array.from(editorNode.querySelectorAll('.view-lines .view-line span')) as HTMLElement[];
    const normalizedHoverText = String(hoverText);
    const exact = spans.find((span) => (span.textContent ?? '').trim() === normalizedHoverText);
    const quoted = spans.find((span) => (span.textContent ?? '').trim() === `"${normalizedHoverText}"`);
    const partials = spans
      .filter((span) => (span.textContent ?? '').includes(normalizedHoverText))
      .sort((a, b) => (a.textContent ?? '').length - (b.textContent ?? '').length);
    const target = exact ?? quoted ?? partials[0];
    if (!target) throw new Error(`Unable to find Monaco token containing "${hoverText}"`);
    const rect = target.getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  }, options.hoverText);
  await page.mouse.move(point.x, point.y);
  await expect
    .poll(
      async () => {
        const hoverCount = await page.evaluate(() => document.querySelectorAll('.monaco-hover').length);
        if (hoverCount > 0) return true;
        await page.mouse.move(point.x + 1, point.y + 1);
        return (await page.evaluate(() => document.querySelectorAll('.monaco-hover').length)) > 0;
      },
      { timeout },
    )
    .toBe(true);
}

export async function readMonacoHoverRows(page: Page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll('.monaco-hover .hover-row-contents')).map((node) =>
      (node.textContent ?? '').trim(),
    ),
  );
}

export async function readMonacoHoverHtml(page: Page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll('.monaco-hover .hover-row-contents')).map((node) => node.innerHTML),
  );
}

export async function expectMonacoHoverContains(page: Page, expectedParts: Array<string | RegExp>, timeout = DEFAULT_UI_TIMEOUT) {
  await expect
    .poll(
      async () => {
        const rows = await readMonacoHoverRows(page);
        const htmls = await readMonacoHoverHtml(page);
        const text = [...rows, ...htmls].join('\n');
        return expectedParts.every((part) => (typeof part === 'string' ? text.includes(part) : part.test(text)));
      },
      { timeout },
    )
    .toBe(true);
}

export type GraphEditEventDetail = TreeaseTestGraphEditEventDetail;
type GraphEditEvent = TreeaseTestGraphEditEvent;

export async function installGraphEditEventCapture(page: Page) {
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible({ timeout: DEFAULT_UI_TIMEOUT });
  await page.evaluate(() => {
    const treease = window._treease;
    if (!treease) {
      throw new Error('window._treease is unavailable');
    }
    const state = window as unknown as { __graphEditCaptureInstalled?: boolean };
    treease.test.resetGraphEditEvents();
    if (state.__graphEditCaptureInstalled) return;
    const pushEvent = (type: string) => (event: Event) => {
      const detail = (event as CustomEvent).detail;
      treease.test.pushGraphEditEvent({ type, detail });
    };
    document.addEventListener('graph-edit-open', pushEvent('open'));
    document.addEventListener('graph-edit-commit', pushEvent('commit'));
    document.addEventListener('graph-edit-result', pushEvent('result'));
    document.addEventListener('graph-edit-probes', pushEvent('probes'));
    state.__graphEditCaptureInstalled = true;
  });
}

async function readGraphEditEvents(page: Page): Promise<GraphEditEvent[]> {
  return evaluateTreease(page, (treease) => treease.test.getGraphEditEvents());
}

async function waitForGraphViewerPaint(page: Page) {
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
  );
}

type GraphRuntimeHandle = WindowTreease['graph'];
type GraphRuntimeScope = 'root' | 'panel' | 'workspace';

async function evaluateGraphRuntime<T, P = undefined>(
  page: Page,
  evaluator: (runtime: GraphRuntimeHandle, payload: P) => T | Promise<T>,
  payload?: P,
): Promise<Awaited<T>> {
  return evaluateTreease(page, (treease, args: { fnSource: string; payload?: P }) => {
    return new Function('runtime', 'payload', `return (${args.fnSource})(runtime, payload);`)(
      treease.graph,
      args.payload,
    ) as T;
  }, { fnSource: evaluator.toString(), payload });
}

export async function getLatestGraphProbes(page: Page): Promise<Array<{ x: number; y: number }>> {
  const directProbes = await evaluateGraphRuntime(page, (runtime) =>
    (runtime.getClickProbeTargets?.('root') ?? [])
      .map((probe) =>
        typeof probe.coord?.x === 'number' && typeof probe.coord?.y === 'number'
          ? { x: Number(probe.coord.x), y: Number(probe.coord.y) }
          : null,
      )
      .filter((probe): probe is { x: number; y: number } => !!probe),
  ).catch(() => []);
  if (directProbes.length > 0) {
    return directProbes;
  }
  const events = await readGraphEditEvents(page);
  const probes = events.filter((evt) => evt.type === 'probes').at(-1)?.detail?.probes ?? [];
  return probes
    .filter((probe): probe is { x: number; y: number } => typeof probe?.x === 'number' && typeof probe?.y === 'number')
    .map((probe) => ({ x: probe.x, y: probe.y }));
}

export async function readGraphHighlight(
  page: Page,
): Promise<{ path: string[]; target?: 'key' | 'value' | 'node'; fill?: string | null } | null> {
  return evaluateGraphRuntime(page, (runtime) => {
    const highlight = runtime.getHighlightTarget?.();
    if (!highlight?.path?.length) return null;
    return {
      path: [
        '$',
        ...highlight.path
          .map((segment) =>
            typeof segment?.key === 'string' && segment.key.length > 0
              ? segment.key
              : typeof segment?.index === 'number'
                ? `[${segment.index}]`
                : '',
          )
          .filter((segment) => segment.length > 0),
      ],
      target: highlight.target,
      fill: typeof highlight.fill === 'string' ? highlight.fill : null,
    };
  });
}

export async function readGraphHighlightRect(page: Page): Promise<{
  path: string[];
  target?: 'key' | 'value' | 'node';
  left: number;
  top: number;
  width: number;
  height: number;
} | null> {
  return evaluateGraphRuntime(page, (runtime) => {
    const highlight = runtime.getHighlightTarget?.();
    if (!highlight?.path?.length || !highlight.rect) return null;
    return {
      path: [
        '$',
        ...highlight.path
          .map((segment) =>
            typeof segment?.key === 'string' && segment.key.length > 0
              ? segment.key
              : typeof segment?.index === 'number'
                ? `[${segment.index}]`
                : '',
          )
          .filter((segment) => segment.length > 0),
      ],
      target: highlight.target,
      left: Number(highlight.rect.left ?? 0),
      top: Number(highlight.rect.top ?? 0),
      width: Number(highlight.rect.width ?? 0),
      height: Number(highlight.rect.height ?? 0),
    };
  });
}

export async function readGraphViewportRect(
  page: Page,
): Promise<{ left: number; top: number; width: number; height: number } | null> {
  const box = await page.getByTestId('graph-viewer-canvas').boundingBox();
  if (!box) return null;
  return {
    left: box.x,
    top: box.y,
    width: box.width,
    height: box.height,
  };
}

export async function readGraphHighlightWorld(page: Page): Promise<{
  path: string[];
  target?: 'key' | 'value' | 'node';
  highlight: { x: number; y: number };
  viewportCenter: { x: number; y: number };
} | null> {
  return evaluateGraphRuntime(page, (runtime) => {
    const highlight = runtime.getHighlightTarget?.();
    if (!highlight?.path?.length || !highlight.world?.highlight || !highlight.world?.viewportCenter) return null;
    return {
      path: [
        '$',
        ...highlight.path
          .map((segment) =>
            typeof segment?.key === 'string' && segment.key.length > 0
              ? segment.key
              : typeof segment?.index === 'number'
                ? `[${segment.index}]`
                : '',
          )
          .filter((segment) => segment.length > 0),
      ],
      target: highlight.target,
      highlight: {
        x: Number(highlight.world.highlight.x ?? 0),
        y: Number(highlight.world.highlight.y ?? 0),
      },
      viewportCenter: {
        x: Number(highlight.world.viewportCenter.x ?? 0),
        y: Number(highlight.world.viewportCenter.y ?? 0),
      },
    };
  });
}

export async function readTempGraphSelection(
  page: Page,
): Promise<{ path: string[]; target?: 'key' | 'value' | 'node'; source?: string } | null> {
  return evaluateTreease(page, (treease) => {
    const highlight = treease.editor.getState().tempModel.graphHighlight;
    if (!highlight?.path?.length) return null;
    const path = highlight.path
      .map((segment) => {
        const key = typeof segment?.key === 'string' ? segment.key : '';
        if (key.length > 0) return key;
        return typeof segment?.index === 'number' ? `[${segment.index}]` : '';
      })
      .filter((segment) => segment.length > 0);
    return {
      path: ['$', ...path],
      target: highlight.target,
      source: highlight.source,
    };
  });
}

type GraphClickProbeSnapshot = {
  id: string;
  target?: 'key' | 'value' | 'node';
  text: string;
  valueType: string;
  isTableCell: boolean;
  isHeader: boolean;
  nodeType: string;
  path: string[];
  rawPath: TreeaseRuntimePathSeg[];
  coord: { x: number; y: number } | null;
  rect: { left: number; top: number; width: number; height: number } | null;
  textColor: string | null;
};

async function readGraphClickProbesByKeys(page: Page, scope: GraphRuntimeScope): Promise<GraphClickProbeSnapshot[]> {
  return evaluateGraphRuntime(
    page,
    (runtime, requestedScope) => {
      const readPathKey = (value: unknown): string => {
        if (typeof value !== 'string') {
          throw new Error(`Expected path key string, received ${Object.prototype.toString.call(value)}`);
        }
        return value;
      };
      const normalizePath = (path: TreeaseRuntimePathSeg[] | undefined): TreeaseRuntimePathSeg[] =>
        (path ?? []).map((segment) => ({
          tag: segment?.tag,
          index: segment?.index,
          key: typeof segment?.key === 'undefined' ? undefined : readPathKey(segment?.key),
        }));
      const formatPath = (path: TreeaseRuntimePathSeg[] | undefined): string[] =>
        normalizePath(path)
          .map((segment): string => (typeof segment.key === 'string' && segment.key.length > 0 ? segment.key : typeof segment.index === 'number' ? `[${segment.index}]` : ''))
          .filter((segment): segment is string => segment.length > 0);

      return (runtime.getClickProbeTargets?.(requestedScope as 'root' | 'workspace') ?? []).map((probe) => {
        const rawPath = normalizePath(probe.cell?.path);
        return {
          id: String(probe.id ?? ''),
          target: probe.target,
          text: probe.cell?.text ?? '',
          valueType: probe.cell?.valueType ?? '',
          isTableCell: !!probe.cell?.isTableCell,
          isHeader: !!probe.cell?.isHeader,
          nodeType: probe.nodeType ?? '',
          textColor: typeof probe.textColor === 'string' ? probe.textColor : null,
          path: formatPath(probe.cell?.path),
          rawPath,
          coord:
            typeof probe.coord?.x === 'number' && typeof probe.coord?.y === 'number'
              ? { x: Number(probe.coord.x), y: Number(probe.coord.y) }
              : null,
          rect:
            typeof probe.rect?.left === 'number' &&
            typeof probe.rect?.top === 'number' &&
            typeof probe.rect?.width === 'number' &&
            typeof probe.rect?.height === 'number'
              ? {
                  left: Number(probe.rect.left),
                  top: Number(probe.rect.top),
                  width: Number(probe.rect.width),
                  height: Number(probe.rect.height),
                }
              : null,
        };
      });
    },
    scope,
  );
}

export async function readGraphClickProbes(page: Page): Promise<GraphClickProbeSnapshot[]> {
  return readGraphClickProbesByKeys(page, 'root');
}

export async function readColumnNavigatorClickProbes(page: Page): Promise<GraphClickProbeSnapshot[]> {
  return readGraphClickProbesByKeys(page, 'workspace');
}

export async function readGraphRevealProbe(
  page: Page,
  probeId: string,
): Promise<{
  path: string[];
  rawPath: TreeaseRuntimePathSeg[];
  target?: 'key' | 'value' | 'node';
} | null> {
  return readGraphClickProbes(page).then((probes) => {
    const probe = probes.find((candidate) => candidate.id === probeId);
    if (!probe?.path?.length) return null;
    return {
      path: ['$', ...probe.path],
      rawPath: probe.rawPath,
      target: probe.target,
    };
  });
}

export async function readGraphLastReveal(page: Page): Promise<{
  path: string[];
  rawPath: TreeaseRuntimePathSeg[];
  target?: 'key' | 'value' | 'node';
} | null> {
  return evaluateGraphRuntime(page, (runtime) => {
    const reveal = runtime.getLastReveal?.();
    if (!reveal?.path?.length) return null;
    const path = reveal.path
      .map((segment) => {
        const key = typeof segment?.key === 'string' ? segment.key : '';
        if (key.length > 0) return key;
        return typeof segment?.index === 'number' ? `[${segment.index}]` : '';
      })
      .filter((segment) => segment.length > 0);
    return {
      path: ['$', ...path],
      rawPath: reveal.path,
      target: reveal.target,
    };
  });
}

export async function readGraphLastRowScroll(
  page: Page,
): Promise<{ path: string[]; rawPath: TreeaseRuntimePathSeg[]; scrollY: number } | null> {
  return evaluateGraphRuntime(page, (runtime) => {
    const scrollState = runtime.getRowScrollState?.() ?? null;
    if (!scrollState?.path?.length || typeof scrollState.scrollY !== 'number') return null;
    const path = scrollState.path
      .map((segment) => {
        const key = typeof segment?.key === 'string' ? segment.key : '';
        if (key.length > 0) return key;
        return typeof segment?.index === 'number' ? `[${segment.index}]` : '';
      })
      .filter((segment) => segment.length > 0);
    return {
      path: ['$', ...path],
      rawPath: scrollState.path,
      scrollY: scrollState.scrollY,
    };
  });
}

export async function readGraphHitResult(
  page: Page,
  point: { x: number; y: number },
  scope: GraphRuntimeScope = 'root',
): Promise<{
  scope: GraphRuntimeScope;
  point: { x: number; y: number };
  hit: {
    id: string;
    target?: 'key' | 'value' | 'node';
    path: string[];
    text: string;
  } | null;
} | null> {
  const canvasBox = await page.getByTestId('graph-viewer-canvas').boundingBox();
  if (!canvasBox) return null;
  return evaluateGraphRuntime(
    page,
    (runtime, payload) => {
      const result = runtime.getHitResult?.(payload.point);
      if (!result) return null;
      return {
        scope: payload.scope,
        point: {
          x: Number(result.point.x ?? 0),
          y: Number(result.point.y ?? 0),
        },
        hit: result.hit
          ? {
              id: String(result.hit.id ?? ''),
              target: result.hit.target,
              path: [
                '$',
                ...(result.hit.cell?.path ?? [])
                  .map((segment) =>
                    typeof segment?.key === 'string' && segment.key.length > 0
                      ? segment.key
                      : typeof segment?.index === 'number'
                        ? `[${segment.index}]`
                        : '',
                  )
                  .filter((segment) => segment.length > 0),
              ],
              text: result.hit.cell?.text ?? '',
            }
          : null,
      };
    },
    {
      point: {
        x: canvasBox.x + point.x,
        y: canvasBox.y + point.y,
      },
      scope,
    },
  );
}

export async function clearGraphLastReveal(page: Page): Promise<void> {
  await evaluateGraphRuntime(page, (runtime) => runtime.clearLastReveal?.());
}

export async function revealGraphPath(
  page: Page,
  path: Array<{ key?: unknown; index?: number; tag?: number }>,
  options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean },
): Promise<void> {
  const normalizedPath = normalizeBridgePath(path);
  await evaluateTreease(
    page,
    (
      treease,
      payload: {
        path: Array<{ key?: string; index?: number; tag?: number }>;
        options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean };
      },
    ) => {
      treease.graph.revealPath(payload.path, payload.options);
    },
    { path: normalizedPath, options },
  );
}

export async function setTempGraphSelection(
  page: Page,
  path: Array<{ key?: unknown; index?: number; tag?: number }>,
  target?: 'key' | 'value' | 'node',
): Promise<void> {
  const normalizedPath = normalizeBridgePath(path);
  await evaluateTreease(
    page,
    (treease, payload: { path: Array<{ key?: string; index?: number; tag?: number }>; target?: 'key' | 'value' | 'node' }) => {
      treease.editor.setTempGraphSelection(payload.path, payload.target);
    },
    { path: normalizedPath, target },
  );
}

export async function resolveCursorForPath(
  page: Page,
  options: {
    path: Array<{ key?: unknown; index?: number; tag?: number }>;
    target?: 'key' | 'value' | 'node';
  },
): Promise<string | null> {
  return evaluateTreease(page, async (treease, { path, target }) => {
    if (!path?.length) return null;
    const state = treease.editor.getState();
    const settingsState = treease.settings.getState();
    const resolvedTarget = target === 'key' ? 'key' : 'value';
    const span = await treease.worker.callShared<PathSpan | null>('pathSpan', {
      documentKey: state.documentKey,
      language: state.languageId,
      text: state.sourceText,
      path,
      target: resolvedTarget,
      nest: !!settingsState.settings.parser.enableNest,
    });
    if (!span || span.row < 0 || span.column < 0) return null;
    return `Ln ${span.row + 1}, Col ${span.column + 1}`;
  }, options);
}

export async function installClipboardCapture(page: Page): Promise<void> {
  await page.evaluate(() => {
    const treease = window._treease;
    if (!treease) {
      throw new Error('window._treease is unavailable');
    }
    treease.test.resetClipboardWrites();
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText(text: string) {
          treease.test.pushClipboardWrite(text);
          return Promise.resolve();
        },
      },
    });
  });
}

export async function readClipboardWrites(page: Page): Promise<string[]> {
  return evaluateTreease(page, (treease) => treease.test.getClipboardWrites());
}

export async function clearGraphEditEvents(page: Page): Promise<void> {
  await evaluateTreease(page, (treease) => {
    treease.test.resetGraphEditEvents();
  });
}

export async function buildGraphSnapshot(page: Page): Promise<TreeaseGraphBuildResult> {
  return evaluateTreease(page, (treease) => treease.graph.buildGraph());
}

export async function generatePreview(page: Page, payload: Parameters<WindowTreease['preview']['generate']>[0]) {
  return evaluateTreease(page, (treease, request) => treease.preview.generate(request), payload);
}

export async function readHoverPanelDebugPhase(_page: Page): Promise<string> {
  return '';
}

export async function readSettingsSnapshot(page: Page): Promise<ReturnType<WindowTreease['settings']['getState']>> {
  return evaluateTreease(page, (treease) => treease.settings.getState());
}

export async function callTreeaseWorker<T>(
  page: Page,
  type: string,
  payload?: Record<string, any>,
  transfer?: Transferable[],
): Promise<T> {
  return evaluateTreease(page, (treease, args) => treease.worker.callShared<T>(args.type, args.payload, args.transfer), {
    type,
    payload,
    transfer,
  });
}

export async function getElementWidth(page: Page, testId: string) {
  const locator = page.getByTestId(testId);
  await expect(locator).toBeVisible({ timeout: DEFAULT_UI_TIMEOUT });
  const box = await locator.boundingBox();
  if (!box) throw new Error(`Unable to read bounding box for ${testId}`);
  return box.width;
}

export async function dragSplitterDivider(page: Page, deltaX: number) {
  const divider = page.getByTestId('splitter-divider');
  await expect(divider).toBeVisible({ timeout: DEFAULT_UI_TIMEOUT });
  const box = await divider.boundingBox();
  if (!box) throw new Error('Unable to read splitter divider bounds');
  const startX = box.x + box.width / 2;
  const startY = box.y + box.height / 2;
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + deltaX, startY, { steps: 12 });
  await page.mouse.up();
}

export async function clickGraphProbeAt(page: Page, probe: { x: number; y: number }) {
  const canvas = page.getByTestId('graph-viewer-canvas');
  await waitForGraphViewerPaint(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');
  await page.mouse.click(box.x + probe.x, box.y + probe.y);
}

export async function clickColumnNavigatorProbeAt(page: Page, probe: { x: number; y: number }) {
  const workspace = page.getByTestId('column-navigator-graph');
  await waitForGraphViewerPaint(page);
  const box = await workspace.boundingBox();
  if (!box) throw new Error('column-navigator-graph bounding box missing');
  await page.mouse.click(box.x + probe.x, box.y + probe.y);
}

export async function clickGraphProbe(page: Page, probeIndex = 0) {
  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: DEFAULT_UI_TIMEOUT })
    .toBeGreaterThan(probeIndex);
  const probe = (await getLatestGraphProbes(page))[probeIndex];
  if (!probe) throw new Error(`graph probe ${probeIndex} missing`);
  await clickGraphProbeAt(page, probe);
}

async function commitGraphValueLikeViaProbes(
  page: Page,
  options: {
    sourceText: string;
    inputText: string;
    selectAllModifier: 'Meta' | 'Control';
    matchesOpenEvent: (detail: GraphEditEventDetail) => boolean;
    matchesProbeSnapshot?: (probe: GraphClickProbeSnapshot) => Promise<boolean> | boolean;
    verifyCommitted: (sourceText: string) => boolean;
    readProbes: () => Promise<Array<{ x: number; y: number }>>;
    readProbeSnapshots: () => Promise<GraphClickProbeSnapshot[]>;
    commitByHook: (probeId: string, text: string) => Promise<boolean>;
    waitForProbesAfterReset: () => Promise<void>;
    restoreAfterReset?: () => Promise<void>;
  },
): Promise<boolean> {
  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) return false;

  await expect.poll(async () => (await options.readProbeSnapshots()).length, { timeout: 3_000 }).toBeGreaterThan(0);
  let probeSnapshots = await options.readProbeSnapshots();
  const probeCount = probeSnapshots.length;

  for (let index = 0; index < probeCount; index += 1) {
    const probe = probeSnapshots[index];
    if (!probe?.id) continue;
    const matchesProbe = options.matchesProbeSnapshot
      ? await options.matchesProbeSnapshot(probe)
      : !probe.rawPath?.length ||
        options.matchesOpenEvent({
          path: probe.rawPath,
          kind: probe.target,
        } as GraphEditEventDetail);
    if (!matchesProbe) {
      continue;
    }

    const committedByHook = await options.commitByHook(probe.id, options.inputText);
    if (!committedByHook) continue;
    const committed = await expect
      .poll(
        async () => {
          const current = (await readEditorState(page)).sourceText;
          return options.verifyCommitted(current);
        },
        { timeout: 2_000 },
      )
      .toBe(true)
      .then(() => true)
      .catch(() => false);
    if (committed) {
      const readiness = await readRuntimeReadiness(page);
      await waitForGraphRendered(page, DEFAULT_UI_TIMEOUT, {
        documentKey: readiness.documentKey,
        revision: readiness.editorRevision,
      });
      return true;
    }
    await setEditorContent(page, { sourceText: options.sourceText });
    await clearGraphEditEvents(page);
    await options.restoreAfterReset?.();
    await options.waitForProbesAfterReset();
    probeSnapshots = await options.readProbeSnapshots();
  }

  const mouseProbeIndexes = [];
  for (const [index, probe] of probeSnapshots.entries()) {
    const matchesProbe = options.matchesProbeSnapshot
      ? await options.matchesProbeSnapshot(probe)
      : !probe.rawPath?.length ||
        options.matchesOpenEvent({
          path: probe.rawPath,
          kind: probe.target,
        } as GraphEditEventDetail);
    if (matchesProbe) mouseProbeIndexes.push(index);
  }

  for (const index of mouseProbeIndexes) {
    await setEditorContent(page, { sourceText: options.sourceText });
    await clearGraphEditEvents(page);
    await options.restoreAfterReset?.();
    await options.waitForProbesAfterReset();
    probeSnapshots = await options.readProbeSnapshots();

    const probe = probeSnapshots[index];
    if (!probe?.coord) continue;
    const x = box.x + probe.coord.x;
    const y = box.y + probe.coord.y;

    const beforeOpenCount = (await readGraphEditEvents(page)).filter((evt) => evt.type === 'open').length;
    await page.mouse.dblclick(x, y);

    let opened = false;
    try {
      await expect
        .poll(
          async () => {
            const all = await readGraphEditEvents(page);
            const opens = all.filter((evt) => evt.type === 'open');
            if (opens.length <= beforeOpenCount) return false;
            const fresh = opens.slice(beforeOpenCount);
            return fresh.some((evt) => options.matchesOpenEvent(evt.detail));
          },
          { timeout: 1_500 },
        )
        .toBe(true);
      opened = true;
    } catch {
      opened = false;
    }

    if (!opened) continue;

    await page.keyboard.press(`${options.selectAllModifier}+A`);
    await page.keyboard.type(options.inputText);
    await page.getByTestId('monaco-source-editor').click();

    let committed = false;
    try {
      await expect
        .poll(
          async () => {
            const current = (await readEditorState(page)).sourceText;
            return options.verifyCommitted(current);
          },
          { timeout: 2_000 },
        )
        .toBe(true);
      committed = true;
    } catch {
      committed = false;
    }

    if (committed) {
      const readiness = await readRuntimeReadiness(page);
      await waitForGraphRendered(page, DEFAULT_UI_TIMEOUT, {
        documentKey: readiness.documentKey,
        revision: readiness.editorRevision,
      });
      return true;
    }
  }

  return false;
}

export async function commitGraphValueViaProbes(
  page: Page,
  options: {
    sourceText: string;
    inputText: string;
    selectAllModifier: 'Meta' | 'Control';
    matchesOpenEvent: (detail: GraphEditEventDetail) => boolean;
    verifyCommitted: (sourceText: string) => boolean;
  },
): Promise<boolean> {
  return commitGraphValueLikeViaProbes(page, {
    ...options,
    readProbes: () => getLatestGraphProbes(page),
    readProbeSnapshots: () => readGraphClickProbes(page),
    commitByHook: (probeId, text) =>
      evaluateGraphRuntime(page, async (runtime, payload: { probeId: string; text: string }) => {
        return (await runtime.commitProbe(payload.probeId, payload.text)) ?? false;
      }, { probeId, text }),
    waitForProbesAfterReset: async () => {
      await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: DEFAULT_UI_TIMEOUT }).toBeGreaterThan(0);
    },
    matchesProbeSnapshot: (probe) =>
      !probe.rawPath?.length ||
      options.matchesOpenEvent({
        path: probe.rawPath,
        kind: probe.target,
      } as GraphEditEventDetail),
  });
}
