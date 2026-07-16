"""Run rustfmt only for crates containing changed Rust files."""

from __future__ import annotations

import subprocess
from pathlib import Path


def cargo_manifests(root: Path) -> list[Path]:
    result = subprocess.run(
        ["find", ".", "-type", "f", "-name", "Cargo.toml"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return sorted(
        root / path.removeprefix("./")
        for path in result.stdout.splitlines()
        if "/target/" not in path
    )


def manifest_for(path: Path, manifests: list[Path]) -> Path | None:
    candidates = [
        manifest
        for manifest in manifests
        if path.is_relative_to(manifest.parent)
    ]
    return max(candidates, key=lambda manifest: len(manifest.parts), default=None)


def run(root: Path, files: list[str]) -> int:
    manifests = cargo_manifests(root)
    changed = [root / path for path in files if path.endswith(".rs")]
    affected = {
        manifest
        for path in changed
        if (manifest := manifest_for(path, manifests)) is not None
    }

    status = 0
    for manifest in sorted(affected):
        relative_manifest = manifest.relative_to(root)
        print(f"[codex-hook] cargo fmt --manifest-path {relative_manifest}")
        result = subprocess.run(
            ["cargo", "fmt", "--manifest-path", relative_manifest, "--all"],
            cwd=root,
        )
        status |= result.returncode
    return status
