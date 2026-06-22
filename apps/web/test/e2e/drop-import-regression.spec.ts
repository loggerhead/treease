import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type Page } from './fixtures';
import {
  dropFile,
  getLatestGraphProbes,
  getMonacoValue,
  readEditorState,
  readImportStreamState,
  readTempGraphSelection,
  setMonacoPosition,
  setMonacoPositionByText,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const IS_CI = !!process.env.CI;
const SOURCE_DROP_BUDGET_MS = IS_CI ? 10_000 : 5_000;
const LARGE_IMPORT_FIRST_VISIBLE_BUDGET_MS = IS_CI ? 6_000 : 3_000;
const HOVER_FIXTURE_IMPORT_BUDGET_MS = IS_CI ? 10_000 : 5_000;
const oneMbMinJsonFixtureText = readFileSync(join(process.cwd(), '../../test/fixtures/json/1MB-min.1.json'), 'utf8');
const oneMbMinJsonFixturePath = join(process.cwd(), '../../test/fixtures/json/1MB-min.1.json');
const oneMbMinJsonRows = JSON.parse(oneMbMinJsonFixtureText) as Array<{ name: string; language: string; id: string }>;
const largeJsonFixtureText = readFileSync(join(process.cwd(), '../../test/fixtures/json/5MB-min.1.json'), 'utf8');
const hoverPanelFixtureText = readFileSync(join(process.cwd(), '../../test/fixtures/json/2mb.1.json'), 'utf8');
const smallImportedRowsText = JSON.stringify(oneMbMinJsonRows.slice(0, 2), null, 2);

function buildLargeJsonText(recordCount: number) {
  return JSON.stringify(
    {
      meta: {
        suite: 'drop-import-regression',
        recordCount,
      },
      items: Array.from({ length: recordCount }, (_, index) => ({
        id: index,
        label: `item-${index}`,
        enabled: index % 2 === 0,
        score: index / 10,
        tags: [`tag-${index % 5}`, `group-${index % 9}`],
        nested: {
          owner: `owner-${index % 11}`,
          flags: {
            archived: index % 7 === 0,
            pinned: index % 13 === 0,
          },
          points: Array.from({ length: 4 }, (_, pointIndex) => pointIndex + index),
        },
      })),
    },
    null,
    2,
  );
}


async function readGraphRevisions(page: Page) {
  return page.evaluate(() => {
    const treease = window._treease;
    if (!treease) throw new Error('window._treease is unavailable');
    const state = treease.editor.getState();
    return {
      editorRevision: state.editorRevision,
      graphAppliedRevision: state.graphAppliedRevision,
    };
  });
}

async function readCommittedRenderCallCount(page: Page): Promise<number> {
  return page.evaluate(() => {
    const state = window._treease?.graph.getStreamState();
    return Number(state?.renderCalls ?? 0);
  });
}

async function expectWithinBudget(label: string, budgetMs: number, action: () => Promise<void>) {
  const startedAt = Date.now();
  await action();
  const elapsedMs = Date.now() - startedAt;
  expect(elapsedMs, `${label} exceeded budget: ${elapsedMs}ms > ${budgetMs}ms`).toBeLessThanOrEqual(budgetMs);
}

async function readGraphValueTextsByPath(page: Page, wantedPaths: string[]): Promise<Record<string, string[]>> {
  return page.evaluate((paths) => {
    const treease = window._treease;
    if (!treease) throw new Error('window._treease is unavailable');
    const wanted = new Set(paths);
    const probes = treease.graph.getClickProbeTargets('root') ?? [];
    const byPath: Record<string, string[]> = {};
    for (const probe of probes) {
      if (probe.target !== 'value' || probe.nodeType !== 'Text') continue;
      const path = (probe.cell?.path ?? [])
        .map((segment) =>
          typeof segment?.key === 'string' && segment.key.length > 0
            ? segment.key
            : typeof segment?.index === 'number'
              ? `[${segment.index}]`
              : ''
        )
        .filter((segment) => segment.length > 0)
        .join('.');
      if (!wanted.has(path)) continue;
      const text = probe.cell?.text ?? '';
      if (!byPath[path]) byPath[path] = [];
      if (!byPath[path].includes(text)) byPath[path].push(text);
    }
    return byPath;
  }, wantedPaths);
}

async function readGraphLayoutViolations(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseCollectGraphLayoutViolations?: (graph: unknown) => unknown[];
    };
    const treease = window._treease;
    if (!treease) throw new Error('window._treease is unavailable');
    if (!runtimeWindow.__treeaseCollectGraphLayoutViolations) {
      throw new Error('streaming graph layout observation is unavailable');
    }
    const graph = treease.graph.getLastGraphData?.();
    return runtimeWindow.__treeaseCollectGraphLayoutViolations(graph).slice(0, 10);
  });
}

async function installStreamingGraphLayoutObservation(page: Page) {
  await page.evaluate(() => {
    type LayoutViolation = {
      type: string;
      nodePath?: string;
      first?: unknown;
      second?: unknown;
      rowIndex?: number;
      columnIndex?: number;
      detail?: string;
    };
    type Sample = { phase: string | null; violations: LayoutViolation[] };
    const runtimeWindow = window as Window & {
      __treeaseCollectGraphLayoutViolations?: (graph: unknown) => LayoutViolation[];
      __treeaseStreamingGraphLayoutObservation?: { stopped: boolean; samples: Sample[] };
    };
    runtimeWindow.__treeaseStreamingGraphLayoutObservation = { stopped: false, samples: [] };

    const pathText = (path: any[]) =>
      (path ?? [])
        .map((segment: any) =>
          typeof segment?.key === 'string' && segment.key.length > 0
            ? segment.key
            : typeof segment?.index === 'number'
              ? `[${segment.index}]`
              : ''
        )
        .filter((segment: string) => segment.length > 0)
        .join('.') || '$';
    const boxSnapshot = (owner: any, path: string) => ({
      path,
      x: Number(owner?.boxArgs?.x ?? 0),
      y: Number(owner?.boxArgs?.y ?? 0),
      width: Number(owner?.boxArgs?.width ?? 0),
      height: Number(owner?.boxArgs?.height ?? 0),
    });
    runtimeWindow.__treeaseCollectGraphLayoutViolations = (graph: unknown) => {
      const violations: LayoutViolation[] = [];
      const nodes = Array.isArray((graph as any)?.nodes) ? (graph as any).nodes : [];
      const nodeBoxes = nodes
        .map((node: any) => {
          const path = pathText(node.path ?? node.key?.path ?? []);
          const parentPath = pathText((node.path ?? node.key?.path ?? []).slice(0, -1));
          return {
            renderHandle: Number(node.renderHandle ?? -1),
            kind: String(node.kind ?? ''),
            parentPath,
            ...boxSnapshot(node, path),
          };
        })
        .filter((box: any) => box.width > 0 && box.height > 0);

      const nodeBoxByBounds = new Map<string, any>();
      for (const nodeBox of nodeBoxes) {
        const key = `${Math.round(nodeBox.x)}:${Math.round(nodeBox.y)}:${Math.round(nodeBox.width)}:${Math.round(nodeBox.height)}`;
        const first = nodeBoxByBounds.get(key);
        if (first && first.renderHandle !== nodeBox.renderHandle) {
          violations.push({
            type: first.parentPath === nodeBox.parentPath ? 'sibling-node-overlap' : 'node-overlap',
            first,
            second: nodeBox,
          });
        } else {
          nodeBoxByBounds.set(key, nodeBox);
        }
      }

      for (const node of nodes) {
        const nodePath = pathText(node.path ?? node.key?.path ?? []);
        const rows = Array.isArray(node.table?.rows) ? node.table.rows : [];
        const columns = Array.isArray(node.table?.columns) ? node.table.columns : [];

        for (let rowIndex = 1; rowIndex < rows.length; rowIndex += 1) {
          const previous = boxSnapshot(rows[rowIndex - 1], `${nodePath}[row ${rowIndex - 1}]`);
          const current = boxSnapshot(rows[rowIndex], `${nodePath}[row ${rowIndex}]`);
          if (current.y < previous.y + previous.height) {
            violations.push({
              type: 'table-row-overlap',
              nodePath,
              rowIndex,
              first: previous,
              second: current,
            });
          }
        }

        for (let columnIndex = 1; columnIndex < columns.length; columnIndex += 1) {
          const previous = boxSnapshot(columns[columnIndex - 1], `${nodePath}[column ${columnIndex - 1}]`);
          const current = boxSnapshot(columns[columnIndex], `${nodePath}[column ${columnIndex}]`);
          if (current.x < previous.x + previous.width) {
            violations.push({
              type: 'table-column-overlap',
              nodePath,
              columnIndex,
              first: previous,
              second: current,
            });
          }
        }

        rows.forEach((row: any, rowIndex: number) => {
          const rowBox = boxSnapshot(row, `${nodePath}[row ${rowIndex}]`);
          const cells = Array.isArray(row.cells) ? row.cells : [];
          for (let columnIndex = 1; columnIndex < cells.length; columnIndex += 1) {
            const previous = boxSnapshot(cells[columnIndex - 1], `${nodePath}[row ${rowIndex}][cell ${columnIndex - 1}]`);
            const current = boxSnapshot(cells[columnIndex], `${nodePath}[row ${rowIndex}][cell ${columnIndex}]`);
            if (current.x < previous.x + previous.width) {
              violations.push({
                type: 'table-cell-overlap',
                nodePath,
                rowIndex,
                columnIndex,
                first: previous,
                second: current,
              });
            }
          }
          cells.forEach((cell: any, columnIndex: number) => {
            const cellBox = boxSnapshot(cell, `${nodePath}[row ${rowIndex}][cell ${columnIndex}]`);
            if (cellBox.width > rowBox.width || cellBox.height > rowBox.height) {
              violations.push({
                type: 'table-cell-exceeds-row',
                nodePath,
                rowIndex,
                columnIndex,
                first: rowBox,
                second: cellBox,
              });
            }
          });
        });
      }

      return violations;
    };

    const tick = () => {
      const observation = runtimeWindow.__treeaseStreamingGraphLayoutObservation;
      if (!observation || observation.stopped) return;
      const progressState = window._treease?.graph?.getStreamProgressState?.() ?? null;
      const phase = progressState?.phase ?? null;
      if (phase && phase !== 'idle') {
        const graph = window._treease?.graph?.getLastGraphData?.();
        const violations = runtimeWindow.__treeaseCollectGraphLayoutViolations(graph);
        if (violations.length > 0) {
          observation.samples.push({ phase, violations: violations.slice(0, 3) });
        }
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

async function stopStreamingGraphLayoutObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseStreamingGraphLayoutObservation?: {
        stopped: boolean;
        samples: Array<{ phase: string | null; violations: unknown[] }>;
      };
    };
    const observation = runtimeWindow.__treeaseStreamingGraphLayoutObservation;
    if (!observation) return null;
    observation.stopped = true;
    return observation;
  });
}


async function readGraphTableChildLeaks(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseCollectGraphTableChildLeaks?: (graph: unknown) => unknown[];
    };
    const treease = window._treease;
    if (!treease) throw new Error('window._treease is unavailable');
    if (!runtimeWindow.__treeaseCollectGraphTableChildLeaks) {
      throw new Error('streaming graph table leak observation is unavailable');
    }
    const graph = treease.graph.getLastGraphData?.();
    return runtimeWindow.__treeaseCollectGraphTableChildLeaks(graph).slice(0, 10);
  });
}

async function installStreamingGraphTableLeakObservation(page: Page) {
  await page.evaluate(() => {
    type TableChildLeak = {
      tablePath: string;
      childPath: string;
      childRenderHandle: number;
    };
    type Sample = { phase: string | null; leaks: TableChildLeak[] };
    const runtimeWindow = window as Window & {
      __treeaseCollectGraphTableChildLeaks?: (graph: unknown) => TableChildLeak[];
      __treeaseStreamingGraphTableLeakObservation?: { stopped: boolean; samples: Sample[] };
    };
    runtimeWindow.__treeaseStreamingGraphTableLeakObservation = { stopped: false, samples: [] };

    const pathSegments = (path: any[]) =>
      (path ?? []).map((segment: any) => {
        const key = typeof segment?.key === 'string' ? segment.key : '';
        if (key.length > 0) return key;
        return typeof segment?.index === 'number' ? `[${segment.index}]` : '';
      });
    const pathText = (path: any[]) => pathSegments(path).filter(Boolean).join('.') || '$';
    const pathStartsWith = (candidate: any[], prefix: any[]) => {
      if (!Array.isArray(candidate) || !Array.isArray(prefix) || candidate.length < prefix.length) return false;
      for (let index = 0; index < prefix.length; index += 1) {
        const left = candidate[index] ?? {};
        const right = prefix[index] ?? {};
        if (left.tag !== right.tag || left.key !== right.key || left.index !== right.index) return false;
      }
      return true;
    };

    runtimeWindow.__treeaseCollectGraphTableChildLeaks = (graph: unknown) => {
      const nodes = Array.isArray((graph as any)?.nodes) ? (graph as any).nodes : [];
      const tableNodes = nodes.filter((node: any) => node?.table && Array.isArray(node.path));
      const leaks: TableChildLeak[] = [];
      for (const tableNode of tableNodes) {
        const tablePath = tableNode.path ?? [];
        for (const node of nodes) {
          const nodePath = node?.path ?? [];
          if (
            node !== tableNode &&
            !node?.table &&
            Array.isArray(nodePath) &&
            nodePath.length > tablePath.length &&
            pathStartsWith(nodePath, tablePath)
          ) {
            leaks.push({
              tablePath: pathText(tablePath),
              childPath: pathText(nodePath),
              childRenderHandle: Number(node.renderHandle ?? -1),
            });
          }
        }
      }
      return leaks;
    };

    const tick = () => {
      const observation = runtimeWindow.__treeaseStreamingGraphTableLeakObservation;
      if (!observation || observation.stopped) return;
      const progressState = window._treease?.graph?.getStreamProgressState?.() ?? null;
      const phase = progressState?.phase ?? null;
      if (phase && phase !== 'idle') {
        const graph = window._treease?.graph?.getLastGraphData?.();
        const leaks = runtimeWindow.__treeaseCollectGraphTableChildLeaks(graph);
        if (leaks.length > 0) {
          observation.samples.push({ phase, leaks: leaks.slice(0, 5) });
        }
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

async function stopStreamingGraphTableLeakObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseStreamingGraphTableLeakObservation?: {
        stopped: boolean;
        samples: Array<{ phase: string | null; leaks: unknown[] }>;
      };
    };
    const observation = runtimeWindow.__treeaseStreamingGraphTableLeakObservation;
    if (!observation) return null;
    observation.stopped = true;
    return observation;
  });
}
async function installGraphProgressObservation(page: Page) {
  await page.evaluate(() => {
    type GraphProgressSample = {
      streamRunId: string;
      value: number;
      roundedValue: number;
      phase: string | null;
    };
    type GraphProgressObservation = {
      stopped: boolean;
      samples: GraphProgressSample[];
    };
    const runtimeWindow = window as Window & {
      _treease?: {
        graph?: {
          getStreamProgressState?: () => {
            value?: number;
            phase?: string | null;
            visible?: boolean;
            streamRunId?: string;
          } | null;
        };
      };
      __treeaseGraphProgressObservation?: GraphProgressObservation;
      __treeaseGraphProgressDebug?: Array<Record<string, unknown>>;
    };
    runtimeWindow.__treeaseGraphProgressDebug = [];
    runtimeWindow.__treeaseGraphProgressObservation = {
      stopped: false,
      samples: [],
    };
    const tick = () => {
      const observation = runtimeWindow.__treeaseGraphProgressObservation;
      if (!observation || observation.stopped) return;
      const progressState = runtimeWindow._treease?.graph?.getStreamProgressState?.() ?? null;
      if (progressState && progressState.phase && progressState.phase !== 'idle') {
        const value = Number(progressState.value ?? Number.NaN);
        if (Number.isFinite(value)) {
          const sample: GraphProgressSample = {
            streamRunId: String(progressState.streamRunId ?? ''),
            value,
            roundedValue: Math.round(value),
            phase: progressState.phase,
          };
          const lastSample = observation.samples.at(-1);
          if (
            !lastSample ||
            lastSample.streamRunId !== sample.streamRunId ||
            lastSample.value !== sample.value ||
            lastSample.roundedValue !== sample.roundedValue ||
            lastSample.phase !== sample.phase
          ) {
            observation.samples.push(sample);
          }
        }
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

async function stopGraphProgressObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseGraphProgressObservation?: {
        stopped: boolean;
        samples: Array<{
          streamRunId: string;
          value: number;
          roundedValue: number;
          phase: string | null;
        }>;
      };
      __treeaseGraphProgressDebug?: Array<Record<string, unknown>>;
    };
    const observation = runtimeWindow.__treeaseGraphProgressObservation;
    if (!observation) return null;
    observation.stopped = true;
    return {
      ...observation,
      debug: runtimeWindow.__treeaseGraphProgressDebug ?? [],
    };
  });
}

async function installGraphImportStreamObservation(page: Page, baselineRenderCalls: number) {
  await page.evaluate((baseline) => {
    type GraphImportStreamObservation = {
      stopped: boolean;
      sessionId: string | null;
      firstPartialAtMs: number | null;
      baselineRenderCalls: number | null;
      firstPartialPhase: string | null;
      firstPartialActive: boolean | null;
      finalSeenAtMs: number | null;
      renderCallsAtFinal: number | null;
      maxRenderCallsAfterFinal: number | null;
      renderCallChanges: Array<{
        renderCalls: number;
        lastPhase: string | null;
        lastRenderTextLength: number | null;
        fullEditPhase: string | null;
        sourceTextLength: number;
        editorRevision: number;
        graphAppliedRevision: number;
      }>;
    };
    const runtimeWindow = window as Window & {
      _treease?: {
        editor?: {
          getState?: () => {
            fullEditUiState?: {
              active?: boolean;
              sessionId?: string | null;
              phase?: string | null;
              reason?: string | null;
            };
              sourceText?: string;
              editorRevision?: number;
              graphAppliedRevision?: number;
          };
        };
        graph?: {
          getStreamState?: () => {
            partialSeen?: boolean;
            finalSeen?: boolean;
            renderCalls?: number;
            revision?: number;
            documentKey?: string;
          } | null;
        };
      };
      __treeaseGraphImportStreamObservation?: GraphImportStreamObservation;
    };
    runtimeWindow.__treeaseGraphImportStreamObservation = {
      stopped: false,
      sessionId: null,
      firstPartialAtMs: null,
      firstPartialPhase: null,
      firstPartialActive: null,
      finalSeenAtMs: null,
      renderCallsAtFinal: null,
      baselineRenderCalls: baseline,
      maxRenderCallsAfterFinal: null,
      renderCallChanges: [],
    };
    const tick = () => {
      const observation = runtimeWindow.__treeaseGraphImportStreamObservation;
      if (!observation || observation.stopped) return;
      const editorState = runtimeWindow._treease?.editor?.getState?.() ?? null;
      const fullEdit = editorState?.fullEditUiState ?? null;
      const stream = runtimeWindow._treease?.graph?.getStreamState?.() ?? null;
      const activeSessionId = fullEdit?.active && fullEdit.reason === 'drop-file' ? fullEdit.sessionId : null;
      if (activeSessionId && !observation.sessionId) {
        observation.sessionId = activeSessionId;
      }
      if (observation.sessionId) {
        const renderCalls = Number(stream?.renderCalls ?? 0);
        const currentRunStarted =
          renderCalls > (observation.baselineRenderCalls ?? -1) &&
          stream?.revision === editorState?.editorRevision &&
          stream?.documentKey === editorState?.documentKey;
        if (currentRunStarted && stream?.partialSeen && observation.firstPartialAtMs == null) {
          observation.firstPartialAtMs = performance.now();
          observation.firstPartialPhase = fullEdit?.phase ?? null;
          observation.firstPartialActive = fullEdit?.active ?? false;
        }
        const lastRenderCalls = observation.renderCallChanges.at(-1)?.renderCalls ?? null;
        if (lastRenderCalls !== renderCalls) {
          observation.renderCallChanges.push({
            renderCalls,
            lastPhase: typeof stream?.lastPhase === 'string' ? stream.lastPhase : null,
            lastRenderTextLength:
              typeof stream?.lastRenderTextLength === 'number' ? stream.lastRenderTextLength : null,
            fullEditPhase: fullEdit?.phase ?? null,
            sourceTextLength: typeof editorState?.sourceText === 'string' ? editorState.sourceText.length : 0,
            editorRevision: typeof editorState?.editorRevision === 'number' ? editorState.editorRevision : 0,
            graphAppliedRevision:
              typeof editorState?.graphAppliedRevision === 'number' ? editorState.graphAppliedRevision : 0,
          });
        }
        if (currentRunStarted && stream?.finalSeen && observation.finalSeenAtMs == null) {
          observation.finalSeenAtMs = performance.now();
          observation.renderCallsAtFinal = renderCalls;
          observation.maxRenderCallsAfterFinal = renderCalls;
        }
        if (observation.finalSeenAtMs != null) {
          observation.maxRenderCallsAfterFinal = Math.max(observation.maxRenderCallsAfterFinal ?? 0, renderCalls);
        }
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }, baselineRenderCalls);
}

async function readGraphImportStreamObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseGraphImportStreamObservation?: {
        stopped: boolean;
        sessionId: string | null;
        firstPartialAtMs: number | null;
        firstPartialPhase: string | null;
        firstPartialActive: boolean | null;
        finalSeenAtMs: number | null;
        baselineRenderCalls: number | null;
        renderCallsAtFinal: number | null;
        maxRenderCallsAfterFinal: number | null;
        renderCallChanges?: Array<{
          renderCalls: number;
          lastPhase: string | null;
          lastRenderTextLength: number | null;
          fullEditPhase: string | null;
          sourceTextLength: number;
          editorRevision: number;
          graphAppliedRevision: number;
        }>;
      };
    };
    return runtimeWindow.__treeaseGraphImportStreamObservation ?? null;
  });
}

async function stopGraphImportStreamObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseGraphImportStreamObservation?: {
        stopped: boolean;
        sessionId: string | null;
        firstPartialAtMs: number | null;
        firstPartialPhase: string | null;
        firstPartialActive: boolean | null;
        finalSeenAtMs: number | null;
        renderCallsAtFinal: number | null;
        baselineRenderCalls: number | null;
        maxRenderCallsAfterFinal: number | null;
        renderCallChanges?: Array<{
          renderCalls: number;
          lastPhase: string | null;
          lastRenderTextLength: number | null;
          fullEditPhase: string | null;
          sourceTextLength: number;
          editorRevision: number;
          graphAppliedRevision: number;
        }>;
      };
    };
    const observation = runtimeWindow.__treeaseGraphImportStreamObservation;
    if (!observation) return null;
    observation.stopped = true;
    return observation;
  });
}

async function installGraphErrorObservation(page: Page) {
  await page.evaluate(() => {
    type GraphErrorObservation = {
      seen: boolean;
      stopped: boolean;
      firstSeen: {
        message: string;
        phase: string | null;
        sourceLength: number;
        editorRevision: number;
        graphAppliedRevision: number;
        elapsedMs: number;
      } | null;
    };
    const runtimeWindow = window as Window & {
      _treease?: {
        editor: {
          getState(): {
            sourceText: string;
            editorRevision: number;
            graphAppliedRevision: number;
            fullEditUiState?: { phase?: string | null } | null;
          };
        };
      };
      __treeaseGraphErrorObservation?: GraphErrorObservation;
    };
    const startedAt = performance.now();
    runtimeWindow.__treeaseGraphErrorObservation = {
      seen: false,
      stopped: false,
      firstSeen: null,
    };
    const tick = () => {
      const observation = runtimeWindow.__treeaseGraphErrorObservation;
      if (!observation || observation.stopped) return;
      const state = runtimeWindow._treease?.editor.getState();
      const message = (document.querySelector('[data-testid="graph-error-message"]')?.textContent ?? '').trim();
      if (message && state && observation.firstSeen == null) {
        observation.seen = true;
        observation.firstSeen = {
          message,
          phase: state.fullEditUiState?.phase ?? null,
          sourceLength: state.sourceText.length,
          editorRevision: state.editorRevision,
          graphAppliedRevision: state.graphAppliedRevision,
          elapsedMs: performance.now() - startedAt,
        };
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}


async function stopGraphErrorObservation(page: Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as Window & {
      __treeaseGraphErrorObservation?: {
        seen: boolean;
        stopped: boolean;
        firstSeen: {
          message: string;
          phase: string | null;
          sourceLength: number;
          editorRevision: number;
          graphAppliedRevision: number;
          elapsedMs: number;
        } | null;
      };
    };
    const observation = runtimeWindow.__treeaseGraphErrorObservation;
    if (!observation) return null;
    observation.stopped = true;
    return observation;
  });
}


async function openRightTextMode(page: Page) {
  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
}



test('dropping a medium json file onto source editor completes graph rebuild within budget', async ({ page }) => {
  const sourceText = buildLargeJsonText(220);

  await page.goto('/editor');
  await waitForEditorReady(page);

  await expectWithinBudget('source editor drop pipeline', SOURCE_DROP_BUDGET_MS, async () => {
    await dropFile(page, {
      targetTestId: 'source-editor-region',
      fileName: 'drop-source.json',
      content: sourceText,
      mimeType: 'application/json',
    });

    await expect.poll(async () => {
      const [storeText, modelText] = await Promise.all([
        (await readEditorState(page)).sourceText,
        getMonacoValue(page, 'source-editor'),
      ]);
      return storeText.length > 0 && modelText.length > 0;
    }, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(true);
    await waitForGraphRendered(page, SOURCE_DROP_BUDGET_MS);
    await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: SOURCE_DROP_BUDGET_MS }).toBeGreaterThan(0);
  });

  const state = await readEditorState(page);
  const revisions = await readGraphRevisions(page);
  expect(state.languageId).toBe('json');
  expect(revisions.editorRevision).toBeGreaterThan(0);
  expect(revisions.graphAppliedRevision).toBeGreaterThanOrEqual(revisions.editorRevision);
});

test('dropping a 5MB json file shows source text before import finishes', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const startedAt = Date.now();
  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '5MB-min.json',
    content: largeJsonFixtureText,
    mimeType: 'application/json',
  });

  let firstVisibleTextMs = -1;
  await expect.poll(async () => {
    const value = await getMonacoValue(page, 'source-editor');
    if (value.length > 0 && firstVisibleTextMs < 0) {
      firstVisibleTextMs = Date.now() - startedAt;
    }
    return value.length > 0;
  }, { timeout: LARGE_IMPORT_FIRST_VISIBLE_BUDGET_MS }).toBe(true);


  expect(firstVisibleTextMs, `first visible text exceeded budget: ${firstVisibleTextMs}ms > ${LARGE_IMPORT_FIRST_VISIBLE_BUDGET_MS}ms`).toBeGreaterThanOrEqual(0);
  expect(firstVisibleTextMs, `first visible text exceeded budget: ${firstVisibleTextMs}ms > ${LARGE_IMPORT_FIRST_VISIBLE_BUDGET_MS}ms`).toBeLessThanOrEqual(LARGE_IMPORT_FIRST_VISIBLE_BUDGET_MS);

  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: SOURCE_DROP_BUDGET_MS }).toBe(largeJsonFixtureText);
});
test('dropping the 1mb json fixture keeps graph progress monotonic', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphProgressObservation(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '1MB-min.json',
    content: oneMbMinJsonFixtureText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: SOURCE_DROP_BUDGET_MS }).toBe(oneMbMinJsonFixtureText);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(oneMbMinJsonFixtureText);
  await waitForGraphRendered(page, SOURCE_DROP_BUDGET_MS);
  const observation = await stopGraphProgressObservation(page);
  const streamRunId = observation?.samples.at(-1)?.streamRunId ?? '';
  const runSamples = (observation?.samples ?? []).filter((sample) => sample.streamRunId === streamRunId);
  const roundedSamples = runSamples.map((sample) => sample.roundedValue);
  const compressedRoundedSamples = roundedSamples.filter(
    (value, index) => index === 0 || value !== roundedSamples[index - 1],
  );
  const regressions = compressedRoundedSamples
    .map((value, index) => {
      if (index === 0) return null;
      const previous = compressedRoundedSamples[index - 1];
      return typeof previous === 'number' && value < previous ? `${previous}->${value}` : null;
    })
    .filter((value): value is string => value !== null);
  expect(runSamples.length).toBeGreaterThanOrEqual(0);
  expect(regressions, JSON.stringify(runSamples)).toEqual([]);
});


test('importing the 1mb json fixture via file input never lets an older progress stream reappear', async ({ page }) => {
  test.slow();
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphProgressObservation(page);

  await page.getByTestId('topbar-import-button').click();
  await page.getByLabel('Import file input').setInputFiles(oneMbMinJsonFixturePath);
  await waitForGraphRendered(page, 10_000);
  await expect.poll(async () => (await readImportStreamState(page)).phase, { timeout: 10_000 }).toBe('idle');

  const observation = await stopGraphProgressObservation(page);
  const runOrder = (observation?.samples ?? [])
    .map((sample) => sample.streamRunId)
    .filter((streamRunId) => streamRunId.length > 0)
    .filter((streamRunId, index, all) => index === 0 || streamRunId !== all[index - 1]);
  const resurrectedRuns = runOrder.filter((streamRunId, index) => runOrder.indexOf(streamRunId) !== index);

  expect(runOrder.length, JSON.stringify(observation?.debug ?? [])).toBeGreaterThan(0);
  expect(resurrectedRuns, JSON.stringify(observation?.samples ?? [])).toEqual([]);
});

test('dropping the 1mb json fixture never surfaces document analysis failed during import', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphErrorObservation(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '1MB-min.json',
    content: oneMbMinJsonFixtureText,
    mimeType: 'application/json',
  });

    await expect.poll(async () => {
      const [storeText, modelText] = await Promise.all([
        (await readEditorState(page)).sourceText,
        getMonacoValue(page, 'source-editor'),
      ]);
      return storeText.length > 0 && storeText === modelText;
    }, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(true);
  await waitForGraphRendered(page, SOURCE_DROP_BUDGET_MS);
  await page.waitForTimeout(250);

  const observation = await stopGraphErrorObservation(page);
  expect(observation).toEqual({
    seen: false,
    stopped: true,
    firstSeen: null,
  });
  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
});

test('dropping the 1mb json fixture does not render streamed graph cells on top of each other', async ({ page }) => {
  test.setTimeout(30_000);
  const client = await page.context().newCDPSession(page);
  await client.send('Emulation.setCPUThrottlingRate', { rate: 3 });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installStreamingGraphLayoutObservation(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '1MB-min.json',
    content: oneMbMinJsonFixtureText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: SOURCE_DROP_BUDGET_MS }).toBe(oneMbMinJsonFixtureText);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(oneMbMinJsonFixtureText);
  await waitForGraphRendered(page, 15_000);

  const layoutObservation = await stopStreamingGraphLayoutObservation(page);
  expect(layoutObservation?.samples ?? []).toEqual([]);
  const violations = await readGraphLayoutViolations(page);
  expect(violations).toEqual([]);
});

test('dropping the 2mb hover fixture streams the first graph frame before idle without a post-final rebuild', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const beforeRenderCalls = await readCommittedRenderCallCount(page);
  await installGraphImportStreamObservation(page, beforeRenderCalls);
  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '2mb.json',
    content: hoverPanelFixtureText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readImportStreamState(page)).phase, { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS }).toBe('idle');
  await expect
    .poll(async () => (await readGraphImportStreamObservation(page))?.finalSeenAtMs ?? null, {
      timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS,
    })
    .not.toBeNull();
  await expect
    .poll(
      async () => {
        const observation = await readGraphImportStreamObservation(page);
        if (!observation?.finalSeenAtMs) return null;
        return await page.evaluate((finalSeenAtMs) => {
          if (performance.now() - finalSeenAtMs < 250) return null;
          const runtimeWindow = window as Window & {
            __treeaseGraphImportStreamObservation?: {
              renderCallsAtFinal: number | null;
              maxRenderCallsAfterFinal: number | null;
            };
          };
          const current = runtimeWindow.__treeaseGraphImportStreamObservation;
          return {
            renderCallsAtFinal: current?.renderCallsAtFinal ?? null,
            maxRenderCallsAfterFinal: current?.maxRenderCallsAfterFinal ?? null,
          };
        }, observation.finalSeenAtMs);
      },
      { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS },
    )
    .not.toBeNull();
  await waitForGraphRendered(page, HOVER_FIXTURE_IMPORT_BUDGET_MS);
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS }).toBeGreaterThan(0);

  const observation = await stopGraphImportStreamObservation(page);
  expect(observation?.sessionId).toBeTruthy();
  expect(observation?.firstPartialAtMs).not.toBeNull();
  expect(observation?.firstPartialActive).toBe(true);
  expect(observation?.firstPartialPhase).not.toBe('idle');
  expect(observation?.finalSeenAtMs).not.toBeNull();
  expect(observation?.renderCallsAtFinal ?? 0).toBeGreaterThanOrEqual(beforeRenderCalls);
  expect(observation?.maxRenderCallsAfterFinal, JSON.stringify(observation?.renderCallChanges ?? [])).toBe(
    observation?.renderCallsAtFinal,
  );
});

test('dropping the 2mb hover fixture never leaks table child nodes during streaming', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installStreamingGraphLayoutObservation(page);
  await installStreamingGraphTableLeakObservation(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '2mb.json',
    content: hoverPanelFixtureText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readImportStreamState(page)).phase, { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS }).toBe('idle');
  await waitForGraphRendered(page, HOVER_FIXTURE_IMPORT_BUDGET_MS);

  const layoutObservation = await stopStreamingGraphLayoutObservation(page);
  expect(layoutObservation?.samples ?? []).toEqual([]);
  const violations = await readGraphLayoutViolations(page);
  expect(violations).toEqual([]);
  const leakObservation = await stopStreamingGraphTableLeakObservation(page);
  expect(leakObservation?.samples ?? []).toEqual([]);
  const finalLeaks = await readGraphTableChildLeaks(page);
  expect(finalLeaks).toEqual([]);
});

test('dropping the 2mb hover fixture keeps cursor path and graph selection after import settles', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '2mb.json',
    content: hoverPanelFixtureText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readImportStreamState(page)).phase, { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS }).toBe('idle');

  await setMonacoPositionByText(page, 'source-editor', '"Id":');

  const expectedPath = ['$', 'Result', 'Blocks', '[0]', 'Id'];
  await expect(page.getByRole('link', { name: 'Tree path Result.Blocks.0.Id', exact: true })).toBeVisible({
    timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS,
  });
  await expect
    .poll(async () => await readTempGraphSelection(page), { timeout: HOVER_FIXTURE_IMPORT_BUDGET_MS })
    .toEqual(expect.objectContaining({ path: expectedPath, target: 'key', source: 'editor' }));
});

test('dropping a medium json file onto compare panel loads compare text without touching source state', async ({ page }) => {
  const sourceText = buildLargeJsonText(220);
  const compareText = buildLargeJsonText(180);

  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'baseline-source.json',
    content: sourceText,
    mimeType: 'application/json',
  });
  await waitForGraphRendered(page, SOURCE_DROP_BUDGET_MS);
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: SOURCE_DROP_BUDGET_MS }).toBeGreaterThan(0);

  const beforeDrop = await readEditorState(page);
  const beforeRevisions = await readGraphRevisions(page);
  await openRightTextMode(page);

  await expectWithinBudget('right panel drop pipeline', SOURCE_DROP_BUDGET_MS, async () => {
    await dropFile(page, {
      targetTestId: 'right-panel-dropzone',
      fileName: 'drop-compare.json',
      content: compareText,
      mimeType: 'application/json',
    });

    await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: SOURCE_DROP_BUDGET_MS }).toBe(compareText);
    await expect.poll(async () => (await readEditorState(page)).tempModel.scratchText, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(compareText);
  });

  const afterRevisions = await readGraphRevisions(page);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(beforeDrop.sourceText);
  await expect
    .poll(async () => JSON.stringify(JSON.parse(await getMonacoValue(page, 'source-editor'))), { timeout: 5_000 })
    .toBe(JSON.stringify(JSON.parse(beforeDrop.sourceText)));
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe(beforeDrop.languageId);
  expect(afterRevisions.editorRevision).toBe(beforeRevisions.editorRevision);
  expect(afterRevisions.graphAppliedRevision).toBe(beforeRevisions.graphAppliedRevision);
});

test('dropping rows json rebuilds graph from imported rows', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'rows.json',
    content: smallImportedRowsText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => {
    const [storeText, modelText] = await Promise.all([
      (await readEditorState(page)).sourceText,
      getMonacoValue(page, 'source-editor'),
    ]);
    if (storeText.length === 0 || storeText !== modelText) return false;
    return JSON.stringify(JSON.parse(storeText)) === JSON.stringify(JSON.parse(smallImportedRowsText));
  }, { timeout: SOURCE_DROP_BUDGET_MS }).toBe(true);
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: SOURCE_DROP_BUDGET_MS }).toBe('json');
  await waitForGraphRendered(page, 10_000);
  await expect
    .poll(
      async () => readGraphValueTextsByPath(page, ['[0].name', '[0].language', '[1].id']),
      { timeout: 10_000 },
    )
    .toEqual({
      '[0].name': [oneMbMinJsonRows[0]!.name],
      '[0].language': [oneMbMinJsonRows[0]!.language],
      '[1].id': [oneMbMinJsonRows[1]!.id],
    });
});
