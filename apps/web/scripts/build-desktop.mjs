import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function run(args, env) {
  execFileSync(pnpm, args, {
    cwd: webDir,
    stdio: "inherit",
    env,
    shell: process.platform === "win32",
  });
}

if (process.env.TREEASE_SKIP_WASM_BINDGEN === "1") {
  run(["wasm:validate"]);
} else {
  run(["wasm:bindgen:check"]);
  run(["wasm:sync"]);
}
run(["sitemap:generate"]);
run(["exec", "node", "./scripts/with-build-lock.mjs", "vp", "build"], {
  ...process.env,
  TREEASE_WORKSPACE_SURFACE: "desktop",
  WASM_VERSION: execFileSync(
    process.execPath,
    [path.join(webDir, "scripts/wasm-version.mjs")],
    {
      cwd: webDir,
      encoding: "utf8",
    },
  ).trim(),
});
