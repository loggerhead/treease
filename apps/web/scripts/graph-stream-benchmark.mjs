import { spawn, execFileSync } from 'node:child_process';
import { mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const repoRoot = path.resolve(webDir, '..', '..');
const fixturesRoot = path.resolve(repoRoot, 'test', 'fixtures');
const outputDir = path.resolve(webDir, '.tmp', 'graph-stream-benchmark');
const latestJsonPath = path.resolve(outputDir, 'latest.json');
const latestMarkdownPath = path.resolve(outputDir, 'latest.md');
const baselinePath = path.resolve(webDir, 'benchmarks', 'graph-stream-baseline.json');

const defaultCandidateChunkSizes = [16 * 1024, 32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024];
const longFrameMs = 50;
const veryLongFrameMs = 100;
const caseTimeoutMs = 45_000;
const bucketDefinitions = [
  { name: '<256KB', minBytes: 0, maxBytes: 256 * 1024 },
  { name: '256KB-1MB', minBytes: 256 * 1024, maxBytes: 1024 * 1024 },
  { name: '1MB-4MB', minBytes: 1024 * 1024, maxBytes: 4 * 1024 * 1024 },
  { name: '4MB-8MB', minBytes: 4 * 1024 * 1024, maxBytes: 8 * 1024 * 1024 },
  { name: '>=8MB', minBytes: 8 * 1024 * 1024, maxBytes: null },
];
const languages = [
  { dir: 'json', language: 'json' },
  { dir: 'toml', language: 'toml' },
  { dir: 'yaml', language: 'yaml' },
];
const thresholds = {
  successRateDrop: 0,
  throughputAvgRegressionPct: 0.35,
  throughputAvgRegressionMs: 75,
  smoothnessAvgRegressionPct: 0.35,
  smoothnessAvgRegressionMs: 16,
  longFrameCountRegression: 4,
};

const cliArgs = parseCliArgs(process.argv.slice(2));
const candidateChunkSizes = cliArgs.chunkSizes ?? defaultCandidateChunkSizes;
const updateBaseline = cliArgs.updateBaseline;
const skipBaselineComparison = cliArgs.skipBaselineComparison;
const fixtureMinBytes = cliArgs.fixtureMinBytes;
const fixtureMaxBytes = cliArgs.fixtureMaxBytes;

async function main() {
  await mkdir(outputDir, { recursive: true });

  const selectedCases = await selectBenchmarkCases();
  const fixtureInventory = buildFixtureInventory(selectedCases);

  if (selectedCases.length === 0) {
    throw new Error('No valid benchmark fixtures selected from test/fixtures.');
  }

  await ensureWasmArtifacts();
  const wasmVersion = resolveWasmVersion();

  const candidateResults = [];
  for (const chunkSize of candidateChunkSizes) {
    console.log(`[bench] chunkSize=${formatBytes(chunkSize)} start`);
    const result = await benchmarkChunkSize({ chunkSize, wasmVersion, selectedCases });
    candidateResults.push(result);
    console.log(`[bench] chunkSize=${formatBytes(chunkSize)} done`);
  }

  const bucketSummaries = buildBucketSummaries(candidateResults, fixtureInventory);
  const recommendations = buildRecommendations(bucketSummaries);
  const baseline = existsSync(baselinePath) ? JSON.parse(await readFile(baselinePath, 'utf8')) : null;
  const comparison = skipBaselineComparison
    ? {
        status: 'skipped',
        baselinePath: path.relative(webDir, baselinePath),
        failures: [],
      }
    : compareAgainstBaseline({ bucketSummaries, recommendations }, baseline);

  const latest = {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    runner: {
      candidateChunkSizes,
      longFrameMs,
      veryLongFrameMs,
      caseTimeoutMs,
      baselinePath: path.relative(webDir, baselinePath),
      selectedFixturePolicy: 'per-bucket 1 largest valid raw fixture per language from test/fixtures/{json,toml,yaml}',
      fixtureSizeFilter: {
        minBytes: fixtureMinBytes,
        maxBytes: fixtureMaxBytes,
      },
    },
    thresholds,
    fixtureInventory,
    candidateResults,
    bucketSummaries,
    recommendations,
    comparison,
  };

  await writeFile(latestJsonPath, `${JSON.stringify(latest, null, 2)}\n`, 'utf8');
  await writeFile(latestMarkdownPath, buildMarkdownReport(latest), 'utf8');

  if (updateBaseline) {
    await mkdir(path.dirname(baselinePath), { recursive: true });
    const baselinePayload = {
      schemaVersion: latest.schemaVersion,
      generatedAt: latest.generatedAt,
      runner: latest.runner,
      thresholds,
      fixtureInventory,
      bucketSummaries,
      recommendations,
    };
    await writeFile(baselinePath, `${JSON.stringify(baselinePayload, null, 2)}\n`, 'utf8');
    console.log(`[bench] baseline updated: ${path.relative(webDir, baselinePath)}`);
  }

  console.log(`[bench] wrote ${path.relative(webDir, latestJsonPath)}`);
  console.log(`[bench] wrote ${path.relative(webDir, latestMarkdownPath)}`);

  if (comparison.status === 'regression') {
    throw new Error(`Benchmark regression detected: ${comparison.failures.join('; ')}`);
  }
}

async function selectBenchmarkCases() {
  const grouped = new Map();

  for (const spec of languages) {
    const dirPath = path.resolve(fixturesRoot, spec.dir);
    for (const entry of await readdir(dirPath, { withFileTypes: true })) {
      if (!entry.isFile() || !entry.name.includes('.1.')) continue;
      const absolutePath = path.resolve(dirPath, entry.name);
      const size = (await stat(absolutePath)).size;
      if (fixtureMinBytes != null && size < fixtureMinBytes) continue;
      if (fixtureMaxBytes != null && size > fixtureMaxBytes) continue;
      const bucket = bucketForBytes(size);
      const groupKey = `${bucket}:${spec.language}`;
      const list = grouped.get(groupKey) ?? [];
      list.push({
        id: `${spec.language}:${bucket}:${entry.name}`,
        language: spec.language,
        repoPath: path.relative(repoRoot, absolutePath),
        fileName: entry.name,
        bytes: size,
        bucket,
      });
      grouped.set(groupKey, list);
    }
  }

  const selected = [];
  for (const bucket of bucketDefinitions) {
    for (const spec of languages) {
      const groupKey = `${bucket.name}:${spec.language}`;
      const list = grouped.get(groupKey) ?? [];
      if (list.length === 0) continue;
      list.sort((a, b) => b.bytes - a.bytes || a.fileName.localeCompare(b.fileName));
      selected.push(list[0]);
    }
  }

  selected.sort((a, b) => {
    const bucketDelta = bucketIndex(a.bucket) - bucketIndex(b.bucket);
    if (bucketDelta !== 0) return bucketDelta;
    const languageDelta = a.language.localeCompare(b.language);
    if (languageDelta !== 0) return languageDelta;
    return a.fileName.localeCompare(b.fileName);
  });
  return selected;
}

function parseCliArgs(argv) {
  const parsed = {
    updateBaseline: false,
    skipBaselineComparison: false,
    chunkSizes: null,
    fixtureMinBytes: null,
    fixtureMaxBytes: null,
  };

  for (const arg of argv) {
    if (arg === '--') {
      continue;
    }
    if (arg === '--update-baseline') {
      parsed.updateBaseline = true;
      continue;
    }
    if (arg === '--skip-baseline-comparison') {
      parsed.skipBaselineComparison = true;
      continue;
    }
    if (arg.startsWith('--chunk-sizes=')) {
      const raw = arg.slice('--chunk-sizes='.length);
      parsed.chunkSizes = raw
        .split(',')
        .map((value) => parseByteSize(value.trim()))
        .filter((value) => Number.isFinite(value) && value > 0);
      if ((parsed.chunkSizes?.length ?? 0) === 0) {
        throw new Error(`Invalid --chunk-sizes value: ${raw}`);
      }
      continue;
    }
    if (arg.startsWith('--fixture-min-bytes=')) {
      parsed.fixtureMinBytes = parseByteSize(arg.slice('--fixture-min-bytes='.length));
      continue;
    }
    if (arg.startsWith('--fixture-max-bytes=')) {
      parsed.fixtureMaxBytes = parseByteSize(arg.slice('--fixture-max-bytes='.length));
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return parsed;
}

function parseByteSize(rawValue) {
  const value = rawValue.trim().toLowerCase();
  const match = value.match(/^(\d+(?:\.\d+)?)(kb|mb|gb)?$/);
  if (!match) {
    throw new Error(`Invalid byte size: ${rawValue}`);
  }
  const amount = Number(match[1]);
  const unit = match[2] ?? 'b';
  const multiplier = unit === 'kb' ? 1024 : unit === 'mb' ? 1024 * 1024 : unit === 'gb' ? 1024 * 1024 * 1024 : 1;
  return Math.round(amount * multiplier);
}

function buildFixtureInventory(selectedCases) {
  const buckets = {};
  for (const bucket of bucketDefinitions) {
    const cases = selectedCases.filter((item) => item.bucket === bucket.name);
    const languagesInBucket = [...new Set(cases.map((item) => item.language))].sort();
    const riskFlags = [];
    if (cases.length === 0) {
      riskFlags.push('insufficient-fixtures');
    }
    if (cases.length > 0 && cases.length < 2) {
      riskFlags.push('low-confidence');
    }
    if (cases.length > 0 && languagesInBucket.length < 2) {
      riskFlags.push('low-language-diversity');
    }
    if (languagesInBucket.length === 1 && languagesInBucket[0] === 'json') {
      riskFlags.push('json-only');
    }
    buckets[bucket.name] = {
      sampleCount: cases.length,
      languages: languagesInBucket,
      riskFlags,
      cases,
    };
  }
  return { buckets, selectedCaseCount: selectedCases.length };
}

async function ensureWasmArtifacts() {
  await runCommand('pnpm', ['wasm:sync'], { cwd: webDir, stdio: 'inherit' });
}

function resolveWasmVersion() {
  return execFileSync('node', ['./scripts/wasm-version.mjs'], {
    cwd: webDir,
    encoding: 'utf8',
  }).trim();
}

async function benchmarkChunkSize({ chunkSize, wasmVersion, selectedCases }) {
  const port = 4173;
  const server = await startViteServer({ chunkSize, wasmVersion, port });
  const browser = await chromium.launch({ headless: true });
  try {
    await warmupBrowser(browser, `http://127.0.0.1:${port}`);
    const cases = [];
    for (const fixture of selectedCases) {
      const sourceText = await readFile(path.resolve(repoRoot, fixture.repoPath), 'utf8');
      console.log(`  [case] ${fixture.bucket} ${fixture.language} ${fixture.fileName}`);
      cases.push(
        await benchmarkCase(browser, `http://127.0.0.1:${port}`, {
          ...fixture,
          sourceText,
        }),
      );
    }
    return {
      chunkSize,
      cases,
    };
  } finally {
    await browser.close();
    await stopProcess(server.child);
  }
}

async function startViteServer({ chunkSize, wasmVersion, port }) {
  const child = spawn('pnpm', ['exec', 'vite', '--host', '127.0.0.1', '--port', String(port), '--strictPort'], {
    cwd: webDir,
    env: {
      ...process.env,
      WASM_VERSION: wasmVersion,
      TREEASE_WASM_IMPL: 'rust',
      TREEASE_WASM_STREAM_CHUNK_PRODUCTION: String(chunkSize),
      BENCHMARK_MODE: '1',
    },
  });

  let stdout = '';
  let stderr = '';
  child.stdout?.on('data', (chunk) => {
    stdout += String(chunk);
    process.stdout.write(String(chunk));
  });
  child.stderr?.on('data', (chunk) => {
    stderr += String(chunk);
    process.stderr.write(String(chunk));
  });

  await waitForHttpReady(`http://127.0.0.1:${port}/editor`, child, { stdout, stderr }, 60_000);
  return { child };
}

async function waitForHttpReady(url, child, logs, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (child.exitCode != null) {
      throw new Error(`Vite exited before ready (code=${child.exitCode}).`);
    }
    try {
      const response = await fetch(url, { redirect: 'manual' });
      if (response.status >= 200 && response.status < 500) {
        return;
      }
    } catch {
      // keep polling
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for Vite at ${url}.`);
}

async function warmupBrowser(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  try {
    await page.goto(`${baseUrl}/editor`, { waitUntil: 'domcontentloaded' });
    await waitForEditorReady(page);
    await runPageBenchmark(page, {
      sourceText: '{"warmup":1}',
      language: 'json',
      timeoutMs: 10_000,
      longFrameMs,
      veryLongFrameMs,
    });
  } finally {
    await page.close();
  }
}

async function benchmarkCase(browser, baseUrl, fixture) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  try {
    await page.goto(`${baseUrl}/editor`, { waitUntil: 'domcontentloaded' });
    await waitForEditorReady(page);
    const metrics = await runPageBenchmark(page, {
      sourceText: fixture.sourceText,
      language: fixture.language,
      timeoutMs: caseTimeoutMs,
      longFrameMs,
      veryLongFrameMs,
    });
    return {
      id: fixture.id,
      language: fixture.language,
      bucket: fixture.bucket,
      repoPath: fixture.repoPath,
      fileName: fixture.fileName,
      bytes: fixture.bytes,
      ...metrics,
    };
  } finally {
    await page.close();
  }
}

async function waitForEditorReady(page) {
  await page.waitForFunction(() => Boolean(window._treease?.editor?.isReady?.('source-editor')), { timeout: 30_000 });
}

async function runPageBenchmark(page, payload) {
  return page.evaluate(async ({ sourceText, language, timeoutMs, longFrameMs, veryLongFrameMs }) => {
    const treease = window._treease;
    if (!treease) {
      throw new Error('window._treease is unavailable');
    }

    const pauseForFrame = () =>
      new Promise((resolve) => {
        requestAnimationFrame(() => resolve(undefined));
      });

    const percentile = (values, p) => {
      if (values.length === 0) return null;
      const sorted = [...values].sort((a, b) => a - b);
      const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * p) - 1));
      return sorted[index] ?? null;
    };

    const beforeRevision = treease.editor.getState().editorRevision;
    let lastFrameAt = performance.now();
    const frameGaps = [];
    let rafId = 0;
    let trackingFrames = true;
    const frameTick = (now) => {
      if (!trackingFrames) return;
      frameGaps.push(now - lastFrameAt);
      lastFrameAt = now;
      rafId = requestAnimationFrame(frameTick);
    };
    rafId = requestAnimationFrame(frameTick);

    treease.editor.setLanguageId(language);
    treease.editor.setValue('source-editor', sourceText);

    let targetRevision = beforeRevision;
    const startedAt = performance.now();
    while (performance.now() - startedAt < timeoutMs) {
      const state = treease.editor.getState();
      if (state.sourceText === sourceText && state.languageId === language && state.editorRevision > beforeRevision) {
        targetRevision = state.editorRevision;
        break;
      }
      await pauseForFrame();
    }

    let timedOut = true;
    let finalState = treease.editor.getState();
    let streamState = treease.graph.getStreamState?.() ?? null;
    while (performance.now() - startedAt < timeoutMs) {
      finalState = treease.editor.getState();
      streamState = treease.graph.getStreamState?.() ?? null;
      const revisionMatches = Number(streamState?.revision ?? -1) === finalState.editorRevision;
      const done = Number.isFinite(streamState?.doneAtMs)
        && streamState?.finalSeen === true
        && finalState.graphAppliedRevision >= finalState.editorRevision
        && finalState.editorRevision >= targetRevision
        && revisionMatches;
      const failed = Number.isFinite(streamState?.failedAtMs) && Number.isFinite(streamState?.doneAtMs);
      if (done || failed) {
        timedOut = false;
        break;
      }
      await pauseForFrame();
    }

    trackingFrames = false;
    cancelAnimationFrame(rafId);
    await pauseForFrame();
    finalState = treease.editor.getState();
    streamState = treease.graph.getStreamState?.() ?? null;

    const measuredFrameGaps = frameGaps.slice(1);
    const startedAtMs = Number(streamState?.startedAtMs ?? NaN);
    const firstPartialAtMs = Number(streamState?.firstPartialAtMs ?? NaN);
    const doneAtMs = Number(streamState?.doneAtMs ?? NaN);
    const appliedAtMs = Number(streamState?.appliedAtMs ?? NaN);

    return {
      success: !timedOut && Boolean(streamState?.finalSeen) && finalState.graphAppliedRevision >= finalState.editorRevision,
      timedOut,
      finalRevision: finalState.editorRevision,
      graphAppliedRevision: finalState.graphAppliedRevision,
      partialSeen: Boolean(streamState?.partialSeen),
      finalSeen: Boolean(streamState?.finalSeen),
      errorMessage: String(streamState?.errorMessage ?? ''),
      timeToFirstPartialMs: Number.isFinite(firstPartialAtMs) && Number.isFinite(startedAtMs) ? firstPartialAtMs - startedAtMs : null,
      timeToDoneMs: Number.isFinite(doneAtMs) && Number.isFinite(startedAtMs) ? doneAtMs - startedAtMs : null,
      timeToGraphAppliedMs: Number.isFinite(appliedAtMs) && Number.isFinite(startedAtMs) ? appliedAtMs - startedAtMs : null,
      chunkCount: Number(streamState?.chunkCount ?? 0),
      progressEventCount: Number(streamState?.progressEventCount ?? 0),
      applyDeltaCount: Number(streamState?.applyDeltaCount ?? 0),
      maxApplyDeltaMs: Number(streamState?.maxApplyDeltaMs ?? 0),
      renderCalls: Number(streamState?.renderCalls ?? 0),
      maxFrameGapMs: measuredFrameGaps.length === 0 ? null : Math.max(...measuredFrameGaps),
      p95FrameGapMs: percentile(measuredFrameGaps, 0.95),
      longFrameCount: measuredFrameGaps.filter((gap) => gap >= longFrameMs).length,
      veryLongFrameCount: measuredFrameGaps.filter((gap) => gap >= veryLongFrameMs).length,
    };
  }, payload);
}

function buildBucketSummaries(candidateResults, fixtureInventory) {
  const summaries = {};
  for (const bucket of bucketDefinitions) {
    const bucketCases = fixtureInventory.buckets[bucket.name];
    const candidateSummaries = candidateResults.map((candidate) => {
      const cases = candidate.cases.filter((item) => item.bucket === bucket.name);
      return {
        chunkSize: candidate.chunkSize,
        metrics: aggregateMetrics(cases),
      };
    });
    summaries[bucket.name] = {
      sampleCount: bucketCases.sampleCount,
      languages: bucketCases.languages,
      riskFlags: [...bucketCases.riskFlags],
      candidates: candidateSummaries,
    };
  }
  return summaries;
}

function aggregateMetrics(cases) {
  const successful = cases.filter((item) => item.success);
  const successRate = cases.length === 0 ? 0 : successful.length / cases.length;
  const average = (values) => {
    const filtered = values.filter((value) => Number.isFinite(value));
    if (filtered.length === 0) return null;
    return filtered.reduce((sum, value) => sum + value, 0) / filtered.length;
  };
  const percentile = (values, p) => {
    const filtered = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
    if (filtered.length === 0) return null;
    const index = Math.min(filtered.length - 1, Math.max(0, Math.ceil(filtered.length * p) - 1));
    return filtered[index] ?? null;
  };

  return {
    totalCases: cases.length,
    successfulCases: successful.length,
    successRate,
    avgTimeToFirstPartialMs: average(successful.map((item) => item.timeToFirstPartialMs)),
    avgTimeToDoneMs: average(successful.map((item) => item.timeToDoneMs)),
    avgTimeToGraphAppliedMs: average(successful.map((item) => item.timeToGraphAppliedMs)),
    p95TimeToGraphAppliedMs: percentile(successful.map((item) => item.timeToGraphAppliedMs), 0.95),
    avgChunkCount: average(successful.map((item) => item.chunkCount)),
    avgProgressEventCount: average(successful.map((item) => item.progressEventCount)),
    avgApplyDeltaCount: average(successful.map((item) => item.applyDeltaCount)),
    avgMaxApplyDeltaMs: average(successful.map((item) => item.maxApplyDeltaMs)),
    avgMaxFrameGapMs: average(successful.map((item) => item.maxFrameGapMs)),
    p95FrameGapMs: percentile(successful.map((item) => item.p95FrameGapMs), 0.95),
    avgLongFrameCount: average(successful.map((item) => item.longFrameCount)),
    avgVeryLongFrameCount: average(successful.map((item) => item.veryLongFrameCount)),
  };
}

function buildRecommendations(bucketSummaries) {
  const throughputByBucket = {};
  const smoothnessByBucket = {};
  for (const bucket of bucketDefinitions) {
    const summary = bucketSummaries[bucket.name];
    const throughput = chooseWinner(summary.candidates, compareThroughput);
    const smoothness = chooseWinner(summary.candidates, compareSmoothness);
    if (throughput) {
      throughputByBucket[bucket.name] = {
        chunkSize: throughput.chunkSize,
        metrics: throughput.metrics,
      };
    }
    if (smoothness) {
      smoothnessByBucket[bucket.name] = {
        chunkSize: smoothness.chunkSize,
        metrics: smoothness.metrics,
      };
    }
    if (
      throughput
      && smoothness
      && throughput.chunkSize !== smoothness.chunkSize
      && !summary.riskFlags.includes('split-recommendation')
    ) {
      summary.riskFlags.push('split-recommendation');
    }
  }
  return { throughputByBucket, smoothnessByBucket };
}

function chooseWinner(candidates, comparator) {
  const eligible = candidates.filter((candidate) => candidate.metrics.successfulCases > 0);
  if (eligible.length === 0) return null;
  return [...eligible].sort(comparator)[0] ?? null;
}

function compareThroughput(a, b) {
  return compareMetrics(a, b, [
    (candidate) => -candidate.metrics.successRate,
    (candidate) => candidate.metrics.avgTimeToGraphAppliedMs ?? Number.POSITIVE_INFINITY,
    (candidate) => candidate.metrics.p95TimeToGraphAppliedMs ?? Number.POSITIVE_INFINITY,
    (candidate) => candidate.chunkSize,
  ]);
}

function compareSmoothness(a, b) {
  return compareMetrics(a, b, [
    (candidate) => -candidate.metrics.successRate,
    (candidate) => candidate.metrics.avgLongFrameCount ?? Number.POSITIVE_INFINITY,
    (candidate) => candidate.metrics.avgMaxFrameGapMs ?? Number.POSITIVE_INFINITY,
    (candidate) => candidate.metrics.avgMaxApplyDeltaMs ?? Number.POSITIVE_INFINITY,
    (candidate) => candidate.chunkSize,
  ]);
}

function compareMetrics(a, b, selectors) {
  for (const select of selectors) {
    const left = select(a);
    const right = select(b);
    if (left < right) return -1;
    if (left > right) return 1;
  }
  return 0;
}

function compareAgainstBaseline(latest, baseline) {
  if (!baseline) {
    return {
      status: 'missing',
      baselinePath: path.relative(webDir, baselinePath),
      failures: [],
    };
  }

  const failures = [];
  for (const bucket of bucketDefinitions) {
    const bucketName = bucket.name;
    const currentThroughput = latest.recommendations.throughputByBucket[bucketName] ?? null;
    const currentSmoothness = latest.recommendations.smoothnessByBucket[bucketName] ?? null;
    const baselineThroughput = baseline.recommendations?.throughputByBucket?.[bucketName] ?? null;
    const baselineSmoothness = baseline.recommendations?.smoothnessByBucket?.[bucketName] ?? null;

    if (baselineThroughput && currentThroughput) {
      if (currentThroughput.metrics.successRate + thresholds.successRateDrop < baselineThroughput.metrics.successRate) {
        failures.push(`${bucketName} throughput success rate dropped`);
      }
      if (regressed(
        baselineThroughput.metrics.avgTimeToGraphAppliedMs,
        currentThroughput.metrics.avgTimeToGraphAppliedMs,
        thresholds.throughputAvgRegressionPct,
        thresholds.throughputAvgRegressionMs,
      )) {
        failures.push(`${bucketName} throughput avgTimeToGraphAppliedMs regressed`);
      }
    }

    if (baselineSmoothness && currentSmoothness) {
      if (currentSmoothness.metrics.successRate + thresholds.successRateDrop < baselineSmoothness.metrics.successRate) {
        failures.push(`${bucketName} smoothness success rate dropped`);
      }
      if (regressed(
        baselineSmoothness.metrics.avgMaxFrameGapMs,
        currentSmoothness.metrics.avgMaxFrameGapMs,
        thresholds.smoothnessAvgRegressionPct,
        thresholds.smoothnessAvgRegressionMs,
      )) {
        failures.push(`${bucketName} smoothness avgMaxFrameGapMs regressed`);
      }
      if (regressedCount(
        baselineSmoothness.metrics.avgLongFrameCount,
        currentSmoothness.metrics.avgLongFrameCount,
        thresholds.longFrameCountRegression,
      )) {
        failures.push(`${bucketName} smoothness avgLongFrameCount regressed`);
      }
    }
  }

  return {
    status: failures.length === 0 ? 'pass' : 'regression',
    baselinePath: path.relative(webDir, baselinePath),
    failures,
  };
}

function regressed(baselineValue, currentValue, pctThreshold, absoluteMsThreshold) {
  if (!Number.isFinite(baselineValue) || !Number.isFinite(currentValue)) return false;
  const allowed = Math.max(Math.abs(baselineValue) * pctThreshold, absoluteMsThreshold);
  return currentValue - baselineValue > allowed;
}

function regressedCount(baselineValue, currentValue, absoluteThreshold) {
  if (!Number.isFinite(baselineValue) || !Number.isFinite(currentValue)) return false;
  return currentValue - baselineValue > absoluteThreshold;
}

function buildMarkdownReport(latest) {
  const lines = [];
  lines.push('# Graph Stream Benchmark');
  lines.push('');
  lines.push(`- Generated: ${latest.generatedAt}`);
  lines.push(`- Candidates: ${latest.runner.candidateChunkSizes.map((value) => formatBytes(value)).join(', ')}`);
  lines.push(`- Fixture policy: ${latest.runner.selectedFixturePolicy}`);
  lines.push(`- Baseline: ${latest.runner.baselinePath}`);
  lines.push(`- Comparison: ${latest.comparison.status}`);
  if (latest.comparison.failures.length > 0) {
    lines.push(`- Failures: ${latest.comparison.failures.join('; ')}`);
  }
  lines.push('');
  lines.push('| Bucket | Samples | Languages | Throughput winner | Smoothness winner | Risks |');
  lines.push('| --- | ---: | --- | --- | --- | --- |');
  for (const bucket of bucketDefinitions) {
    const summary = latest.bucketSummaries[bucket.name];
    const throughput = latest.recommendations.throughputByBucket[bucket.name];
    const smoothness = latest.recommendations.smoothnessByBucket[bucket.name];
    lines.push(`| ${bucket.name} | ${summary.sampleCount} | ${summary.languages.join(', ') || '—'} | ${formatWinner(throughput)} | ${formatWinner(smoothness)} | ${summary.riskFlags.join(', ') || '—'} |`);
  }
  lines.push('');
  for (const bucket of bucketDefinitions) {
    const summary = latest.bucketSummaries[bucket.name];
    lines.push(`## ${bucket.name}`);
    lines.push('');
    lines.push('| Chunk size | Success | Avg graph applied | P95 graph applied | Avg max frame gap | Avg long frames |');
    lines.push('| --- | ---: | ---: | ---: | ---: | ---: |');
    for (const candidate of summary.candidates) {
      lines.push(`| ${formatBytes(candidate.chunkSize)} | ${formatPercent(candidate.metrics.successRate)} | ${formatMs(candidate.metrics.avgTimeToGraphAppliedMs)} | ${formatMs(candidate.metrics.p95TimeToGraphAppliedMs)} | ${formatMs(candidate.metrics.avgMaxFrameGapMs)} | ${formatCount(candidate.metrics.avgLongFrameCount)} |`);
    }
    lines.push('');
  }
  return `${lines.join('\n')}\n`;
}

function formatWinner(winner) {
  if (!winner) return '—';
  return `${formatBytes(winner.chunkSize)} (${formatMs(winner.metrics.avgTimeToGraphAppliedMs)} / ${formatMs(winner.metrics.avgMaxFrameGapMs)})`;
}

function formatPercent(value) {
  if (!Number.isFinite(value)) return '—';
  return `${(value * 100).toFixed(0)}%`;
}

function formatMs(value) {
  if (!Number.isFinite(value)) return '—';
  return `${value.toFixed(1)}ms`;
}

function formatCount(value) {
  if (!Number.isFinite(value)) return '—';
  return value.toFixed(2);
}

function bucketForBytes(bytes) {
  for (const bucket of bucketDefinitions) {
    if (bytes >= bucket.minBytes && (bucket.maxBytes == null || bytes < bucket.maxBytes)) {
      return bucket.name;
    }
  }
  return bucketDefinitions[bucketDefinitions.length - 1].name;
}

function bucketIndex(name) {
  return bucketDefinitions.findIndex((bucket) => bucket.name === name);
}

function formatBytes(bytes) {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(bytes % (1024 * 1024) === 0 ? 0 : 1)}MB`;
  return `${Math.round(bytes / 1024)}KB`;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function runCommand(command, args, options = {}) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env ?? process.env,
      stdio: options.stdio ?? 'inherit',
    });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolve(undefined);
        return;
      }
      reject(new Error(`${command} ${args.join(' ')} exited with code ${code}`));
    });
  });
}

async function stopProcess(child) {
  if (!child || child.exitCode != null) return;
  child.kill('SIGTERM');
  await Promise.race([
    new Promise((resolve) => child.once('close', resolve)),
    delay(5_000).then(async () => {
      child.kill('SIGKILL');
      await new Promise((resolve) => child.once('close', resolve));
    }),
  ]);
}

main().catch(async (error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
