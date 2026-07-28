import { readFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium, expect } from "@playwright/test";
import { fromScriptsDir } from "./project-paths.mjs";

const BASE_URL = process.env.TREEASE_LANDING_CAPTURE_BASE_URL ?? "http://localhost:4173";
const OUTPUT_DIR = fromScriptsDir(import.meta.url, "../static/landing");
const VIEWPORT = { width: 1200, height: 860 };
const EXAMPLE_JSON_PATH = fromScriptsDir(import.meta.url, "../../../example/simple.json");
const EXAMPLE_JSON_TEXT = readFileSync(EXAMPLE_JSON_PATH, "utf8").trim();
const EXAMPLE_JSON = JSON.parse(EXAMPLE_JSON_TEXT);
const YAML_IMPORT_TEXT = "user:\n  name: Alice\ncount: 42\n";
const TWO_MB_JSON_PATH = fromScriptsDir(import.meta.url, "../../../test/fixtures/json/2mb.1.json");
const TWO_MB_JSON_TEXT = readFileSync(TWO_MB_JSON_PATH, "utf8");
const CAPTURE_FILTER = (process.env.TREEASE_LANDING_CAPTURE_FILTER ?? "").trim();
const EXPORT_PREVIEW_JSON_TEXT = JSON.stringify(
  {
    title: "Example",
    count: 42,
    owner: {
      name: "Treease",
      region: "ap-singapore",
      active: true,
    },
    flags: {
      preview: true,
      compare: true,
      export: true,
    },
    items: [
      { id: 1, name: "alpha", enabled: true },
      { id: 2, name: "beta", enabled: false },
      { id: 3, name: "gamma", enabled: true },
      { id: 4, name: "delta", enabled: true },
      { id: 5, name: "omega", enabled: false },
    ],
  },
  null,
  2,
);

mkdirSync(OUTPUT_DIR, { recursive: true });

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function mulberry32(seed) {
  let state = seed >>> 0;
  return () => {
    state |= 0;
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function pick(rng, items) {
  return items[Math.floor(rng() * items.length)];
}

function buildCompareVariant() {
  const rng = mulberry32(20260621);
  const next = clone(EXAMPLE_JSON);

  next.object.float = Number((next.object.float + pick(rng, [2.5, 4.25, 8.75])).toFixed(3));
  next.object.bool = !next.object.bool;
  delete next.object.nil;
  next.object[`added_${pick(rng, ["note", "flag", "marker"])}`] = pick(rng, [
    "landing-demo",
    "compare-pass",
    "diff-ready",
  ]);

  next.table_without_header.splice(1, 1);
  next.table_without_header.push(pick(rng, ["delta", "omega", "preview"]));

  next.table_with_header[0].h2 += pick(rng, [7, 9, 12]);
  next.table_with_header.push({
    h1: pick(rng, [31, 34, 55]),
    h2: pick(rng, [32, 35, 56]),
    h3: pick(rng, [33, 36, 57]),
  });

  next.preview.color = pick(rng, ["#22c55e", "#f97316", "#0ea5e9"]);
  next.preview.unicode = "你好，Treease";
  next.preview.base64 = "SGVsbG8gdHJlZWFzZSBsYW5kaW5nIGRlbW8=";
  next.preview.jwt =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJkZW1vLXVzZXIiLCJyb2xlIjoiY29tcGFyZSIsImV4cCI6MTk0MDAwMDAwMH0.c2lnbmF0dXJl";
  next.preview.img = "https://treease.com/compare/icon.png";
  next.preview.uris[0] = "https%3A%2F%2Ftreease.com%2Fpreview%3Ffrom%3Dcompare%26step%3D1";
  next.preview.uris[1] =
    "https://treease.com/path?redirect=https%3A%2F%2Ftreease.com%2Fdone%3Fmode%3Dcompare";
  next.preview.added_url = "https://treease.com/docs?tab=compare&mode=inline";
  delete next.preview.time;

  return JSON.stringify(next, null, 2);
}

function buildContainerFixtureText() {
  return [
    '{"line":1,"skip":"first"}',
    '{"line":2,"nested":{"name":"Alice"}}',
    '{"line":3,"skip":"third"}',
  ].join("\n");
}

async function evaluateTreease(page, fn, payload) {
  return page.evaluate(
    ({ fnSource, payload: nextPayload }) => {
      const treease = window._treease;
      if (!treease) throw new Error("window._treease is unavailable");
      return new Function("treease", "payload", `return (${fnSource})(treease, payload);`)(
        treease,
        nextPayload,
      );
    },
    { fnSource: fn.toString(), payload },
  );
}

async function waitForEditorReady(page) {
  await expect(
    page
      .getByRole("button", { name: "Graph mode", exact: true })
      .or(page.getByRole("button", { name: "Text mode", exact: true })),
  ).toBeVisible({ timeout: 10_000 });
}

async function waitForGraphRendered(page) {
  await expect
    .poll(
      async () =>
        evaluateTreease(page, (treease) => {
          const state = treease.editor.getState();
          const graph = treease.graph.getInteractionState?.();
          return (
            state.graphAppliedRevision >= state.editorRevision &&
            state.editorRevision > 0 &&
            graph?.current === true &&
            graph?.pendingRenderWork === false
          );
        }),
      { timeout: 10_000 },
    )
    .toBe(true);
}

async function waitForImportIdle(page, timeout = 10_000) {
  await expect
    .poll(
      async () => evaluateTreease(page, (treease) => treease.editor.getState().fullEditUiState.phase),
      { timeout },
    )
    .toBe("idle");
}

async function waitForStreamProgressVisible(page, timeout = 10_000) {
  await expect
    .poll(
      async () =>
        evaluateTreease(page, (treease) => {
          const state = treease.graph.getStreamProgressState?.();
          return state?.visible === true && state?.phase ? state.phase : null;
        }),
      { timeout },
    )
    .not.toBeNull();
}

async function pinVisibleProgressOverlay(page, holdMs = 1_600) {
  await page.evaluate((nextHoldMs) => {
    const progressBar = document.querySelector('[role="progressbar"]');
    if (!(progressBar instanceof HTMLElement)) {
      throw new Error("progressbar not found for pinning");
    }

    const shell =
      progressBar.closest("div.absolute") ??
      progressBar.parentElement ??
      progressBar;
    if (!(shell instanceof HTMLElement)) {
      throw new Error("progress overlay shell not found");
    }

    document.querySelector('[data-testid="landing-progress-overlay-pin"]')?.remove();

    const rect = shell.getBoundingClientRect();
    const clone = shell.cloneNode(true);
    if (!(clone instanceof HTMLElement)) {
      throw new Error("unable to clone progress overlay shell");
    }

    clone.setAttribute("data-testid", "landing-progress-overlay-pin");
    clone.style.position = "fixed";
    clone.style.left = `${rect.left}px`;
    clone.style.top = `${rect.top}px`;
    clone.style.width = `${rect.width}px`;
    clone.style.height = `${rect.height}px`;
    clone.style.right = "auto";
    clone.style.bottom = "auto";
    clone.style.zIndex = "999";
    clone.style.pointerEvents = "none";
    clone.style.margin = "0";
    document.body.append(clone);

    window.setTimeout(() => clone.remove(), nextHoldMs);
  }, holdMs);
}

async function waitForRightPreviewText(page, expected, timeout = 5_000) {
  await expect(page.getByTestId("monaco-right-editor")).toBeVisible({ timeout });
  await expect
    .poll(
      async () => evaluateTreease(page, (treease) => treease.editor.getValue("right-editor")),
      { timeout },
    )
    .toContain(expected);
}

async function setLanguage(page, languageId) {
  await evaluateTreease(
    page,
    (treease, nextLanguage) => {
      treease.editor.setLanguageId(nextLanguage);
    },
    languageId,
  );
}

async function setEditorContent(page, { sourceText, language }) {
  if (language) {
    await setLanguage(page, language);
  }
  await evaluateTreease(
    page,
    (treease, args) => {
      treease.editor.setValueExact?.("source-editor", args.sourceText);
    },
    { sourceText },
  );
}

async function setMonacoValue(page, hookId, value) {
  await evaluateTreease(
    page,
    (treease, args) => {
      treease.editor.setValue(args.hookId, args.value);
    },
    { hookId, value },
  );
}

async function setMonacoPosition(page, hookId, lineNumber, column) {
  await evaluateTreease(
    page,
    (treease, payload) => {
      treease.editor.setPosition(payload.hookId, payload.lineNumber, payload.column);
    },
    { hookId, lineNumber, column },
  );
}

async function setMonacoPositionByText(page, hookId, searchText) {
  await evaluateTreease(
    page,
    (treease, payload) => {
      const text = treease.editor.getValue(payload.hookId);
      const idx = text.indexOf(payload.searchText);
      if (idx < 0) throw new Error(`Text not found: ${payload.searchText}`);
      const before = text.slice(0, idx);
      const lineNumber = before.split("\n").length;
      const lastNewline = before.lastIndexOf("\n");
      const column = idx - (lastNewline + 1) + 1;
      treease.editor.setPosition(payload.hookId, lineNumber, column);
    },
    { hookId, searchText },
  );
}

async function chooseFile(
  page,
  { triggerLabel, inputLabel, fileName, content, mimeType = "text/plain" },
) {
  await page.getByRole("button", { name: triggerLabel, exact: true }).click();
  await page.getByLabel(inputLabel).setInputFiles({
    name: fileName,
    mimeType,
    buffer: Buffer.from(content),
  });
}

async function dropFile(page, { targetTestId, fileName, content, mimeType = "text/plain" }) {
  const target = page.getByTestId(targetTestId);
  await expect(target).toBeVisible({ timeout: 5_000 });
  await target.evaluate(
    (node, payload) => {
      const dataTransfer = new DataTransfer();
      const file = new File([payload.content], payload.fileName, { type: payload.mimeType });
      dataTransfer.items.add(file);
      node.dispatchEvent(new DragEvent("dragover", { bubbles: true, cancelable: true, dataTransfer }));
      node.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }));
    },
    { fileName, content, mimeType },
  );
}

async function openTextMode(page) {
  await page.getByRole("button", { name: "Text mode", exact: true }).click();
  await expect(page.getByTestId("monaco-right-editor")).toBeVisible({ timeout: 5_000 });
}

async function ensureGraphMode(page) {
  const graphModeButton = page.getByRole("button", { name: "Graph mode", exact: true });
  if (await graphModeButton.isVisible().catch(() => false)) {
    await graphModeButton.click();
  } else {
    await expect(page.getByRole("button", { name: "Text mode", exact: true })).toBeVisible({
      timeout: 5_000,
    });
  }
}

async function setSplitRatio(page, ratio = 0.5) {
  await expect(page.getByTestId("left-pane")).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId("right-pane")).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId("splitter-divider")).toBeVisible({ timeout: 5_000 });

  await page.evaluate((nextRatio) => {
    const layout = document.querySelector(".app-split-layout");
    const leftPane = document.querySelector('[data-testid="left-pane"]');
    const rightPane = document.querySelector('[data-testid="right-pane"]');
    const divider = document.querySelector('[data-testid="splitter-divider"]');
    if (!layout || !leftPane || !rightPane || !divider) {
      throw new Error("Unable to resolve split layout nodes");
    }

    const dividerWidth = divider.getBoundingClientRect().width || 10;
    const usableWidth = layout.clientWidth - dividerWidth;
    const leftWidth = Math.round(usableWidth * nextRatio);
    const rightWidth = Math.max(usableWidth - leftWidth, 0);

    leftPane.style.width = `${leftWidth}px`;
    rightPane.style.width = `${rightWidth}px`;
    divider.style.left = `${leftWidth}px`;

    window.dispatchEvent(new Event("resize"));
  }, ratio);

  await page.waitForTimeout(220);
}

async function syncRightToSource(page) {
  const source = await evaluateTreease(page, (treease) => treease.editor.getState().sourceText);
  await setMonacoValue(page, "right-editor", source);
}

async function openMonacoHover(page, { hookId = "source-editor", lineNumber, column, hoverText }) {
  await setMonacoPosition(page, hookId, lineNumber, column);

  const point = await page
    .getByTestId(`monaco-${hookId}`)
    .last()
    .evaluate((node, text) => {
      const spans = Array.from(node.querySelectorAll(".view-lines .view-line span"));
      const exact = spans.find((span) => (span.textContent ?? "").trim() === String(text));
      const quoted = spans.find((span) => (span.textContent ?? "").trim() === `"${String(text)}"`);
      const partials = spans
        .filter((span) => (span.textContent ?? "").includes(String(text)))
        .sort((left, right) => (left.textContent ?? "").length - (right.textContent ?? "").length);
      const target = exact ?? quoted ?? partials[0];
      if (!target) throw new Error(`Unable to find Monaco token containing "${text}"`);
      const rect = target.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
        rect: {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        },
      };
    }, hoverText);

  await page.mouse.move(point.x, point.y);
  return point.rect;
}

async function readRootGraphProbes(page) {
  return evaluateTreease(page, (treease) =>
    (treease.graph.getClickProbeTargets?.("root") ?? []).map((probe) => ({
      id: String(probe.id ?? ""),
      target: probe.target,
      text: probe.cell?.text ?? "",
      path: (probe.cell?.path ?? [])
        .map((segment) =>
          typeof segment?.key === "string" && segment.key.length > 0
            ? segment.key
            : typeof segment?.index === "number"
              ? `[${segment.index}]`
              : "",
        )
        .filter((segment) => segment.length > 0),
      coord:
        typeof probe.coord?.x === "number" && typeof probe.coord?.y === "number"
          ? { x: Number(probe.coord.x), y: Number(probe.coord.y) }
          : null,
      rect:
        typeof probe.rect?.left === "number" &&
        typeof probe.rect?.top === "number" &&
        typeof probe.rect?.width === "number" &&
        typeof probe.rect?.height === "number"
          ? {
              left: Number(probe.rect.left),
              top: Number(probe.rect.top),
              width: Number(probe.rect.width),
              height: Number(probe.rect.height),
            }
          : null,
    })),
  );
}

async function commitGraphProbeValue(page, matcher, nextValue) {
  const probe = (await readRootGraphProbes(page)).find(matcher);
  if (!probe?.id) {
    throw new Error("matching graph probe missing for commit");
  }
  const committed = await evaluateTreease(
    page,
    async (treease, payload) => {
      return (await treease.graph.commitProbe(payload.probeId, payload.text)) ?? false;
    },
    { probeId: probe.id, text: nextValue },
  );
  if (!committed) {
    throw new Error(`graph commit failed for ${probe.id}`);
  }
}

async function getUnionClip(page, boxes, padding = 18) {
  const visibleBoxes = boxes.filter(Boolean);
  if (visibleBoxes.length === 0) {
    throw new Error("No bounding boxes available for capture");
  }

  const minLeft = Math.min(...visibleBoxes.map((box) => box.x));
  const minTop = Math.min(...visibleBoxes.map((box) => box.y));
  const maxRight = Math.max(...visibleBoxes.map((box) => box.x + box.width));
  const maxBottom = Math.max(...visibleBoxes.map((box) => box.y + box.height));

  const viewport = page.viewportSize();
  if (!viewport) {
    throw new Error("Viewport size unavailable");
  }

  const x = Math.max(Math.floor(minLeft - padding), 0);
  const y = Math.max(Math.floor(minTop - padding), 0);
  const right = Math.min(Math.ceil(maxRight + padding), viewport.width);
  const bottom = Math.min(Math.ceil(maxBottom + padding), viewport.height);

  return {
    x,
    y,
    width: Math.max(right - x, 1),
    height: Math.max(bottom - y, 1),
  };
}

function expandBox(box, { left = 0, top = 0, right = 0, bottom = 0 }) {
  return {
    x: box.x - left,
    y: box.y - top,
    width: box.width + left + right,
    height: box.height + top + bottom,
  };
}

async function captureTarget(page, outputPath, target) {
  if (target.type === "full") {
    await page.screenshot({ path: outputPath, fullPage: false });
    return;
  }

  if (target.type === "locator") {
    await target.locator.screenshot({ path: outputPath });
    return;
  }

  if (target.type === "union") {
    const boxes = [];
    for (const item of target.items) {
      if (item.kind === "locator") {
        await expect(item.locator).toBeVisible({ timeout: 5_000 });
        boxes.push(await item.locator.boundingBox());
      } else {
        boxes.push(await item.box(page));
      }
    }
    const clip = await getUnionClip(page, boxes, target.padding ?? 18);
    await page.screenshot({ path: outputPath, clip });
    return;
  }

  throw new Error(`Unsupported capture target: ${target.type}`);
}

async function createSession(name, recordVideo = false, viewport = VIEWPORT) {
  const videoDir = recordVideo ? mkdtempSync(join(tmpdir(), `treease-landing-${name}-`)) : null;
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 1,
    ...(recordVideo
      ? {
          recordVideo: {
            dir: videoDir,
            size: viewport,
          },
        }
      : {}),
  });
  const page = await context.newPage();
  await page.goto(`${BASE_URL}/editor`, { waitUntil: "networkidle" });
  await waitForEditorReady(page);
  return { browser, context, page, videoDir };
}

async function finalizeSession(session, outputBaseName, recordVideo = false) {
  const video = recordVideo ? session.page.video() : null;
  await session.context.close();
  await session.browser.close();

  if (!recordVideo || !video || !session.videoDir) return;

  const webmPath = await video.path();
  const mp4Path = resolve(OUTPUT_DIR, `${outputBaseName}.mp4`);
  execFileSync(
    "ffmpeg",
    ["-y", "-i", webmPath, "-an", "-movflags", "+faststart", "-pix_fmt", "yuv420p", mp4Path],
    { stdio: "inherit" },
  );
  rmSync(session.videoDir, { recursive: true, force: true });
}

async function runCapture(definition) {
  const session = await createSession(definition.name, definition.recordVideo, definition.viewport);
  try {
    const outcome = (await definition.prepare(session.page)) ?? {};
    const settleMs = outcome.settleMs ?? definition.settleMs ?? 250;
    const outputBaseName = definition.outputBaseName ?? definition.name;
    if (outcome.captureBeforeAfterHook) {
      await session.page.waitForTimeout(settleMs);
      await captureTarget(
        session.page,
        resolve(OUTPUT_DIR, `${outputBaseName}.png`),
        outcome.target ?? definition.target(session.page, outcome),
      );
      if (typeof outcome.afterCapture === "function") {
        await outcome.afterCapture(session.page);
      }
    } else {
      await session.page.waitForTimeout(settleMs);
      if (typeof outcome.beforeCapture === "function") {
        await outcome.beforeCapture(session.page);
      }
      await captureTarget(
        session.page,
        resolve(OUTPUT_DIR, `${outputBaseName}.png`),
        outcome.target ?? definition.target(session.page, outcome),
      );
      if (typeof outcome.afterCapture === "function") {
        await outcome.afterCapture(session.page);
      }
    }
    await finalizeSession(session, outputBaseName, definition.recordVideo);
  } catch (error) {
    await session.context.close().catch(() => {});
    await session.browser.close().catch(() => {});
    if (session.videoDir) {
      rmSync(session.videoDir, { recursive: true, force: true });
    }
    throw error;
  }
}

const heroCaptures = [
  {
    name: "graph",
    outputBaseName: "hero-demo-graph",
    recordVideo: true,
    async prepare(page) {
      await setEditorContent(page, {
        sourceText: EXAMPLE_JSON_TEXT,
        language: "json",
      });
      await waitForGraphRendered(page);
      await page.getByRole("button", { name: "Search graph", exact: true }).click();
      const input = page.getByRole("textbox", { name: "Search graph", exact: true });
      await expect(input).toBeVisible({ timeout: 5_000 });

      await input.fill("preview");
      await expect(
        page
          .getByRole("button", { name: "Graph search result $.preview.color", exact: true })
          .first(),
      ).toBeVisible({ timeout: 5_000 });
      await page.waitForTimeout(450);

      await input.fill("redirect=");
      const uriResult = page
        .getByRole("button", {
          name: "Graph search result $.preview.uris[1]",
          exact: true,
        })
        .first();
      await expect(uriResult).toBeVisible({ timeout: 5_000 });
      await page.waitForTimeout(450);
      await uriResult.click();
      await page.waitForTimeout(900);
    },
    target() {
      return { type: "full" };
    },
  },
  {
    name: "preview",
    outputBaseName: "hero-demo-preview",
    recordVideo: true,
    async prepare(page) {
      await setEditorContent(page, {
        sourceText: EXAMPLE_JSON_TEXT,
        language: "json",
      });
      await setSplitRatio(page, 0.5);

      const hoverSteps = [
        { lineNumber: 15, column: 16, hoverText: "#4f46e5" },
        { lineNumber: 16, column: 15, hoverText: "2026-04-13T10:00:00Z" },
        { lineNumber: 17, column: 18, hoverText: "你好" },
        { lineNumber: 18, column: 17, hoverText: "aHR0cHM6Ly90cmVlYXNlLmRldi9wcmV2aWV3" },
        { lineNumber: 20, column: 15, hoverText: "https://treease.com/icon.png" },
        {
          lineNumber: 21,
          column: 18,
          hoverText: "https%3A%2F%2Ftreease.com%2Fpreview%3Ffrom%3Dhover",
        },
        { lineNumber: 22, column: 18, hoverText: "redirect=https%3A%2F%2Ftreease.com%2Fdone" },
      ];

      for (const step of hoverSteps) {
        await openMonacoHover(page, step);
        await expect(page.locator(".monaco-hover:not(.hidden)").first()).toBeVisible({
          timeout: 5_000,
        });
        await page.waitForTimeout(520);
      }

      await openMonacoHover(page, { lineNumber: 15, column: 16, hoverText: "#4f46e5" });
      await expect(page.locator(".monaco-hover:not(.hidden)").first()).toBeVisible({
        timeout: 5_000,
      });
      return { settleMs: 160 };
    },
    target() {
      return { type: "full" };
    },
  },
  {
    name: "compare",
    outputBaseName: "hero-demo-compare",
    recordVideo: true,
    async prepare(page) {
      await setEditorContent(page, { sourceText: EXAMPLE_JSON_TEXT, language: "json" });
      await openTextMode(page);
      await setSplitRatio(page, 0.5);
      await syncRightToSource(page);
      await page.waitForTimeout(300);

      const compareText = buildCompareVariant();
      await setMonacoValue(page, "right-editor", compareText);
      await page.waitForTimeout(600);
      await page.getByRole("button", { name: "Compare", exact: true }).click();
      await expect
        .poll(async () =>
          Number(
            (await page
              .getByTestId("right-panel-dropzone")
              .getAttribute("data-compare-highlight-count")) ?? "0",
          ),
        )
        .toBeGreaterThan(0);
      await page.waitForTimeout(1200);
    },
    target() {
      return { type: "full" };
    },
  },
];

const featureCaptures = [
  {
    name: "feature-import",
    viewport: { width: 980, height: 360 },
    async prepare(page) {
      await setSplitRatio(page, 0.34);
      await chooseFile(page, {
        triggerLabel: "Import",
        inputLabel: "Import file input",
        fileName: "sample.yaml",
        content: YAML_IMPORT_TEXT,
      });
      await expect(page.getByText("Imported sample.yaml")).toBeVisible({ timeout: 5_000 });
      await expect
        .poll(async () => evaluateTreease(page, (treease) => treease.editor.getState().languageId))
        .toBe("yaml");
    },
    target(page) {
      return {
        type: "union",
        padding: 20,
        items: [
          { kind: "locator", locator: page.getByRole("button", { name: "Import", exact: true }) },
          { kind: "locator", locator: page.getByText("Imported sample.yaml") },
          { kind: "locator", locator: page.getByTestId("source-editor-region") },
          { kind: "locator", locator: page.getByTestId("graph-viewer-canvas") },
        ],
      };
    },
  },
  {
    name: "feature-format",
    viewport: { width: 900, height: 340 },
    async prepare(page) {
      await setSplitRatio(page, 0.74);
      await setEditorContent(page, {
        sourceText:
          '{"table_with_header":[{"h1":11}],"table_without_header":["a","b"],"object":{"int":42},"preview":{"color":"#4f46e5"}}',
        language: "json",
      });
      const commandInput = page.getByRole("textbox", { name: "Search command", exact: true });
      await commandInput.fill("sort");
      await commandInput.press("Enter");
      await commandInput.press("Enter");
      await expect
        .poll(async () => evaluateTreease(page, (treease) => treease.editor.getState().sourceText))
        .toContain('"object"');
    },
    target(page) {
      return {
        type: "union",
        items: [
          {
            kind: "box",
            box: async () =>
              expandBox(await page.getByTestId("source-editor-region").boundingBox(), {
                right: 96,
              }),
          },
          {
            kind: "box",
            box: async () =>
              expandBox(
                await page
                  .getByRole("textbox", { name: "Search command", exact: true })
                  .boundingBox(),
                {
                  left: 18,
                  right: 18,
                  top: 18,
                  bottom: 18,
                },
              ),
          },
        ],
      };
    },
  },
  {
    name: "feature-container",
    viewport: { width: 920, height: 430 },
    async prepare(page) {
      await setSplitRatio(page, 0.34);
      const sourceText = buildContainerFixtureText();
      await setEditorContent(page, {
        sourceText,
        language: "json",
      });
      await ensureGraphMode(page);
      await setMonacoPositionByText(page, "source-editor", "Alice");
      await expect
        .poll(async () =>
          (await readRootGraphProbes(page)).map((probe) => probe.path.join(".")).join("|"),
        )
        .toContain("nested.name");
    },
    target(page) {
      return {
        type: "union",
        padding: 20,
        items: [
          { kind: "locator", locator: page.getByTestId("source-editor-region") },
          { kind: "locator", locator: page.getByTestId("graph-viewer-canvas") },
        ],
      };
    },
  },
  {
    name: "feature-reveal",
    viewport: { width: 920, height: 430 },
    async prepare(page) {
      await setSplitRatio(page, 0.34);
      await setEditorContent(page, {
        sourceText:
          '{"user":{"name":"Alice","role":"admin"},"count":42,"preview":{"color":"#4f46e5"}}',
        language: "json",
      });
      await waitForGraphRendered(page);
      await page.getByRole("button", { name: "Search graph", exact: true }).click();
      const input = page.getByRole("textbox", { name: "Search graph", exact: true });
      await input.fill("role");
      const result = page
        .getByRole("button", { name: "Graph search result $.user.role", exact: true })
        .first();
      await expect(result).toBeVisible({ timeout: 5_000 });
      await result.click();
      await expect(page.getByTestId("tree-path-crumb-0")).toBeVisible({ timeout: 5_000 });
      await expect(page.getByTestId("tree-path-crumb-2")).toBeVisible({ timeout: 5_000 });
    },
    target(page) {
      return {
        type: "union",
        padding: 20,
        items: [
          { kind: "locator", locator: page.getByTestId("source-editor-region") },
          { kind: "locator", locator: page.getByTestId("graph-viewer-canvas") },
          { kind: "locator", locator: page.locator('nav[aria-label="breadcrumb"]') },
        ],
      };
    },
  },
  {
    name: "feature-preview",
    viewport: { width: 980, height: 360 },
    async prepare(page) {
      await setEditorContent(page, {
        sourceText: '{"preview":{"color":"#4f46e5"}}',
        language: "json",
      });
      const tokenRect = await openMonacoHover(page, {
        lineNumber: 1,
        column: 22,
        hoverText: "#4f46e5",
      });
      await expect(page.locator(".monaco-hover:not(.hidden)").first()).toBeVisible({
        timeout: 5_000,
      });
      return {
        tokenRect: {
          x: tokenRect.left - 120,
          y: tokenRect.top - 70,
          width: tokenRect.width + 220,
          height: tokenRect.height + 150,
        },
      };
    },
    target(page, outcome) {
      return {
        type: "union",
        padding: 18,
        items: [
          {
            kind: "box",
            box: async () => outcome.tokenRect,
          },
          { kind: "locator", locator: page.locator(".monaco-hover:not(.hidden)").first() },
        ],
      };
    },
  },
  {
    name: "feature-sync",
    viewport: { width: 980, height: 360 },
    async prepare(page) {
      await setSplitRatio(page, 0.52);
      await setEditorContent(page, {
        sourceText: '{"user":{"name":"Alice","role":"admin"},"count":42}',
        language: "json",
      });
      await waitForGraphRendered(page);
      await commitGraphProbeValue(
        page,
        (probe) =>
          probe.target === "value" &&
          probe.text === "Alice" &&
          probe.path.join(".") === "user.name",
        "Carol",
      );
      await expect
        .poll(async () => evaluateTreease(page, (treease) => treease.editor.getState().sourceText))
        .toContain('"Carol"');
    },
    target(page) {
      return {
        type: "union",
        padding: 20,
        items: [
          { kind: "locator", locator: page.getByTestId("source-editor-region") },
          { kind: "locator", locator: page.getByTestId("graph-viewer-canvas") },
        ],
      };
    },
  },
];

const workflowCaptures = [
  {
    name: "workflow-progress",
    outputBaseName: "workflow-progress",
    viewport: { width: 780, height: 486 },
    recordVideo: true,
    async prepare(page) {
      await setSplitRatio(page, 0.4);
      await dropFile(page, {
        targetTestId: "source-editor-region",
        fileName: "2mb.json",
        content: TWO_MB_JSON_TEXT,
        mimeType: "application/json",
      });
      await waitForStreamProgressVisible(page, 10_000);
      const progressBar = page.getByRole("progressbar");
      await expect(progressBar).toBeVisible({ timeout: 5_000 });
      expandBox(await progressBar.boundingBox(), {
        left: 16,
        top: 52,
        right: 16,
        bottom: 28,
      });
      await pinVisibleProgressOverlay(page, 1_600);
      await page.waitForTimeout(260);
      return {
        settleMs: 0,
        captureBeforeAfterHook: true,
        target: { type: "full" },
        afterCapture: async (capturePage) => {
          await waitForImportIdle(capturePage, 10_000);
          await capturePage.waitForTimeout(320);
        },
      };
    },
  },
  {
    name: "workflow-export",
    outputBaseName: "workflow-export",
    viewport: { width: 780, height: 400 },
    async prepare(page) {
      await setSplitRatio(page, 0.46);
      await setEditorContent(page, {
        sourceText: EXPORT_PREVIEW_JSON_TEXT,
        language: "json",
      });
      await page.getByRole("button", { name: "Export", exact: true }).click();
      const panel = page.getByTestId("export-panel");
      await expect(panel).toBeVisible({ timeout: 5_000 });
      await panel.getByRole("button", { name: "Export format", exact: true }).click();
      await page.getByRole("option", { name: "YAML", exact: true }).click();
      await page.getByRole("button", { name: "Preview export result", exact: true }).click();
      await waitForRightPreviewText(page, "title: Example");
      await expect(page.getByText("Previewed JSON to YAML")).toBeVisible({ timeout: 5_000 });
    },
    target() {
      return { type: "full" };
    },
  },
];

for (const definition of [...heroCaptures, ...featureCaptures, ...workflowCaptures].filter((item) =>
  CAPTURE_FILTER.length === 0
    ? true
    : item.name.includes(CAPTURE_FILTER) || (item.outputBaseName ?? "").includes(CAPTURE_FILTER),
)) {
  // eslint-disable-next-line no-await-in-loop
  await runCapture(definition);
}
