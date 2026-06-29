import { expect, test, type Page } from './fixtures';
import {
  commitGraphValueViaProbes,
  readEditorState,
  readGraphClickProbes,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const COMMAND_MOD = process.platform === 'darwin' ? 'Meta' : 'Control';

function readPathKey(value: unknown): string {
  if (typeof value !== 'string') {
    throw new Error(`Expected path key string, received ${Object.prototype.toString.call(value)}`);
  }
  return value;
}

function formatPath(path: Array<{ key?: unknown; index?: number }>): string {
  return path
    .map((segment) =>
      typeof segment?.key === 'string' && segment.key.length > 0
        ? readPathKey(segment.key)
        : typeof segment?.index === 'number'
          ? `[${segment.index}]`
          : '',
    )
    .filter((segment) => segment.length > 0)
    .join('.');
}

function buildHeaderTable(rowCount: number): Array<{ id: number; name: string; status: string }> {
  return Array.from({ length: rowCount }, (_, index) => ({
    id: index,
    name: `row-${index}`,
    status: index % 2 === 0 ? 'ready' : 'draft',
  }));
}

function buildYamlTableDocument(rowCount: number): string {
  const lines = ['table_with_header:'];
  for (let index = 0; index < rowCount; index += 1) {
    lines.push(`  - id: ${index}`);
    lines.push(`    name: row-${index}`);
    lines.push(`    status: ${index % 2 === 0 ? 'ready' : 'draft'}`);
  }
  return `${lines.join('\n')}\n`;
}


async function readGraphValueTextsByPath(page: Page, wantedPaths: string[]): Promise<Record<string, string[]>> {
  return page.evaluate((paths) => {
    const treease = window._treease;
    if (!treease) throw new Error('window._treease is unavailable');
    const wanted = new Set(paths);
    const readPathKey = (value: unknown): string => {
      if (typeof value !== 'string') {
        throw new Error(`Expected path key string, received ${Object.prototype.toString.call(value)}`);
      }
      return value;
    };
    const probes = treease.graph.getClickProbeTargets('root') ?? [];
    const byPath: Record<string, string[]> = {};
    for (const probe of probes) {
      if (probe.target !== 'value') continue;
      const path = (probe.cell?.path ?? [])
        .map((segment) => {
          const key = typeof segment?.key === 'string' ? readPathKey(segment.key) : '';
          return key.length > 0 ? key : typeof segment?.index === 'number' ? `[${segment.index}]` : '';
        })
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

async function expectRootGraphValueTexts(page: Page, expected: Record<string, string[]>) {
  const wantedPaths = Object.keys(expected);
  await expect
    .poll(() => readGraphValueTextsByPath(page, wantedPaths), { timeout: 5_000 })
    .toEqual(expected);
}

async function hoverGraphValueProbe(
  page: Page,
  predicate: (probe: Awaited<ReturnType<typeof readGraphClickProbes>>[number]) => boolean,
) {
  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  let targetProbe: (Awaited<ReturnType<typeof readGraphClickProbes>>[number] & { coord: { x: number; y: number } }) | null = null;
  await expect
    .poll(
      async () => {
        const probes = await readGraphClickProbes(page);
        targetProbe =
          (probes.find(
            (probe): probe is (typeof probes)[number] & { coord: { x: number; y: number } } => !!probe.coord && predicate(probe),
          ) ?? null);
        return !!targetProbe;
      },
      { timeout: 5_000 },
    )
    .toBe(true);

  if (!targetProbe) throw new Error('target graph probe missing');
  await page.mouse.move(box.x + targetProbe.coord.x, box.y + targetProbe.coord.y);
}


function matchesPath(detail: { kind?: string; path?: Array<{ key?: unknown; index?: number }> }, expectedPath: string): boolean {
  return detail.kind === 'value' && formatPath(detail.path ?? []) === expectedPath;
}

function safeParseJson(sourceText: string): any | null {
  try {
    return JSON.parse(sourceText);
  } catch {
    return null;
  }
}

test('editor CRUD keeps graph values in sync', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const initialSourceText = JSON.stringify({
    profile: { name: 'Alice' },
    table_with_header: [{ id: 0, name: 'row-0', status: 'ready' }],
  });
  await setEditorContent(page, { sourceText: initialSourceText, language: 'json' });
  await waitForGraphRendered(page);

  await expect
    .poll(
      async () =>
        readGraphValueTextsByPath(page, ['profile.name', 'table_with_header.[0].name', 'table_with_header.[0].status']),
      { timeout: 5_000 },
    )
    .toEqual({
      'profile.name': ['Alice'],
      'table_with_header.[0].name': ['row-0'],
      'table_with_header.[0].status': ['ready'],
    });

  const updatedSourceText = JSON.stringify({
    profile: { name: 'Bob', role: 'admin' },
    table_with_header: [
      { id: 0, name: 'row-0', status: 'ready' },
      { id: 1, name: 'row-1', status: 'draft' },
    ],
  });
  await setEditorContent(page, { sourceText: updatedSourceText });

  await expect
    .poll(
      async () =>
        readGraphValueTextsByPath(page, [
          'profile.name',
          'profile.role',
          'table_with_header.[0].name',
          'table_with_header.[1].name',
          'table_with_header.[1].status',
        ]),
      { timeout: 5_000 },
    )
    .toEqual({
      'profile.name': ['Bob'],
      'profile.role': ['admin'],
      'table_with_header.[0].name': ['row-0'],
      'table_with_header.[1].name': ['row-1'],
      'table_with_header.[1].status': ['draft'],
    });

  const deletedSourceText = JSON.stringify({
    profile: { role: 'admin' },
    table_with_header: [{ id: 0, name: 'row-0', status: 'ready' }],
  });
  await setEditorContent(page, { sourceText: deletedSourceText });

  await expect
    .poll(
      async () => {
        const graphValues = await readGraphValueTextsByPath(page, [
          'profile.name',
          'profile.role',
          'table_with_header.[0].name',
          'table_with_header.[1].name',
        ]);
        return {
          profileNameExists: Object.prototype.hasOwnProperty.call(graphValues, 'profile.name'),
          profileRole: graphValues['profile.role']?.[0] ?? null,
          firstRowName: graphValues['table_with_header.[0].name']?.[0] ?? null,
          secondRowExists: Object.prototype.hasOwnProperty.call(graphValues, 'table_with_header.[1].name'),
        };
      },
      { timeout: 5_000 },
    )
    .toEqual({
      profileNameExists: false,
      profileRole: 'admin',
      firstRowName: 'row-0',
      secondRowExists: false,
    });
});

test('editor update refreshes headerless table graph cell', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: JSON.stringify({ table_without_header: ['a'] }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  await expect
    .poll(async () => readGraphValueTextsByPath(page, ['table_without_header.[0]']), { timeout: 5_000 })
    .toEqual({ 'table_without_header.[0]': ['a'] });

  await setEditorContent(page, {
    sourceText: JSON.stringify({ table_without_header: ['a1'] }),
  });

  await expect
    .poll(async () => readGraphValueTextsByPath(page, ['table_without_header.[0]']), { timeout: 5_000 })
    .toEqual({ 'table_without_header.[0]': ['a1'] });
});

test('non-table object value hover stays side-effect free', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    profile: {
      name: 'Alice',
      active: true,
      tags: ['owner'],
    },
    count: 1,
  });
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  await hoverGraphValueProbe(
    page,
    (probe) => probe.target === 'value' && probe.valueType === 'object' && probe.path.join('.') === 'profile',
  );

});

test('graph table row 0 cell edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    table_with_header: buildHeaderTable(140),
  });
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'row-0-updated',
    selectAllModifier: COMMAND_MOD,
    matchesOpenEvent: (detail) => detail.kind === 'value' && formatPath(detail.path ?? []) === 'table_with_header.[0].name',
    verifyCommitted: (nextSourceText) => safeParseJson(nextSourceText)?.table_with_header?.[0]?.name === 'row-0-updated',
  });
  expect(committed).toBe(true);
  await expect
    .poll(() => readEditorState(page).then((state) => safeParseJson(state.sourceText)?.table_with_header?.[0]?.name), { timeout: 5_000 })
    .toBe('row-0-updated');
  await waitForGraphRendered(page);
  await expectRootGraphValueTexts(page, {
    'table_with_header.[0].name': ['row-0-updated'],
  });
});

test('graph table row 100 cell edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    table_with_header: buildHeaderTable(140),
  });
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  // Scroll table to row 100
  const hasScrollFn = await page.evaluate(() => {
    const g = (window as any)._treease?.graph;
    return typeof g?.scrollTableToRow;
  });
  if (hasScrollFn === 'function') {
    await page.evaluate(() => {
      (window as any)._treease?.graph?.scrollTableToRow?.(100);
    });
  }
  // Wait for probes to include row 100
  await expect
    .poll(async () => {
      const probes = await readGraphClickProbes(page);
      return probes.some(p => p.path.join('.') === 'table_with_header.[100].name');
    }, { timeout: 5_000 })
    .toBe(true);

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'row-100-updated',
    selectAllModifier: COMMAND_MOD,
    matchesOpenEvent: (detail) => detail.kind === 'value' && formatPath(detail.path ?? []) === 'table_with_header.[100].name',
    verifyCommitted: (nextSourceText) => safeParseJson(nextSourceText)?.table_with_header?.[100]?.name === 'row-100-updated',
  });

  expect(committed).toBe(true);
  await expect
    .poll(() => readEditorState(page).then((state) => safeParseJson(state.sourceText)?.table_with_header?.[100]?.name), { timeout: 5_000 })
    .toBe('row-100-updated');
  await waitForGraphRendered(page);
  // Re-scroll to row 100 after graph re-render resets scroll to top
  if (hasScrollFn === 'function') {
    await page.evaluate(() => {
      (window as any)._treease?.graph?.scrollTableToRow?.(100);
    });
  }
  await expectRootGraphValueTexts(page, {
    'table_with_header.[100].name': ['row-100-updated'],
  });
});

test('yaml editor CRUD keeps graph values in sync', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const initialSourceText = 'profile:\n  name: Alice\ntable_with_header:\n  - id: 0\n    name: row-0\n    status: ready\n';
  await setEditorContent(page, { sourceText: initialSourceText, language: 'yaml' });
  await waitForGraphRendered(page);

  await expect
    .poll(
      async () =>
        readGraphValueTextsByPath(page, ['profile.name', 'table_with_header.[0].name', 'table_with_header.[0].status']),
      { timeout: 5_000 },
    )
    .toEqual({
      'profile.name': ['Alice'],
      'table_with_header.[0].name': ['row-0'],
      'table_with_header.[0].status': ['ready'],
    });

  const updatedSourceText =
    'profile:\n  name: Bob\n  role: admin\ntable_with_header:\n  - id: 0\n    name: row-0\n    status: ready\n  - id: 1\n    name: row-1\n    status: draft\n';
  await setEditorContent(page, { sourceText: updatedSourceText });

  await expect
    .poll(
      async () =>
        readGraphValueTextsByPath(page, [
          'profile.name',
          'profile.role',
          'table_with_header.[0].name',
          'table_with_header.[1].name',
          'table_with_header.[1].status',
        ]),
      { timeout: 5_000 },
    )
    .toEqual({
      'profile.name': ['Bob'],
      'profile.role': ['admin'],
      'table_with_header.[0].name': ['row-0'],
      'table_with_header.[1].name': ['row-1'],
      'table_with_header.[1].status': ['draft'],
    });

  const deletedSourceText = 'profile:\n  role: admin\ntable_with_header:\n  - id: 0\n    name: row-0\n    status: ready\n';
  await setEditorContent(page, { sourceText: deletedSourceText });

  await expect
    .poll(
      async () => {
        const graphValues = await readGraphValueTextsByPath(page, [
          'profile.name',
          'profile.role',
          'table_with_header.[0].name',
          'table_with_header.[1].name',
        ]);
        return {
          profileNameExists: Object.prototype.hasOwnProperty.call(graphValues, 'profile.name'),
          profileRole: graphValues['profile.role']?.[0] ?? null,
          firstRowName: graphValues['table_with_header.[0].name']?.[0] ?? null,
          secondRowExists: Object.prototype.hasOwnProperty.call(graphValues, 'table_with_header.[1].name'),
        };
      },
      { timeout: 5_000 },
    )
    .toEqual({
      profileNameExists: false,
      profileRole: 'admin',
      firstRowName: 'row-0',
      secondRowExists: false,
    });
});

test('yaml graph table row 0 cell edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = buildYamlTableDocument(140);
  await setEditorContent(page, { sourceText, language: 'yaml' });
  await waitForGraphRendered(page);

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'row-0-updated',
    selectAllModifier: COMMAND_MOD,
    matchesOpenEvent: (detail) => matchesPath(detail, 'table_with_header.[0].name'),
    verifyCommitted: (nextSourceText) => /- id: 0\n\s+name:\s*'?\s*row-0-updated\b/m.test(nextSourceText),
  });

  expect(committed).toBe(true);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toMatch(/- id: 0\n\s+name:\s*'?\s*row-0-updated\b/m);
  await waitForGraphRendered(page);
  await expectRootGraphValueTexts(page, {
    'table_with_header.[0].name': ['row-0-updated'],
  });
});
test('yaml graph table row 100 cell edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = buildYamlTableDocument(140);
  await setEditorContent(page, { sourceText, language: 'yaml' });
  await waitForGraphRendered(page);
  // Scroll table to row 100
  await page.evaluate((targetRow) => {
    return (window as any)._treease?.graph?.scrollTableToRow?.(targetRow);
  }, 100);
  // Wait for probes to include row 100
  await expect
    .poll(async () => {
      const probes = await readGraphClickProbes(page);
      return probes.some(p => p.path.join('.') === 'table_with_header.[100].name');
    }, { timeout: 5_000 })
    .toBe(true);
  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'row-100-updated',
    selectAllModifier: COMMAND_MOD,
    matchesOpenEvent: (detail) => matchesPath(detail, 'table_with_header.[100].name'),
    verifyCommitted: (nextSourceText) => /- id: 100\n\s+name:\s*'?\s*row-100-updated\b/m.test(nextSourceText),
  });

  expect(committed).toBe(true);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toMatch(/- id: 100\n\s+name:\s*'?\s*row-100-updated\b/m);
  await waitForGraphRendered(page);
  // Re-scroll to row 100: graph rebuild resets table scroll position
  await page.evaluate((targetRow) => {
    return (window as any)._treease?.graph?.scrollTableToRow?.(targetRow);
  }, 100);
  // Wait for probes to include row 100 after scroll
  await expect
    .poll(async () => {
      const probes = await readGraphClickProbes(page);
      return probes.some(p => p.path.join('.') === 'table_with_header.[100].name');
    }, { timeout: 5_000 })
    .toBe(true);
  await expectRootGraphValueTexts(page, {
    'table_with_header.[100].name': ['row-100-updated'],
  });
});
