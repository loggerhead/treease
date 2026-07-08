import { beforeAll, describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { buildDiffPlans } from './diff-plan';
import { diffStructured, diffText, initWasm } from '@core-wasm/index';

const monaco = {
  editor: {
    MinimapPosition: { Inline: 1 },
    OverviewRulerLane: { Full: 2 },
  },
};

function cloneWasmBytes(path: string): ArrayBuffer {
  const bytes = readFileSync(path);
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

beforeAll(async () => {
  const wasmPath = fileURLToPath(new URL('../../../../../packages/core/wasm/pkg/core.wasm', import.meta.url));
  await initWasm({ wasmBytes: cloneWasmBytes(wasmPath) });
}, 5_000);

describe('diff-plan with real wasm compare output', () => {
  function extractRangeText(text: string, range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number }) {
    const lines = text.split('\n');
    if (range.startLineNumber !== range.endLineNumber) return null;
    const line = lines[range.startLineNumber - 1] ?? '';
    return line.slice(range.startColumn - 1, range.endColumn - 1);
  }

  it('maps inline diff vectors to Monaco inline decorations', async () => {
    const left = '  "foo": "abc" }';
    const right = '{ "foo": "adc" }';
    const result = await diffText(left, right);
    const plans = buildDiffPlans(monaco, result.pairs ?? [], left, right);

    const leftInline = plans.left.decorations
      .filter((item) => item.options.inlineClassName === 'diff-inline-del')
      .map((item) => item.range);
    const rightInline = plans.right.decorations
      .filter((item) => item.options.inlineClassName === 'diff-inline-ins')
      .map((item) => item.range);

    expect(leftInline).toEqual([
      { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, type: 1, inlineDiffs: [] },
      { startLineNumber: 1, startColumn: 12, endLineNumber: 1, endColumn: 13, type: 1, inlineDiffs: [] },
    ]);
    expect(rightInline).toEqual([
      { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, type: 0, inlineDiffs: [] },
      { startLineNumber: 1, startColumn: 12, endLineNumber: 1, endColumn: 13, type: 0, inlineDiffs: [] },
    ]);
  });

  it('maps newline-only deletion to a same-line Monaco hunk', async () => {
    const left = '{\n\n  return tokens;\n}';
    const right = '{\n  return tokens;\n}';
    const result = await diffText(left, right);
    const plans = buildDiffPlans(monaco, result.pairs ?? [], left, right);

    const leftHunks = plans.left.decorations.filter((item) => item.options.className === 'diff-line-del');

    expect(leftHunks).toEqual([
      {
        range: { startLineNumber: 2, startColumn: 1, endLineNumber: 2, endColumn: 1, type: 1, inlineDiffs: [] },
        options: {
          isWholeLine: true,
          className: 'diff-line-del',
          marginClassName: 'diff-margin-del',
          minimap: { color: 'rgba(248,113,113,0.6)', position: 1 },
          overviewRuler: { color: 'rgba(248,113,113,0.8)', position: 2 },
        },
      },
    ]);
  });

  it('maps structured inline diff vectors to Monaco inline decorations', async () => {
    const left = '{ "foo": "abc" }';
    const right = '{ "foo": "adc" }';
    const result = await diffStructured('json', left, right);
    const plans = buildDiffPlans(monaco, result.pairs ?? [], left, right);

    const leftInline = plans.left.decorations
      .filter((item) => item.options.inlineClassName === 'diff-inline-del')
      .map((item) => item.range);
    const rightInline = plans.right.decorations
      .filter((item) => item.options.inlineClassName === 'diff-inline-ins')
      .map((item) => item.range);

    expect(leftInline).toEqual([
      { startLineNumber: 1, startColumn: 12, endLineNumber: 1, endColumn: 13, type: 1, inlineDiffs: [] },
    ]);
    expect(rightInline).toEqual([
      { startLineNumber: 1, startColumn: 12, endLineNumber: 1, endColumn: 13, type: 0, inlineDiffs: [] },
    ]);
  });

  it('maps structured object-entry hunks to Monaco whole-line decorations', async () => {
    const left = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {".vscode/": true,"foo": "bar"}}';
    const right = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {".slash/": true,"foo": "bar"}}';
    const result = await diffStructured('json', left, right);
    const plans = buildDiffPlans(monaco, result.pairs ?? [], left, right);

    const leftHunks = plans.left.decorations.filter((item) => item.options.className === 'diff-line-del');
    const rightHunks = plans.right.decorations.filter((item) => item.options.className === 'diff-line-ins');

    expect(leftHunks).toEqual([
      {
        range: { startLineNumber: 1, startColumn: 74, endLineNumber: 1, endColumn: 90, type: 1, inlineDiffs: [] },
        options: {
          isWholeLine: true,
          className: 'diff-line-del',
          marginClassName: 'diff-margin-del',
          minimap: { color: 'rgba(248,113,113,0.6)', position: 1 },
          overviewRuler: { color: 'rgba(248,113,113,0.8)', position: 2 },
        },
      },
    ]);
    expect(rightHunks).toEqual([
      {
        range: { startLineNumber: 1, startColumn: 74, endLineNumber: 1, endColumn: 89, type: 0, inlineDiffs: [] },
        options: {
          isWholeLine: true,
          className: 'diff-line-ins',
          marginClassName: 'diff-margin-ins',
          minimap: { color: 'rgba(34,197,94,0.6)', position: 1 },
          overviewRuler: { color: 'rgba(34,197,94,0.8)', position: 2 },
        },
      },
    ]);
  });

  it('keeps structured unicode inline ranges off unchanged sibling tokens', async () => {
    const left = `{
  "value": {"message": "存在差异：新增 577 行，删除 382 行", "type": "info"}
}`;
    const right = `{
  "value": {"message": "就你就于：们时 525 有，人那 168 就", "type": "dsjk"}
}`;
    const result = await diffStructured('json', left, right);
    const plans = buildDiffPlans(monaco, result.pairs ?? [], left, right);

    const rightInlineTexts = plans.right.decorations
      .filter((item) => item.options.inlineClassName === 'diff-inline-ins')
      .map((item) => extractRangeText(right, item.range))
      .filter((value): value is string => value !== null);

    expect(rightInlineTexts).not.toContain('type');
    expect(rightInlineTexts).not.toContain('type"');
    expect(rightInlineTexts).not.toContain(':');
    expect(rightInlineTexts).not.toContain('}');
    expect(rightInlineTexts.some((value) => value.includes('168'))).toBe(true);
    expect(rightInlineTexts.some((value) => value.includes('dsjk'))).toBe(true);
  });
});
