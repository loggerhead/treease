import { readFileSync } from 'node:fs';
import { mkdir, open, readdir, writeFile } from 'node:fs/promises';
import { basename, dirname, extname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initWasm } from '@core-wasm/index';
import { guessLanguage } from './guess-language';
import { exampleLanguageByExtension } from '../monaco/language-support';
import type { SupportedEditorLanguageId } from '../monaco/language-support';

const exampleFiles = import.meta.glob('../../../../../example/*', {
  query: '?raw',
  import: 'default',
  eager: true,
});

const repoRoot = join(process.cwd(), '..', '..');
const fixtureRoot = join(repoRoot, 'test', 'fixtures');
const failureReportPath = join(repoRoot, 'guess-failure.md');
const sampleByteLength = 1024;
const corpusParallelism = 64;
const corpusLanguages = ['json', 'toml', 'yaml'] as const satisfies readonly SupportedEditorLanguageId[];
const corpusLanguageByExtension = new Map<string, CorpusLanguage>([
  ['.json', 'json'],
  ['.toml', 'toml'],
  ['.yaml', 'yaml'],
  ['.yml', 'yaml'],
]);

type CorpusLanguage = (typeof corpusLanguages)[number];

type CorpusCase = {
  path: string;
  relativePath: string;
  expected: CorpusLanguage;
};

type CorpusResult = CorpusCase & {
  actual: SupportedEditorLanguageId | null;
};

type CorpusStats = Record<CorpusLanguage, { total: number; failures: number }>;

function cloneWasmBytes(path: string): ArrayBuffer {
  const bytes = readFileSync(path);
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

async function mapConcurrent<T, R>(items: T[], limit: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results = Array.from({ length: items.length }) as R[];
  let nextIndex = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (nextIndex < items.length) {
      const index = nextIndex;
      nextIndex += 1;
      results[index] = await fn(items[index]);
    }
  });
  await Promise.all(workers);
  return results;
}

async function collectFixtureCases(): Promise<CorpusCase[]> {
  const entries = await readdir(fixtureRoot, { recursive: true, withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile())
    .flatMap((entry) => {
      const expected = corpusLanguageByExtension.get(extname(entry.name));
      if (!expected) return [];
      if (!isValidCorpusFileName(entry.name, expected)) return [];
      const path = join(entry.parentPath, entry.name);
      return [
        {
          path,
          relativePath: relative(repoRoot, path),
          expected,
        },
      ];
    })
    .sort((a, b) => a.relativePath.localeCompare(b.relativePath));
}

async function readFirstBytes(path: string): Promise<string> {
  const handle = await open(path, 'r');
  try {
    const buffer = Buffer.alloc(sampleByteLength);
    const { bytesRead } = await handle.read(buffer, 0, sampleByteLength, 0);
    return buffer.subarray(0, bytesRead).toString('utf8');
  } finally {
    await handle.close();
  }
}

function createEmptyStats(): CorpusStats {
  return Object.fromEntries(corpusLanguages.map((language) => [language, { total: 0, failures: 0 }])) as CorpusStats;
}

function isValidCorpusFileName(fileName: string, expected: CorpusLanguage): boolean {
  if (expected === 'yaml') return fileName.endsWith('.1.yaml') || fileName.endsWith('.1.yml');
  return fileName.endsWith(`.1.${expected}`);
}

function isAmbiguousTomlCorpusInput(text: string): boolean {
  const trimmed = text.trim();
  if (trimmed.length === 0) return true;
  return /^\[\s*"[^"\r\n]*"\s*\]$/.test(trimmed);
}

function isJsonParsableInput(text: string): boolean {
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

function isAmbiguousYamlCorpusInput(text: string): boolean {
  const trimmed = text.trim();
  if (trimmed.length === 0) return true;
  const nonEmptyLines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (nonEmptyLines.length > 0 && nonEmptyLines.every((line) => line.startsWith('#'))) return true;
  if (isJsonParsableInput(trimmed)) return true;
  if (/^[-!:]+$/.test(trimmed)) return true;
  if (/^---\S/.test(trimmed)) return true;
  if (/^[\w\s.-]+$/.test(trimmed) && !/:\s/.test(trimmed)) return true;
  if (/^\[\s*\?/.test(trimmed)) return true;
  if (/^(?:\{|\[)/.test(trimmed) && !/[!?&*|>%#]/.test(trimmed)) return true;
  if (/^["'][^\r\n]*["']$/.test(trimmed)) return true;
  return false;
}

function summarizeCorpus(results: CorpusResult[]): { failures: CorpusResult[]; stats: CorpusStats } {
  const stats = createEmptyStats();
  const failures: CorpusResult[] = [];
  for (const result of results) {
    stats[result.expected].total += 1;
    if (result.actual !== result.expected) {
      stats[result.expected].failures += 1;
      failures.push(result);
    }
  }
  return { failures, stats };
}

function formatPercent(count: number, total: number): string {
  if (total === 0) return '0.00%';
  return `${((count / total) * 100).toFixed(2)}%`;
}

function renderSummary(stats: CorpusStats): string {
  const lines = ['guess language fixture corpus summary:', 'language | failures | total | failure rate'];
  let total = 0;
  let totalFailures = 0;
  for (const language of corpusLanguages) {
    const languageStats = stats[language];
    total += languageStats.total;
    totalFailures += languageStats.failures;
    lines.push(
      `${language} | ${languageStats.failures} | ${languageStats.total} | ${formatPercent(
        languageStats.failures,
        languageStats.total,
      )}`,
    );
  }
  lines.push(`total | ${totalFailures} | ${total} | ${formatPercent(totalFailures, total)}`);
  return lines.join('\n');
}

function renderFailureReport(results: CorpusResult[], failures: CorpusResult[], stats: CorpusStats): string {
  const generatedAt = new Date().toISOString();
  const summaryRows = corpusLanguages.map((language) => {
    const languageStats = stats[language];
    return `| ${language} | ${languageStats.failures} | ${languageStats.total} | ${formatPercent(
      languageStats.failures,
      languageStats.total,
    )} |`;
  });
  const total = results.length;
  const totalFailures = failures.length;
  const failureRows = failures.map(
    (failure) => `| \`${failure.relativePath}\` | ${failure.expected} | ${failure.actual ?? 'null'} |`,
  );
  return [
    '# Guess Language Failures',
    '',
    `Generated at: ${generatedAt}`,
    '',
    `Input sample: first ${sampleByteLength} bytes of each valid fixture file.`,
    '',
    '## Summary',
    '',
    '| Language | Failures | Total | Failure Rate |',
    '| --- | ---: | ---: | ---: |',
    ...summaryRows,
    `| total | ${totalFailures} | ${total} | ${formatPercent(totalFailures, total)} |`,
    '',
    '## Failures',
    '',
    failures.length === 0 ? 'No failures.' : '| File | Expected | Actual |',
    failures.length === 0 ? '' : '| --- | --- | --- |',
    ...failureRows,
    '',
  ].join('\n');
}

describe('guessLanguage', () => {
  beforeAll(async () => {
    const wasmPath = fileURLToPath(new URL('../../../../../packages/core/wasm/pkg/core.wasm', import.meta.url));
    await initWasm({ wasmBytes: cloneWasmBytes(wasmPath) });
  }, 5_000);

  describe('example corpus', () => {
    const entries = Object.entries(exampleFiles);

    for (const [path, content] of entries) {
      const fileName = path.split('/').pop() ?? '';
      if (!fileName.startsWith('simple.')) continue;
      if (fileName.startsWith('simple.overrides.')) continue;
      const extension = fileName.split('.').pop();
      if (!extension) continue;
      const expected = exampleLanguageByExtension.get(extension);
      if (!expected) continue;

      it(`detects ${expected} from ${fileName}`, async () => {
        const text = String(content ?? '');
        const guessed = await guessLanguage(text);
        expect(guessed).toBe(expected);
      });
    }
  });

  describe('short input guards', () => {
    it('returns null for empty string', async () => {
      expect(await guessLanguage('')).toBeNull();
    });

    it('returns null for whitespace-only below threshold', async () => {
      expect(await guessLanguage('   ')).toBeNull();
    });

    it('returns null for string shorter than 8 chars', async () => {
      expect(await guessLanguage('abc')).toBeNull();
    });
  });

  describe('feature-only scoring', () => {
    it('detects JSON from double-quoted keys', async () => {
      const input = '{"name": "alice", "age": 30}';
      expect(await guessLanguage(input)).toBe('json');
    });

    it('detects YAML from colon-newline + indent + yaml list', async () => {
      const input = 'name: alice\nage: 30\nitems:\n  - one\n  - two';
      expect(await guessLanguage(input)).toBe('yaml');
    });

    it('detects TOML from sections and equals', async () => {
      const input = '[database]\nhost = "localhost"\nport = 5432';
      expect(await guessLanguage(input)).toBe('toml');
    });


    it('detects Python from single-quoted keys + True/None literals', async () => {
      const input = "{'name': 'alice', 'active': True, 'value': None}";
      expect(await guessLanguage(input)).toBe('python');
    });

    it('detects JavaScript from unquoted keys + trailing comma', async () => {
      const input = '{ name: "alice", age: 30, }';
      expect(await guessLanguage(input)).toBe('javascript');
    });

    it('does not treat object-like text inside pretty JSON strings as unquoted keys', async () => {
      const input = [
        '{',
        '  "description": "Valid values, including:\\n* active\\n* inactive",',
        '  "type": "string"',
        '}',
      ].join('\n');
      expect(await guessLanguage(input)).toBe('json');
    });
  });

  describe('fixture corpus', () => {
    it('detects the language from the first 1024 bytes of every valid fixture file', async () => {
      const cases = await collectFixtureCases();
      const results = await mapConcurrent(cases, corpusParallelism, async (testCase) => {
        const text = await readFirstBytes(testCase.path);
        if (testCase.expected === 'toml' && isAmbiguousTomlCorpusInput(text)) return null;
        if (testCase.expected === 'yaml' && isAmbiguousYamlCorpusInput(text)) return null;
        const actual = await guessLanguage(text);
        return { ...testCase, actual };
      });
      const includedResults = results.filter((result): result is CorpusResult => result !== null);
      const { failures, stats } = summarizeCorpus(includedResults);
      const summary = renderSummary(stats);
      await mkdir(dirname(failureReportPath), { recursive: true });
      await writeFile(failureReportPath, renderFailureReport(includedResults, failures, stats), 'utf8');
      console.info(summary);
      expect(failures, `${summary}\n\nSee ${basename(failureReportPath)} for failed filenames.`).toHaveLength(0);
    });
  });

  describe('with diagnosticsProvider', () => {
    it('ignores diagnosticsProvider and keeps feature-only json result', async () => {
      const provider = vi.fn(async (lang: string, _text: string) => {
        if (lang === 'json') return [];
        return [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 5, kind: 1 }];
      });
      const input = '{"key": "value"}';
      const result = await guessLanguage(input, provider);
      expect(result).toBe('json');
      expect(provider).not.toHaveBeenCalled();
    });

    it('ignores diagnosticsProvider and keeps feature-only yaml result', async () => {
      const provider = vi.fn(async (lang: string) => {
        if (lang === 'yaml') return [];
        return [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 20, kind: 1 }];
      });
      const input = 'name: alice\nage: 30\n';
      const result = await guessLanguage(input, provider);
      expect(result).toBe('yaml');
      expect(provider).not.toHaveBeenCalled();
    });

    it('ignores diagnosticsProvider and keeps short-input null result', async () => {
      const provider = vi.fn(async () => []);
      const input = 'abc';
      const result = await guessLanguage(input, provider);
      expect(result).toBeNull();
      expect(provider).not.toHaveBeenCalled();
    });
  });
});
