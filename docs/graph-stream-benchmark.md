---
summary: "Frozen scope, commands, output files, and regression thresholds for the Web graph stream benchmark."
read_when:
  - Evaluating graph stream chunk-size, throughput, or smoothness regressions
  - Refreshing or explaining the Web graph stream benchmark baseline
---

# Graph Stream Benchmark

## Purpose

- Compare different graph stream chunk sizes through the real `apps/web` page path.
- Input is fixed to raw valid fixtures from the repository-root `test/fixtures/json/`, `test/fixtures/toml/`, and `test/fixtures/yaml/` directories.
- The output is not a per-case speed result, but bucket-level throughput and smoothness recommendations and regression comparisons.

## Frozen Scope

- Candidate chunk sizes: `16KB`, `32KB`, `64KB`, `128KB`, `256KB` (`512KB` tested equal to `256KB` and offered no additional benefit).
- Fixture selection: **one largest valid raw fixture** for each size bucket and language.
- Current buckets: `<256KB`, `256KB-1MB`, `1MB-4MB`, `4MB-8MB`, `>=8MB`.
- Long-frame threshold: `50ms`.
- Very-long-frame threshold: `100ms`.
- Smoothness recommendations sort first by long-frame count, then by maximum frame gap, so a one-off max-gap fluctuation does not mask sustained jank.
- Long-frame-count regressions tolerate four frames. This metric varies substantially for a single-fixture, single-language bucket, but remains a gate together with throughput and maximum frame gap.
- Per-case timeout: `45s`.

## Commands

```bash
cd apps/web && pnpm bench:graph-stream
```

- The command first runs `pnpm wasm:sync`, then starts a Vite dev server for each of the five candidate chunk sizes, drives real Chromium to open `/editor`, and samples it.
- When it finishes, the command compares the current result with the frozen baseline and exits nonzero if a threshold is exceeded.
- To explicitly refresh the baseline, run:

```bash
cd apps/web && pnpm bench:graph-stream -- --update-baseline
```

## Output Files

- Latest run: `apps/web/.tmp/graph-stream-benchmark/latest.json`
- Human-readable summary: `apps/web/.tmp/graph-stream-benchmark/latest.md`
- Frozen baseline: `apps/web/benchmarks/graph-stream-baseline.json`

`latest.json` has these fixed top-level fields:

- `schemaVersion`
- `generatedAt`
- `runner`
- `thresholds`
- `fixtureInventory`
- `candidateResults`
- `bucketSummaries`
- `recommendations`
- `comparison`

Compare future optimizations using this schema, not temporary tables or manually defined scopes.

## Key Metrics

- `timeToFirstPartialMs`
- `timeToDoneMs`
- `timeToGraphAppliedMs`
- `chunkCount`
- `progressEventCount`
- `applyDeltaCount`
- `maxApplyDeltaMs`
- `maxFrameGapMs`
- `p95FrameGapMs`
- `longFrameCount`
- `veryLongFrameCount`

## Recommendation Rules

- Throughput: consider `successRate` first, then `avg(timeToGraphAppliedMs)`, then `p95(timeToGraphAppliedMs)`.
- Smoothness: consider `successRate` first, then `avg(maxFrameGapMs)`, then `avg(longFrameCount)`, then `avg(maxApplyDeltaMs)`.
- If the throughput and smoothness winners differ, report `split-recommendation`.

## Regression Thresholds

After reading `apps/web/benchmarks/graph-stream-baseline.json`, `pnpm bench:graph-stream` compares bucket winners:

- `successRate` must not decrease.
- Throughput: a degradation in `avg(timeToGraphAppliedMs)` greater than `max(35%, 75ms)` is a regression.
- Smoothness: a degradation in `avg(maxFrameGapMs)` greater than `max(35%, 16ms)` is a regression.
- Smoothness: an increase in `avg(longFrameCount)` greater than `2` is a regression.

The currently frozen baseline is v2: `apps/web/benchmarks/graph-stream-baseline.json`.

## Current Frozen Recommendation (v2)

Based on the 2026-06-02 benchmark (5 candidates × 3 languages × 4 buckets; the >=8MB bucket completed reliably for the first time):

- `<256KB`：throughput `128KB`（132.6ms），smoothness `16KB`（30.9ms）
- `256KB-1MB`: `64KB` wins both throughput and smoothness (3928.7ms / 1075.0ms)
- `1MB-4MB`：throughput `256KB`（5766.9ms），smoothness `64KB`（1208.4ms / 18 long frames）
- `>=8MB`：throughput `256KB`（7250.8ms），smoothness `128KB`（408.3ms）

**The production default has been updated to 128KB** because:

- It is the most balanced single-value choice across all buckets.
- It is 30% faster than 64KB for > =8MB and 12% faster for 1MB-4MB.
- It is tied with 64KB for <256KB (132.6ms vs 136.2ms).
- It is slightly slower than 64KB for 256KB-1MB (5518.5ms vs 3928.7ms), but remains within an acceptable range.

## Current Runtime Chunk-Size Policy

`apps/web/src/lib/graph-stream/chunk-size-policy.ts` centrally determines Graph stream chunk size. Normal GraphViewer renders and same-language file-import graph jobs share this runtime policy:

| Input size  | chunk size | Rationale                                                                                                                                                                  |
| ----------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| < 256KB     | 128KB      | Best throughput (124.2ms), with comparable smoothness                                                                                                                      |
| 256KB - 1MB | 64KB       | Wins both throughput and smoothness (4033ms / 1092ms)                                                                                                                      |
| 1MB - 4MB   | 128KB      | Near-best throughput (5675ms), best smoothness (1192ms)                                                                                                                    |
| 4MB - 10MB  | 256KB      | Continues to use the conclusion for the large-file range covered by the frozen benchmark                                                                                   |
| > 10MB      | 1MB        | An additional runtime heuristic branch; it exceeds the frozen benchmark's five-candidate limit and must be evaluated separately rather than mixed with the frozen baseline |

The frozen benchmark still compares only the five candidates `16KB`, `32KB`, `64KB`, `128KB`, and `256KB`. The runtime `>10MB → 1MB` branch is a later policy adjustment and does not mean the frozen baseline covers the `1MB` candidate.

The readable-file-import graph path sends `BinaryChunk` to avoid first UTF-8-decoding on the browser main thread and then sending `TextChunk` across the Worker/WASM boundary. In-memory full-text rendering still sends `TextChunk` because the source is already a JS string.

## Risk Markers

- `insufficient-fixtures`: the current bucket lacks enough samples for a stable recommendation.
- `low-confidence`: the current bucket has too few valid samples for a reliable conclusion.
- `low-language-diversity`: the current bucket has overly narrow language coverage.
- `json-only`: only JSON raw fixtures remain available for the current bucket.
- `split-recommendation`: throughput and smoothness have different winners.

## Remaining Boundaries

- `4MB-8MB`: the repository raw corpus currently has no samples, so this bucket is recorded as empty and does not participate in threshold conclusions.
- `>=8MB`: currently only `jsonexamples__semanticscholar-corpus.1.json` is available as a raw sample. Although the frozen benchmark covers five candidates and produced results, the sample count remains too low; it is an observation only and must not be generalized as a stable large-file baseline.
- `1MB-4MB`: the current bucket has only two samples and still exhibits a 50% success rate. The winner is frozen, but it can be used only for regression monitoring and must not be described as a stable optimization for all large documents.
