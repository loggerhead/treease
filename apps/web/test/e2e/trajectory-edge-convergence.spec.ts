import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures';
import { clickGraphProbeAt, dropFile, getMonacoValue, readEditorState, readGraphClickProbes, setEditorContent, setMonacoPositionByText, waitForEditorReady, waitForGraphRendered } from './utils';

test.setTimeout(20_000);

const TRAJECTORY_FIXTURE_TEXT = readFileSync(
  new URL('../../../../test/fixtures/json/trajectory.1.json', import.meta.url),
  'utf8',
);

type EdgeAlignmentMismatch = {
  edgeKey: string;
  kind: 'fromX' | 'fromY' | 'toX' | 'toY';
  actual: number;
  expected: number;
  diff: number;
};

type EdgeLayerSnapshot = {
  graphEdgeCount: number;
  layerChildCount: number;
};

async function readEdgeAlignmentMismatches(page: import('@playwright/test').Page): Promise<EdgeAlignmentMismatch[]> {
  return page.evaluate(() => {
    const runtime = window._treease?.graph as any;
    const graph = runtime?.getLastGraphData?.() as { nodes?: any[]; edges?: any[] } | undefined;
    if (!graph) throw new Error('graph data unavailable');
    const nodes = Array.isArray(graph.nodes) ? graph.nodes : [];
    const edges = Array.isArray(graph.edges) ? graph.edges : [];
    const nodeByHandle = new Map(nodes.map((node: any) => [Number(node.renderHandle), node]));
    const mismatches: EdgeAlignmentMismatch[] = [];
    const centerY = (box: any, owner?: any) => {
      const localY = Number(box?.y ?? 0);
      const ownerY = Number(owner?.boxArgs?.y ?? 0);
      const resolvedY = localY < ownerY ? localY + ownerY : localY;
      return resolvedY + Number(box?.height ?? 0) / 2;
    };

    for (const edge of edges) {
      const parent = nodeByHandle.get(Number(edge.fromRenderHandle));
      const child = nodeByHandle.get(Number(edge.toRenderHandle));
      if (!parent || !child) continue;
      const parentRow = Array.isArray(parent.rows)
        ? parent.rows[Number(edge.fromRow)]
        : Array.isArray(parent.table?.rows)
          ? parent.table.rows[Number(edge.fromRow) - ((parent.table?.headerHeight ?? 0) > 0 ? 1 : 0)]
          : null;
      const childRow = Array.isArray(child.rows)
        ? child.rows[0]
        : Array.isArray(child.table?.rows)
          ? child.table.rows[0]
          : null;
      if (!parentRow || !childRow) continue;
      const expectedFromX = Number(parent.boxArgs?.x ?? 0) + Number(parent.boxArgs?.width ?? 0);
      const expectedToX = Number(child.boxArgs?.x ?? 0);
      const expectedFromY = centerY(parentRow.boxArgs, parent);
      const expectedToY = centerY(childRow.boxArgs, child);
      const actualFromX = Number(edge.bezierArgs?.fromX ?? NaN);
      const actualFromY = Number(edge.bezierArgs?.fromY ?? NaN);
      const actualToX = Number(edge.bezierArgs?.toX ?? NaN);
      const actualToY = Number(edge.bezierArgs?.toY ?? NaN);
      const edgeKey = `${edge.fromRenderHandle}:${edge.fromRow}->${edge.toRenderHandle}:${edge.toRow}`;
      const fromXDiff = Math.abs(actualFromX - expectedFromX);
      const fromDiff = Math.abs(actualFromY - expectedFromY);
      const toXDiff = Math.abs(actualToX - expectedToX);
      const toDiff = Math.abs(actualToY - expectedToY);
      if (fromXDiff > 0.5) {
        mismatches.push({ edgeKey, kind: 'fromX', actual: actualFromX, expected: expectedFromX, diff: fromXDiff });
      }
      if (fromDiff > 0.5) {
        mismatches.push({ edgeKey, kind: 'fromY', actual: actualFromY, expected: expectedFromY, diff: fromDiff });
      }
      if (toXDiff > 0.5) {
        mismatches.push({ edgeKey, kind: 'toX', actual: actualToX, expected: expectedToX, diff: toXDiff });
      }
      if (toDiff > 0.5) {
        mismatches.push({ edgeKey, kind: 'toY', actual: actualToY, expected: expectedToY, diff: toDiff });
      }
    }
    return mismatches.slice(0, 50);
  });
}

async function readEdgeLayerSnapshot(page: import('@playwright/test').Page): Promise<EdgeLayerSnapshot> {
  return page.evaluate(() => {
    const runtime = window._treease?.graph as any;
    const graph = runtime?.getLastGraphData?.() as { edges?: any[] } | undefined;
    const edgeLayer = runtime?.refs?.layers?.edgeLayer;
    const children = Array.isArray(edgeLayer?.children) ? edgeLayer.children : [];
    return {
      graphEdgeCount: Array.isArray(graph?.edges) ? graph.edges.length : 0,
      layerChildCount: children.length,
    };
  });
}

test('trajectory fixture final graph edges converge to the current node layout', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: TRAJECTORY_FIXTURE_TEXT,
    language: 'json',
  });
  await waitForGraphRendered(page, 15_000);
  await setMonacoPositionByText(page, 'source-editor', '"llm_duration"');
  await waitForGraphRendered(page, 15_000);

  const mismatches = await readEdgeAlignmentMismatches(page);
  const edgeLayer = await readEdgeLayerSnapshot(page);
  expect(mismatches).toEqual([]);
  expect(edgeLayer.layerChildCount).toBe(edgeLayer.graphEdgeCount);
});

test('trajectory fixture imported through source drop also converges rendered edges', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'trajectory.json',
    content: TRAJECTORY_FIXTURE_TEXT,
    mimeType: 'application/json',
  });
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 15_000 }).toBe(TRAJECTORY_FIXTURE_TEXT);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 15_000 }).toBe(TRAJECTORY_FIXTURE_TEXT);
  await waitForGraphRendered(page, 15_000);

  const mismatches = await readEdgeAlignmentMismatches(page);
  const edgeLayer = await readEdgeLayerSnapshot(page);
  expect(mismatches).toEqual([]);
  expect(edgeLayer.layerChildCount).toBe(edgeLayer.graphEdgeCount);
});

test('trajectory fixture subgraph workspace keeps pane-sized viewport but preserves scale-1 default rendering', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: TRAJECTORY_FIXTURE_TEXT,
    language: 'json',
  });
  await waitForGraphRendered(page, 15_000);

  await page.evaluate(() => {
    window._treease?.graph?.revealPath?.(
      [
        { tag: 0, key: 'agent_steps', index: 0 },
        { tag: 1, key: '', index: 0 },
        { tag: 0, key: 'steps', index: 0 },
      ],
      { target: 'value', navigate: true },
    );
  });

  const probes = await readGraphClickProbes(page);
  const targetProbe = probes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'agent_steps.[0].steps' && probe.coord,
  );
  expect(targetProbe, JSON.stringify(probes.slice(0, 20), null, 2)).toBeTruthy();
  if (!targetProbe?.coord) throw new Error('trajectory subgraph probe missing coord');

  await clickGraphProbeAt(page, targetProbe.coord);

  const pane = page.getByTestId('graph-subgraph-pane');
  await expect(pane).toBeVisible({ timeout: 5_000 });
  await expect(pane.locator('.graph-subgraph-pane__header')).toHaveText('agent_steps[0].steps');

  const metrics = await pane.evaluate((node) => {
    const canvasHost = node.querySelector('.graph-subgraph-pane__canvas') as HTMLDivElement | null;
    const paneView = node.querySelector('.graph-subgraph-pane-view') as HTMLDivElement | null;
    if (!canvasHost || !paneView) {
      return null;
    }
    return {
      clientWidth: canvasHost.clientWidth,
      clientHeight: canvasHost.clientHeight,
      scrollWidth: canvasHost.scrollWidth,
      scrollHeight: canvasHost.scrollHeight,
      viewWidth: paneView.clientWidth,
      viewHeight: paneView.clientHeight,
      viewportScale: canvasHost.dataset.viewportScale ?? '',
    };
  });
  expect(metrics).not.toBeNull();
  if (!metrics) throw new Error('trajectory subgraph pane metrics missing');
  expect(metrics.viewportScale).toBe('1');
  expect(metrics.viewWidth).toBe(metrics.clientWidth);
  expect(metrics.viewHeight).toBe(metrics.clientHeight);
  expect(metrics.scrollWidth).toBeLessThanOrEqual(metrics.clientWidth * 2);
  expect(metrics.scrollHeight).toBeLessThanOrEqual(metrics.clientHeight * 2);
});
