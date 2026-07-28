import { execFileSync, spawn } from "node:child_process";
import process from "node:process";

const buildDir = `${process.cwd()}/build`;
const routes = ["/", "/editor", "/index.html", "/200.html"];

console.log(`node ${process.version}`);
console.log(execFileSync("pnpm", ["exec", "vp", "--version"], { encoding: "utf8" }).trim());

function startPreview(port, explicitOutDir) {
  const args = ["exec", "vp", "preview"];
  if (explicitOutDir) args.push("--outDir", buildDir);
  args.push("--host", "localhost", "--port", String(port), "--strictPort");
  return spawn("pnpm", args, { detached: true, stdio: "inherit" });
}

async function waitForPreview(port) {
  const url = `http://localhost:${port}/`;
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      await fetch(url);
      return;
    } catch {
      // The preview process may still be starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  throw new Error(`Preview server did not become ready on port ${port}`);
}

async function inspectPreview(port, label) {
  await waitForPreview(port);
  const responses = new Map();
  for (const route of routes) {
    const response = await fetch(`http://localhost:${port}${route}`);
    const body = await response.text();
    responses.set(route, { status: response.status, body });
    console.log(`${label} ${route}: ${response.status}`);
  }
  return responses;
}

function stopPreview(server) {
  if (!server.pid) return;
  try {
    if (process.platform === "win32") {
      server.kill("SIGTERM");
    } else {
      process.kill(-server.pid, "SIGTERM");
    }
  } catch {
    // The process may have already exited after the preview check.
  }
}

const defaultPreview = startPreview(4173, false);
const explicitPreview = startPreview(4174, true);

try {
  const [defaultRoutes, explicitRoutes] = await Promise.all([
    inspectPreview(4173, "default"),
    inspectPreview(4174, "explicit --outDir"),
  ]);

  const fallbackRoutes = {
    "/": "/index.html",
    "/editor": "/200.html",
  };
  for (const route of routes) {
    const explicitResult = explicitRoutes.get(route);
    if (explicitResult.status < 200 || explicitResult.status >= 300) {
      throw new Error(`Explicit --outDir preview route ${route} is not available`);
    }
  }

  for (const [route, target] of Object.entries(fallbackRoutes)) {
    if (explicitRoutes.get(route).body !== explicitRoutes.get(target).body) {
      throw new Error(`Explicit --outDir preview route ${route} does not serve ${target}`);
    }
  }

  for (const route of routes) {
    const defaultResult = defaultRoutes.get(route);
    const explicitResult = explicitRoutes.get(route);
    if (
      defaultResult.status !== explicitResult.status ||
      defaultResult.body !== explicitResult.body
    ) {
      console.warn(
        `--outDir changes preview output for ${route}: default=${defaultResult.status}, explicit=${explicitResult.status}`,
      );
    }
  }
} finally {
  stopPreview(defaultPreview);
  stopPreview(explicitPreview);
}
