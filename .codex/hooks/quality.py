#!/usr/bin/env python3
"""Route changed files to the smallest relevant Treease quality checks."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from checks import docs, rust, ts


def repo_root() -> Path:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip())


def changed_files(root: Path) -> list[str]:
    tracked = subprocess.run(
        ["git", "diff", "--name-only", "HEAD", "--"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    return sorted(set(tracked + untracked))


def main() -> int:
    # Codex sends the hook event as JSON on stdin. The router does not need
    # event-specific fields, but consuming stdin keeps the hook well-behaved.
    sys.stdin.read()

    root = repo_root()
    files = changed_files(root)
    checks = (rust, ts, docs)
    failures = 0

    for check in checks:
        failures |= check.run(root, files)

    return failures


if __name__ == "__main__":
    raise SystemExit(main())
