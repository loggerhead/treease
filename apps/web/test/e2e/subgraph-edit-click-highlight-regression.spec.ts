import { readFileSync } from "node:fs";
import { expect, test, type Page } from "./fixtures";
import {
  clickGraphProbeAt,
  clickSubgraphWorkspaceProbeAt,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readGraphClickProbes,
  readGraphHighlight,
  readSubgraphWorkspaceClickProbes,
  revealGraphPath,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
  waitForGraphRendered,
  waitForSubgraphSettled,
} from "./utils";

const trajectoryFixture = readFileSync(
  new URL("../../../../test/fixtures/json/trajectory.1.json", import.meta.url),
  "utf8",
);

const basicInfoPath = ["agent_steps", "[0]", "steps", "[6]", "basic_info"];
const durationPath = [...basicInfoPath, "duration"];
const basicInfoPathKey = "k:agent_steps|i:0|k:steps|i:6|k:basic_info";
const durationPathKey = `${basicInfoPathKey}|k:duration`;

function matchesPath(probe: { path: string[] }, path: string[]) {
  return probe.path.join(".") === path.join(".");
}

async function readDurationFromSource(page: Page): Promise<unknown> {
  try {
    const source = JSON.parse(await getMonacoValue(page, "source-editor"));
    return source.agent_steps?.[0]?.steps?.[6]?.basic_info?.duration ?? null;
  } catch {
    return null;
  }
}

async function sampleRenderedTokenColors(
  page: Page,
  hookId: string,
  tokenText: string,
  frames = 12,
): Promise<string[]> {
  await expect(page.getByTestId(`monaco-${hookId}`)).toBeVisible();
  return page.evaluate(
    async ({ hookId: nextHookId, tokenText: nextTokenText, frames: nextFrames }) => {
      const colors: string[] = [];
      for (let frame = 0; frame < nextFrames; frame += 1) {
        const root = document.querySelector(`[data-testid="monaco-${nextHookId}"]`);
        const token = Array.from(root?.querySelectorAll(".view-lines .view-line span") ?? []).find(
          (node) =>
            node.textContent === nextTokenText &&
            Array.from(node.classList).some((className) => className.startsWith("mtk")),
        ) as HTMLElement | undefined;
        colors.push(token ? getComputedStyle(token).color : "missing");
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      }
      return colors;
    },
    { hookId, tokenText, frames },
  );
}

async function ensureGraphMode(page: Page) {
  const graphModeButton = page.getByRole("button", {
    name: "Graph mode",
    exact: true,
  });
  if (await graphModeButton.isVisible().catch(() => false)) {
    await graphModeButton.click();
  }
}

test.setTimeout(60_000);

test("editing a nested trajectory scalar preserves blue string syntax highlighting across the subgraph and graph click", async ({
  page,
}, testInfo) => {
  testInfo.annotations.push({
    type: "allow-browser-error",
    description: "v1/usage",
  });
  await page.goto("/editor");
  await waitForEditorReady(page);
  await ensureGraphMode(page);
  await setEditorContent(page, {
    sourceText: trajectoryFixture,
    language: "json",
  });
  await waitForGraphRendered(page, 30_000);
  await revealGraphPath(
    page,
    [
      { key: "agent_steps" },
      { index: 0 },
      { key: "steps" },
      { index: 6 },
      { key: "basic_info" },
    ],
    { target: "value", navigate: true },
  );

  await expect
    .poll(
      async () =>
        (await readGraphClickProbes(page)).some(
          (probe) => matchesPath(probe, basicInfoPath) && probe.coord,
        ),
      { timeout: 10_000 },
    )
    .toBe(true);
  const graphProbes = await readGraphClickProbes(page);
  const basicInfoProbe = graphProbes.find(
    (probe) => matchesPath(probe, basicInfoPath) && probe.coord,
  );
  expect(
    basicInfoProbe,
    `main graph basic_info cell; matching probes: ${JSON.stringify(
      graphProbes.filter((probe) =>
        probe.path.join(".").includes("basic_info"),
      ),
      null,
      2,
    )}`,
  ).toBeTruthy();
  if (!basicInfoProbe?.coord)
    throw new Error("main graph basic_info cell is missing a coordinate");
  await clickGraphProbeAt(page, basicInfoProbe.coord);
  await waitForSubgraphSettled(page, basicInfoPathKey, 30_000);

  const durationProbe = (await readSubgraphWorkspaceClickProbes(page)).find(
    (probe) =>
      probe.target === "value" &&
      probe.text === "6837" &&
      matchesPath(probe, durationPath) &&
      probe.coord,
  );
  expect(durationProbe, "first-level subgraph duration cell").toBeTruthy();
  if (!durationProbe?.coord)
    throw new Error(
      "first-level subgraph duration cell is missing a coordinate",
    );
  await clickSubgraphWorkspaceProbeAt(page, durationProbe.coord);
  await waitForSubgraphSettled(page, durationPathKey, 30_000);

  await expect
    .poll(() => getMonacoValue(page, `subgraph-content:${durationPathKey}`), {
      timeout: 10_000,
    })
    .toBe('"6837"');
  await setMonacoValue(page, `subgraph-content:${durationPathKey}`, '"42"');

  // The content pane must not briefly fall back to number/neutral token colors
  // while the graph edit is being committed. Strings use the semantic blue.
  const subgraphColors = await sampleRenderedTokenColors(
    page,
    `subgraph-content:${durationPathKey}`,
    '"42"',
  );
  expect(subgraphColors).toEqual(Array(subgraphColors.length).fill("rgb(4, 81, 165)"));

  await expect.poll(() => readDurationFromSource(page), { timeout: 30_000 }).toBe("42");
  await waitForGraphRendered(page, 30_000);
  await waitForSubgraphSettled(page, durationPathKey, 30_000);

  await revealGraphPath(
    page,
    [
      { key: "agent_steps" },
      { index: 0 },
      { key: "steps" },
      { index: 6 },
      { key: "basic_info" },
      { key: "duration" },
    ],
    { target: "value", navigate: true },
  );
  await expect
    .poll(
      async () =>
        (await readGraphClickProbes(page)).some(
          (probe) =>
            probe.target === "value" &&
            probe.text === "42" &&
            matchesPath(probe, durationPath) &&
            probe.coord,
        ),
      { timeout: 10_000 },
    )
    .toBe(true);
  const updatedDurationProbe = (await readGraphClickProbes(page)).find(
    (probe) =>
      probe.target === "value" &&
      probe.text === "42" &&
      matchesPath(probe, durationPath) &&
      probe.coord,
  );
  expect(updatedDurationProbe, "edited main-graph duration cell").toBeTruthy();
  if (!updatedDurationProbe?.coord)
    throw new Error("edited duration cell is missing a coordinate");

  await clickGraphProbeAt(page, updatedDurationProbe.coord);
  await expect
    .poll(() => readGraphHighlight(page), { timeout: 5_000 })
    .toMatchObject({
      path: ["$", ...durationPath],
      target: "value",
    });
  await expect
    .poll(() => getMonacoRenderedTokenColor(page, "source-editor", '"42"'), {
      timeout: 5_000,
    })
    .toBe("rgb(4, 81, 165)");
  await expect(
    page
      .getByTestId("graph-subgraph-workspace")
      .locator(".treease-json-block-highlight"),
  ).toHaveCount(0);
});
