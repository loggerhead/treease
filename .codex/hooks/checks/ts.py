"""Run the smallest documented quality checks for changed TypeScript files."""

from __future__ import annotations

import subprocess
from pathlib import Path


TS_EXTENSIONS = (".ts", ".tsx", ".mts", ".cts")
CHECKS_BY_DIRECTORY = {
    "apps/web/": ("lint", "check"),
    "apps/server/": ("check",),
}


def run_script(root: Path, directory: str, script: str) -> int:
    print(f"[codex-hook] pnpm --dir {directory} {script}")
    result = subprocess.run(
        ["pnpm", "--dir", directory, script],
        cwd=root,
    )
    return result.returncode


def run(root: Path, files: list[str]) -> int:
    changed = [path for path in files if path.endswith(TS_EXTENSIONS)]
    failures = 0

    for directory, scripts in CHECKS_BY_DIRECTORY.items():
        if not any(path.startswith(directory) for path in changed):
            continue

        for script in scripts:
            status = run_script(root, directory, script)
            failures |= status
            if status:
                break

    return failures
