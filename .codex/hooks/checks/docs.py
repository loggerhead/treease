"""Run the documented docs structure check when docs-related files change."""

from __future__ import annotations

import subprocess
from pathlib import Path


def run(root: Path, files: list[str]) -> int:
    docs_changed = any(path.startswith("docs/") for path in files)
    docs_script_changed = any(
        path in {
            "scripts/check-docs.mjs",
            "scripts/docs-list.mjs",
            "scripts/generate-docs-map.mjs",
        }
        for path in files
    )
    if not docs_changed and not docs_script_changed:
        return 0

    print("[codex-hook] docs structure check")
    result = subprocess.run(["node", "scripts/check-docs.mjs"], cwd=root)
    return result.returncode
