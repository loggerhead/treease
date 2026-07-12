import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures';
import {
  dropFile,
  getMonacoValue,
  readEditorState,
  setEditorContent,
  setMonacoPositionByText,
  waitForEditorReady,
  waitForGraphRendered,
  waitForImportSettled,
} from './utils';

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
    const resolveEdgeAnchorY = (node: any, rowIndex: number) => {
      if (node?.kind === 'table' && node?.table) {
        const headerOffset = (node.table.headerHeight ?? 0) > 0 ? 1 : 0;
        if (headerOffset === 1 && rowIndex === 0) {
          return Number(node.boxArgs?.y ?? 0) + Number(node.table.headerHeight ?? 0) / 2;
        }
        const bodyIndex = rowIndex - headerOffset;
        const row = bodyIndex >= 0 ? node.table.rows?.[bodyIndex] : undefined;
        return row ? centerY(row.boxArgs, node) : null;
      }
      const row = node?.rows?.[rowIndex];
      return row ? centerY(row.boxArgs, node) : null;
    };

    for (const edge of edges) {
      const parent = nodeByHandle.get(Number(edge.fromRenderHandle));
      const child = nodeByHandle.get(Number(edge.toRenderHandle));
      if (!parent || !child) continue;
      const expectedFromX = Number(parent.boxArgs?.x ?? 0) + Number(parent.boxArgs?.width ?? 0);
      const expectedToX = Number(child.boxArgs?.x ?? 0);
      const expectedFromY = resolveEdgeAnchorY(parent, Number(edge.fromRow));
      const expectedToY = resolveEdgeAnchorY(child, Number(edge.toRow));
      if (expectedFromY == null || expectedToY == null) continue;
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
  await waitForImportSettled(page, 15_000);
  const [modelText, state] = await Promise.all([
    getMonacoValue(page, 'source-editor'),
    readEditorState(page),
  ]);
  expect(state.sourceText).toBe(modelText);
  await waitForGraphRendered(page, 15_000);

  const mismatches = await readEdgeAlignmentMismatches(page);
  const edgeLayer = await readEdgeLayerSnapshot(page);
  expect(mismatches).toEqual([]);
  expect(edgeLayer.layerChildCount).toBe(edgeLayer.graphEdgeCount);
});
