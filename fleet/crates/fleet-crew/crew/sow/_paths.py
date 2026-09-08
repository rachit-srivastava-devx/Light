"""Default citation-resolution root."""

from __future__ import annotations

from pathlib import Path


def _default_repo_root(repo_root: Path | None) -> Path:
    if repo_root is not None:
        return repo_root.resolve()
    # crew/sow/_paths.py -> this crate's own root (crates/fleet-crew).
    return Path(__file__).resolve().parents[2]
