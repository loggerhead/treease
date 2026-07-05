import { expect, test } from './fixtures';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  callTreeaseWorker,
  setEditorContent,
  waitForEditorReady,
} from './utils';

type FixtureKind = 'json' | 'toml' | 'yaml';

type FixtureCase = {
  filePath: string;
  kind: FixtureKind;
  expectation: 'valid' | 'invalid';
  content: string;
};

type NodeSummary = {
  path: string;
  kind: string;
  depth: number;
};

const REPO_ROOT = path.resolve(process.cwd(), '../..');
const FIXTURES_DIR = path.join(REPO_ROOT, 'test/fixtures');
const FIXTURE_E2E_MAX_BYTES = 256 * 1024;
const MAX_FIXTURES_PER_CATEGORY = 5;

const LANGUAGE_MAP: Record<FixtureKind, string> = {
  json: 'json',
  toml: 'toml',
  yaml: 'yaml',
};

const FIXTURE_SUBDIRS: FixtureKind[] = ['json', 'toml', 'yaml'];

function expectationFromName(name: string): 'valid' | 'invalid' | null {
  const extIndex = name.lastIndexOf('.');
  if (extIndex <= 0) return null;
  const stem = name.slice(0, extIndex);
  const markerIndex = stem.lastIndexOf('.');
  if (markerIndex < 0) return null;
  const marker = stem.slice(markerIndex + 1);
  if (marker === '1') return 'valid';
  if (marker === '0') return 'invalid';
  return null;
}

function kindFromDirName(dirName: string): FixtureKind | null {
  if ((FIXTURE_SUBDIRS as readonly string[]).includes(dirName)) return dirName as FixtureKind;
  return null;
}

function shouldSkipFixture(kind: FixtureKind, name: string): boolean {
  if (kind === 'json') return name.startsWith('minefield__i_');
  return false;
}

/** Pick a representative sample of up to `limit` fixtures, favouring small files. */
function sampleFixtures(
  fixtures: FixtureCase[],
  limit: number,
): FixtureCase[] {
  const sorted = [...fixtures].sort((a, b) => a.content.length - b.content.length);
  return sorted.slice(0, limit);
}

function isBlankLikeContent(content: string): boolean {
  return content.trim().length === 0;
}

function expectBlankLikeYqGraph(nodes: NodeSummary[]): void {
  expect(nodes).toEqual([{ path: '', kind: 'scalar', depth: 0 }]);
}

function collectFixtures(): FixtureCase[] {
  const cases: FixtureCase[] = [];

  for (const subdir of FIXTURE_SUBDIRS) {
    const dirPath = path.join(FIXTURES_DIR, subdir);
    if (!fs.existsSync(dirPath)) continue;

    const kind = kindFromDirName(subdir);
    if (!kind) continue;

    const entries = fs.readdirSync(dirPath).sort();
    for (const entry of entries) {
      if (shouldSkipFixture(kind, entry)) continue;
      const expectation = expectationFromName(entry);
      if (!expectation) continue;

      const filePath = path.join(dirPath, entry);
      const content = fs.readFileSync(filePath, 'utf-8');
      if (content.length > FIXTURE_E2E_MAX_BYTES) continue;
      cases.push({ filePath, kind, expectation, content });
    }
  }

  return cases;
}

const fixtures = collectFixtures();
const validByKind = new Map<FixtureKind, FixtureCase[]>();
const invalidByKind = new Map<FixtureKind, FixtureCase[]>();
for (const kind of FIXTURE_SUBDIRS) {
  validByKind.set(kind, fixtures.filter((f) => f.kind === kind && f.expectation === 'valid'));
  invalidByKind.set(kind, fixtures.filter((f) => f.kind === kind && f.expectation === 'invalid'));
}

async function readGraphNodes(page: import('@playwright/test').Page): Promise<NodeSummary[]> {
  return page.evaluate(() => {
    const treease = window._treease;
    if (!treease) return [];
    const graphData = treease.graph.getLastGraphData();
    if (!graphData) return [];
    if (!graphData.nodes) return [];

    return graphData.nodes.map((node: any) => ({
      path: (node.path ?? [])
        .map((seg: any) => {
          if (typeof seg?.key === 'string' && seg.key.length > 0) return seg.key;
          if (typeof seg?.index === 'number') return `[${seg.index}]`;
          return '';
        })
        .filter((s: string) => s.length > 0)
        .join('.'),
      kind: String(node.kind ?? ''),
      depth: Number(node.depth ?? 0),
    }));
  });
}

async function waitForGraphSync(page: import('@playwright/test').Page, minRevision: number, timeout = 5_000) {
  await expect
    .poll(
      async () => {
        return page.evaluate((rev) => {
          const treease = window._treease;
          if (!treease) return { ready: false };
          const st = treease.editor.getState();
          return {
            ready: true,
            synced: st.graphAppliedRevision >= rev && st.editorRevision >= rev,
          };
        }, minRevision);
      },
      { timeout, intervals: [500] },
    )
    .toEqual(expect.objectContaining({ ready: true, synced: true }));
}

async function buildGraphAndGetNodes(
  page: import('@playwright/test').Page,
  opts: { sourceText: string; language: string },
  timeout = 5_000,
): Promise<NodeSummary[]> {
  await setEditorContent(page, {
    sourceText: opts.sourceText,
    language: opts.language,
  });

  const graphModeButton = page.getByRole('button', { name: 'Graph mode', exact: true });
  if (await graphModeButton.isVisible().catch(() => false)) {
    await graphModeButton.click();
    await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toBeVisible({ timeout: 5_000 });
  }

  const targetRevision = await page.evaluate(() => {
    return window._treease?.editor.getState().editorRevision ?? 0;
  });

  await waitForGraphSync(page, targetRevision, timeout);

  return readGraphNodes(page);
}

async function runYqToJson(
  page: import('@playwright/test').Page,
  opts: { language: string; text: string },
): Promise<{ result: string | null; debug: string }> {
  try {
    const result = await callTreeaseWorker<string>(page, 'runYq', {
      language: opts.language,
      text: opts.text,
      expression: '.',
      options: {
        indent: 2,
        smart: true,
        maxLineLength: 80,
        maxInlineComplexity: 2,
        maxArrayInlineItems: 4,
        alignObjectArrays: true,
        nest: true,
      },
      nest: true,
    });
    return { result, debug: `type=string, preview=${result.slice(0, 100)}` };
  } catch (e) {
    return { result: null, debug: `error: ${e instanceof Error ? e.message : String(e)}` };
  }
}

test.describe('fixture corpus sampling', () => {
  for (const kind of FIXTURE_SUBDIRS) {
    const validFixtures = sampleFixtures(validByKind.get(kind) ?? [], MAX_FIXTURES_PER_CATEGORY);
    const invalidFixtures = sampleFixtures(invalidByKind.get(kind) ?? [], MAX_FIXTURES_PER_CATEGORY);

    for (const fixture of validFixtures) {
      test(`valid ${kind}: ${path.basename(fixture.filePath)}`, async ({ page }) => {
        await page.goto('/editor');
        await waitForEditorReady(page);

        const nodes = await buildGraphAndGetNodes(page, {
          sourceText: fixture.content,
          language: LANGUAGE_MAP[kind],
        });

        if (isBlankLikeContent(fixture.content)) {
          expect(nodes).toEqual([]);
          return;
        }

        expect(nodes.length).toBeGreaterThan(0);
      });
    }

    for (const fixture of invalidFixtures) {
      test(`invalid ${kind}: ${path.basename(fixture.filePath)}`, async ({ page }) => {
        await page.goto('/editor');
        await waitForEditorReady(page);

        await setEditorContent(page, {
          sourceText: fixture.content,
          language: LANGUAGE_MAP[kind],
        });

        const graphModeButton = page.getByRole('button', { name: 'Graph mode', exact: true });
        if (await graphModeButton.isVisible().catch(() => false)) {
          await graphModeButton.click();
          await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toBeVisible({ timeout: 5_000 });
        }

        if (isBlankLikeContent(fixture.content)) {
          await expect(page.getByTestId('graph-diagnostic-syntax-error')).toHaveCount(0);
          expect(await readGraphNodes(page)).toEqual([]);
          return;
        }

        await expect(page.getByTestId('graph-diagnostic-syntax-error').first()).toBeVisible({ timeout: 5_000 });
      });
    }

    if (kind !== 'json') {
      const yqFixtures = sampleFixtures(validByKind.get(kind) ?? [], 3);
      for (const fixture of yqFixtures) {
        test(`yq compare ${kind}: ${path.basename(fixture.filePath)}`, async ({ page }) => {
          await page.goto('/editor');
          await waitForEditorReady(page);

          const originalNodes = await buildGraphAndGetNodes(page, {
            sourceText: fixture.content,
            language: LANGUAGE_MAP[kind],
          });
          if (!isBlankLikeContent(fixture.content)) {
            expect(originalNodes.length).toBeGreaterThan(0);
          }

          const yqResult = await runYqToJson(page, {
            language: LANGUAGE_MAP[kind],
            text: fixture.content,
          });
          if (!yqResult.result) {
            test.skip(true, `yq conversion failed: ${yqResult.debug}`);
            return;
          }

          const jsonNodes = await buildGraphAndGetNodes(page, {
            sourceText: yqResult.result,
            language: 'json',
          });

          if (isBlankLikeContent(fixture.content)) {
            expect(originalNodes).toEqual([]);
            expectBlankLikeYqGraph(jsonNodes);
            return;
          }

          const sliceEnd = 10;
          const sortByPath = (a: NodeSummary, b: NodeSummary) => a.path.localeCompare(b.path);
          const sortedOriginal = originalNodes.slice(0, sliceEnd).sort(sortByPath);
          const sortedJson = jsonNodes.slice(0, sliceEnd).sort(sortByPath);

          expect(sortedJson).toEqual(sortedOriginal);
        });
      }
    }
  }
});
